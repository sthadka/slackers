use serde_json::Value;

/// Extract markdown content from Slack attachments (legacy format)
pub fn extract_mrkdwn_from_attachments(attachments: &[Value]) -> String {
    let mut parts = Vec::new();

    for attachment in attachments {
        // Pretext
        if let Some(pretext) = attachment.get("pretext").and_then(|v| v.as_str()) {
            if !pretext.is_empty() {
                parts.push(pretext.to_string());
            }
        }

        // Title (with link if present)
        if let Some(title) = attachment.get("title").and_then(|v| v.as_str()) {
            if !title.is_empty() {
                if let Some(title_link) = attachment.get("title_link").and_then(|v| v.as_str()) {
                    parts.push(format!("[{}]({})", title, title_link));
                } else {
                    parts.push(format!("**{}**", title));
                }
            }
        }

        // Text
        if let Some(text) = attachment.get("text").and_then(|v| v.as_str()) {
            if !text.is_empty() {
                parts.push(text.to_string());
            }
        }

        // Fields
        if let Some(fields) = attachment.get("fields").and_then(|v| v.as_array()) {
            for field in fields {
                if let (Some(title), Some(value)) = (
                    field.get("title").and_then(|v| v.as_str()),
                    field.get("value").and_then(|v| v.as_str()),
                ) {
                    parts.push(format!("**{}**: {}", title, value));
                }
            }
        }

        // Fallback
        if parts.is_empty() {
            if let Some(fallback) = attachment.get("fallback").and_then(|v| v.as_str()) {
                if !fallback.is_empty() {
                    parts.push(fallback.to_string());
                }
            }
        }
    }

    parts.join("\n\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_extract_from_simple_attachment() {
        let attachments = vec![json!({
            "text": "This is attachment text",
            "fallback": "Fallback text"
        })];

        let result = extract_mrkdwn_from_attachments(&attachments);
        assert_eq!(result, "This is attachment text");
    }

    #[test]
    fn test_extract_with_title() {
        let attachments = vec![json!({
            "title": "Important",
            "text": "Message text"
        })];

        let result = extract_mrkdwn_from_attachments(&attachments);
        assert!(result.contains("**Important**"));
        assert!(result.contains("Message text"));
    }

    #[test]
    fn test_extract_with_fields() {
        let attachments = vec![json!({
            "fields": [
                {"title": "Status", "value": "Active"},
                {"title": "Count", "value": "42"}
            ]
        })];

        let result = extract_mrkdwn_from_attachments(&attachments);
        assert!(result.contains("**Status**: Active"));
        assert!(result.contains("**Count**: 42"));
    }
}
