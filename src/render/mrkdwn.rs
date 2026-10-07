use crate::slack::emoji;
use crate::slack::users::CompactSlackUser;
use regex::Regex;
use std::collections::HashMap;
use std::sync::LazyLock;

/// Matches a bare user mention `<@Uxxx>` (no `|label`).
static BARE_USER_MENTION: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"<@((?:U|W)[A-Z0-9]{8,})>").unwrap());

/// Convert Slack mrkdwn to standard Markdown
///
/// Applies transformations in order:
/// 1. <URL|label> -> [label](URL)
/// 2. <URL> -> URL
/// 3. <#C123|name> -> #name
/// 4. <@U123|name> -> @name, <@U123> -> @U123
/// 5. <!here> -> @here, etc.
/// 6. HTML entities: &lt; &gt; &amp;
/// 7. Emoji shortcodes -> Unicode
pub fn mrkdwn_to_markdown(text: &str) -> String {
    let mut result = text.to_string();

    // 1. <URL|label> -> [label](URL)
    let re = Regex::new(r"<(https?://[^|>]+)\|([^>]+)>").unwrap();
    result = re.replace_all(&result, "[$2]($1)").to_string();

    // 2. <URL> -> URL (bare links)
    let re = Regex::new(r"<(https?://[^>]+)>").unwrap();
    result = re.replace_all(&result, "$1").to_string();

    // 3. <#C123|name> -> #name
    let re = Regex::new(r"<#[A-Z0-9]+\|([^>]+)>").unwrap();
    result = re.replace_all(&result, "#$1").to_string();

    // 4. <@U123|name> -> @name
    let re = Regex::new(r"<@[A-Z0-9]+\|([^>]+)>").unwrap();
    result = re.replace_all(&result, "@$1").to_string();

    // 4b. <@U123> -> @U123 (bare user mentions)
    let re = Regex::new(r"<@([A-Z0-9]+)>").unwrap();
    result = re.replace_all(&result, "@$1").to_string();

    // 5. Special mentions
    result = result.replace("<!here>", "@here");
    result = result.replace("<!channel>", "@channel");
    result = result.replace("<!everyone>", "@everyone");

    // 6. HTML entities
    result = result.replace("&lt;", "<");
    result = result.replace("&gt;", ">");
    result = result.replace("&amp;", "&");

    // 7. Emoji shortcodes -> Unicode
    let re = Regex::new(r":([a-z0-9_+-]+):").unwrap();
    result = re
        .replace_all(&result, |caps: &regex::Captures| {
            let shortcode = &caps[1];
            emoji::shortcode_to_unicode(shortcode).unwrap_or_else(|| format!(":{}:", shortcode))
        })
        .to_string();

    result
}

/// Convert Slack mrkdwn to Markdown, resolving bare `<@Uxxx>` mentions to
/// `@DisplayName` using `users`. Unknown ids fall back to `@Uxxx` (the existing
/// [`mrkdwn_to_markdown`] behavior). Mentions that already carry a `|label`
/// (`<@Uxxx|name>`) keep their label.
pub fn mrkdwn_to_markdown_with_users(
    text: &str,
    users: &HashMap<String, CompactSlackUser>,
) -> String {
    mrkdwn_to_markdown(&resolve_user_mentions(text, users))
}

/// Replace bare `<@Uxxx>` with `@DisplayName` for every known user, leaving
/// unknown ids as `<@Uxxx>` so downstream mrkdwn rendering handles them.
fn resolve_user_mentions(text: &str, users: &HashMap<String, CompactSlackUser>) -> String {
    if users.is_empty() {
        return text.to_string();
    }
    BARE_USER_MENTION
        .replace_all(text, |caps: &regex::Captures| match users.get(&caps[1]) {
            Some(user) => format!("@{}", user_display_name(user)),
            None => caps[0].to_string(),
        })
        .to_string()
}

