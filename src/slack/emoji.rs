/// Normalize a reaction name to a shortcode (without colons)
///
/// Accepts:
/// - `:rocket:` -> `rocket`
/// - `rocket` -> `rocket`
/// - `🚀` -> `rocket`
pub fn normalize_reaction_name(input: &str) -> String {
    let trimmed = input.trim();

    // If it's a Unicode emoji, try to convert to shortcode
    if let Some(emoji) = emojis::get(trimmed) {
        return emoji.shortcode().unwrap_or(trimmed).to_string();
    }

    // Strip leading/trailing colons if present
    trimmed
        .strip_prefix(':')
        .and_then(|s| s.strip_suffix(':'))
        .unwrap_or(trimmed)
        .to_string()
}

/// Convert emoji shortcode to Unicode character
pub fn shortcode_to_unicode(shortcode: &str) -> Option<String> {
    let clean = shortcode
        .strip_prefix(':')
        .and_then(|s| s.strip_suffix(':'))
        .unwrap_or(shortcode);

    emojis::get_by_shortcode(clean).map(|e| e.as_str().to_string())
}

/// Convert Unicode emoji to shortcode (without colons)
#[allow(dead_code)]
pub fn emoji_to_shortcode(unicode: &str) -> Option<String> {
    emojis::get(unicode).and_then(|e| e.shortcode().map(|s| s.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_reaction_name_colon_format() {
        assert_eq!(normalize_reaction_name(":rocket:"), "rocket");
        assert_eq!(normalize_reaction_name(":+1:"), "+1");
    }

    #[test]
    fn test_normalize_reaction_name_bare() {
        assert_eq!(normalize_reaction_name("rocket"), "rocket");
        assert_eq!(normalize_reaction_name("+1"), "+1");
    }

    #[test]
    fn test_normalize_reaction_name_unicode() {
        let result = normalize_reaction_name("🚀");
        assert_eq!(result, "rocket");

        let result = normalize_reaction_name("👍");
        assert_eq!(result, "+1");
    }

    #[test]
    fn test_shortcode_to_unicode() {
        assert_eq!(shortcode_to_unicode("rocket"), Some("🚀".to_string()));
        assert_eq!(shortcode_to_unicode(":rocket:"), Some("🚀".to_string()));
        assert_eq!(shortcode_to_unicode("+1"), Some("👍".to_string()));
    }

    #[test]
    fn test_emoji_to_shortcode() {
        assert_eq!(emoji_to_shortcode("🚀"), Some("rocket".to_string()));
        assert_eq!(emoji_to_shortcode("👍"), Some("+1".to_string()));
    }
}
