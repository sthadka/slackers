use regex::Regex;
use std::sync::LazyLock;

static SLACK_TOKEN_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"<(?:@[UWB][A-Z0-9]+(?:\|[^>]*)?|#[CG][A-Z0-9]+(?:\|[^>]*)?|!subteam\^[A-Z0-9]+(?:\|[^>]*)?|![a-zA-Z]+(?:\|[^>]*)?|(?:https?://|mailto:)[^>]+)>"
    ).unwrap()
});

static BARE_USER_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?:^|(?P<pre>[^A-Za-z0-9_]))@(?P<id>[UWB][A-Z0-9]{6,})\b").unwrap()
});

static BROADCAST_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?:^|(?P<pre>[^A-Za-z0-9_]))@(?P<name>here|channel|everyone)\b").unwrap()
});

static PLACEHOLDER_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\x00(\d+)\x00").unwrap()
});

/// Prepare user-authored text for Slack's `chat.postMessage` / `chat.update`.
///
/// Slack's mrkdwn contract requires:
///  - literal `&`, `<`, `>` escaped as `&amp;`, `&lt;`, `&gt;`
///  - user mentions wrapped as `<@U123>`, channel mentions as `<#C123>`,
///    usergroup mentions as `<!subteam^S123>`, and broadcast mentions as
///    `<!here>` / `<!channel>` / `<!everyone>`
///
/// Humans (and LLMs piping text into the CLI) commonly write `@U123` and
/// raw `&`/`<`/`>` — this helper normalizes that to what Slack expects,
/// while leaving already-well-formed Slack tokens intact.
pub fn format_outbound_slack_text(text: &str) -> String {
    if text.is_empty() {
        return String::new();
    }

    // Protect already-formatted Slack tokens so `<`/`>` inside them aren't escaped.
    let mut stash: Vec<String> = Vec::new();
    let out = SLACK_TOKEN_RE.replace_all(text, |caps: &regex::Captures| {
        let idx = stash.len();
        stash.push(caps[0].to_string());
        format!("\x00{}\x00", idx)
    });
    let mut out = out.into_owned();

    // Escape literal HTML-ish characters per Slack's mrkdwn rules.
    out = out.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;");

    // Promote bare user IDs (`@U05BRPTKL6A`) to real mentions.
    out = BARE_USER_RE
        .replace_all(&out, |caps: &regex::Captures| {
            let pre = caps.name("pre").map_or("", |m| m.as_str());
            let id = &caps["id"];
            format!("{}<@{}>", pre, id)
        })
        .into_owned();

    // Promote broadcast mentions.
    out = BROADCAST_RE
        .replace_all(&out, |caps: &regex::Captures| {
            let pre = caps.name("pre").map_or("", |m| m.as_str());
            let name = &caps["name"];
            format!("{}<!{}>", pre, name)
        })
        .into_owned();

    // Restore protected tokens.
    out = PLACEHOLDER_RE
        .replace_all(&out, |caps: &regex::Captures| {
            let idx: usize = caps[1].parse().unwrap();
            stash[idx].clone()
        })
        .into_owned();

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn promotes_bare_user_ids_to_slack_mention_tokens() {
        assert_eq!(
            format_outbound_slack_text("@U05BRPTKL6A heads up"),
            "<@U05BRPTKL6A> heads up"
        );
        assert_eq!(
            format_outbound_slack_text("cc @W123456A and @BABCDEFG"),
            "cc <@W123456A> and <@BABCDEFG>"
        );
    }

    #[test]
    fn leaves_already_formatted_mention_tokens_alone() {
        assert_eq!(
            format_outbound_slack_text("hi <@U123456A>!"),
            "hi <@U123456A>!"
        );
        assert_eq!(
            format_outbound_slack_text("hi <@U123456A|nick>!"),
            "hi <@U123456A|nick>!"
        );
    }

    #[test]
    fn leaves_already_formatted_usergroup_mention_tokens_alone() {
        assert_eq!(
            format_outbound_slack_text("ping <!subteam^S12345678|@team>"),
            "ping <!subteam^S12345678|@team>"
        );
        assert_eq!(
            format_outbound_slack_text("ping <!subteam^S12345678>"),
            "ping <!subteam^S12345678>"
        );
    }

    #[test]
    fn promotes_broadcast_mentions() {
        assert_eq!(
            format_outbound_slack_text("@here ping"),
            "<!here> ping"
        );
        assert_eq!(
            format_outbound_slack_text("cc @channel and @everyone"),
            "cc <!channel> and <!everyone>"
        );
    }

    #[test]
    fn escapes_bare_angle_brackets_and_ampersand() {
        assert_eq!(
            format_outbound_slack_text("a < b && c > d"),
            "a &lt; b &amp;&amp; c &gt; d"
        );
    }

    #[test]
    fn does_not_escape_inside_already_formatted_slack_tokens() {
        assert_eq!(
            format_outbound_slack_text("see <https://example.com|link>"),
            "see <https://example.com|link>"
        );
        assert_eq!(
            format_outbound_slack_text("mail <mailto:bob@example.com|Bob>"),
            "mail <mailto:bob@example.com|Bob>"
        );
        assert_eq!(
            format_outbound_slack_text("see <https://a.test/?x=1&y=2>"),
            "see <https://a.test/?x=1&y=2>"
        );
    }

    #[test]
    fn does_not_promote_email_like_or_mid_word_at() {
        assert_eq!(
            format_outbound_slack_text("mail me at user@Udomain.com"),
            "mail me at user@Udomain.com"
        );
    }

    #[test]
    fn handles_empty_input() {
        assert_eq!(format_outbound_slack_text(""), "");
    }

    #[test]
    fn real_world_ci_dump_stays_readable_with_mention_and_url() {
        let input =
            r#"@U05BRPTKL6A heads up: CI "Install dependencies" is failing: https://github.com/x/y/actions/runs/1 & it needs <fix>"#;
        assert_eq!(
            format_outbound_slack_text(input),
            r#"<@U05BRPTKL6A> heads up: CI "Install dependencies" is failing: https://github.com/x/y/actions/runs/1 &amp; it needs &lt;fix&gt;"#
        );
    }
}