/// Best display label for a user: display name, then real name, then handle,
/// falling back to the raw id.
fn user_display_name(user: &CompactSlackUser) -> &str {
    [
        user.display_name.as_deref(),
        user.real_name.as_deref(),
        user.name.as_deref(),
    ]
    .into_iter()
    .flatten()
    .find(|s| !s.is_empty())
    .unwrap_or(&user.id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_url_with_label() {
        let input = "Check out <https://example.com|this link>";
        let result = mrkdwn_to_markdown(input);
        assert_eq!(result, "Check out [this link](https://example.com)");
    }

    #[test]
    fn test_bare_url() {
        let input = "Visit <https://example.com>";
        let result = mrkdwn_to_markdown(input);
        assert_eq!(result, "Visit https://example.com");
    }

    #[test]
    fn test_channel_mention() {
        let input = "Posted in <#C0123456789|general>";
        let result = mrkdwn_to_markdown(input);
        assert_eq!(result, "Posted in #general");
    }

    #[test]
    fn test_user_mention() {
        let input = "Hey <@U0123456789|john>";
        let result = mrkdwn_to_markdown(input);
        assert_eq!(result, "Hey @john");

        let input = "Hey <@U0123456789>";
        let result = mrkdwn_to_markdown(input);
        assert_eq!(result, "Hey @U0123456789");
    }

    #[test]
    fn test_special_mentions() {
        assert_eq!(mrkdwn_to_markdown("<!here>"), "@here");
        assert_eq!(mrkdwn_to_markdown("<!channel>"), "@channel");
        assert_eq!(mrkdwn_to_markdown("<!everyone>"), "@everyone");
    }

    #[test]
    fn test_html_entities() {
        let input = "Code: &lt;div&gt; &amp; more";
        let result = mrkdwn_to_markdown(input);
        assert_eq!(result, "Code: <div> & more");
    }

    #[test]
    fn test_emoji_shortcodes() {
        let input = "Great work :rocket: :+1:";
        let result = mrkdwn_to_markdown(input);
        assert_eq!(result, "Great work 🚀 👍");
    }

    fn user(id: &str, display: Option<&str>, real: Option<&str>, name: Option<&str>) -> CompactSlackUser {
        CompactSlackUser {
            id: id.to_string(),
            name: name.map(|s| s.to_string()),
            real_name: real.map(|s| s.to_string()),
            display_name: display.map(|s| s.to_string()),
            email: None,
            title: None,
            tz: None,
            is_bot: None,
            deleted: None,
        }
    }

    #[test]
    fn test_bare_mention_resolves_to_display_name() {
        let mut users = HashMap::new();
        users.insert("U0123456789".to_string(), user("U0123456789", Some("janedoe"), Some("Jane Doe"), Some("jane")));
        let out = mrkdwn_to_markdown_with_users("Hey <@U0123456789>!", &users);
        assert_eq!(out, "Hey @janedoe!");
    }

    #[test]
    fn test_display_name_priority_falls_back() {
        let mut users = HashMap::new();
        // No display name -> real name; no real name -> handle.
        users.insert("U0000000001".to_string(), user("U0000000001", None, Some("Real Name"), Some("handle")));
        users.insert("U0000000002".to_string(), user("U0000000002", None, None, Some("handle2")));
        let out = mrkdwn_to_markdown_with_users("<@U0000000001> and <@U0000000002>", &users);
        assert_eq!(out, "@Real Name and @handle2");
    }

    #[test]
    fn test_unknown_mention_degrades_to_id() {
        // Empty map (e.g. a failed users.info fetch) leaves the raw id, matching
        // plain mrkdwn behavior — rendering never fails.
        let users = HashMap::new();
        let out = mrkdwn_to_markdown_with_users("Ping <@U0123456789>", &users);
        assert_eq!(out, "Ping @U0123456789");
    }

    #[test]
    fn test_labeled_mention_keeps_label() {
        let mut users = HashMap::new();
        users.insert("U0123456789".to_string(), user("U0123456789", Some("janedoe"), None, None));
        let out = mrkdwn_to_markdown_with_users("Hi <@U0123456789|jj>", &users);
        assert_eq!(out, "Hi @jj");
    }
}
