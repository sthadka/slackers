use crate::slack::emoji;
use regex::Regex;

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
}
