/// Redact a secret value, showing only the beginning and end
///
/// Used for displaying tokens and cookies safely in `auth whoami` output.
///
/// # Examples
///
/// ```
/// let token = "xoxb-1234567890-1234567890-abcdefghijklmnopqrstuv";
/// let redacted = redact_secret(token, 6, 4);
/// assert_eq!(redacted, "xoxb-1***stuv");
/// ```
pub fn redact_secret(value: &str, keep_start: usize, keep_end: usize) -> String {
    let len = value.len();

    // If the value is too short to redact meaningfully, fully redact
    if len < keep_start + keep_end {
        return "***".to_string();
    }

    let start = &value[..keep_start];
    let end = &value[len - keep_end..];

    format!("{}***{}", start, end)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_redact_secret_default() {
        let token = "xoxb-1234567890-1234567890-abcdefghijklmnopqrstuv";
        let redacted = redact_secret(token, 6, 4);
        assert_eq!(redacted, "xoxb-1***stuv");
    }

    #[test]
    fn test_redact_secret_custom() {
        let value = "verylongsecretvalue";
        let redacted = redact_secret(value, 4, 4);
        assert_eq!(redacted, "very***alue");
    }

    #[test]
    fn test_redact_secret_short_value() {
        let value = "short";
        let redacted = redact_secret(value, 6, 4);
        assert_eq!(redacted, "***");
    }

    #[test]
    fn test_redact_secret_exact_length() {
        let value = "1234567890"; // 10 chars
        let redacted = redact_secret(value, 6, 4);
        assert_eq!(redacted, "123456***7890");
    }
}
