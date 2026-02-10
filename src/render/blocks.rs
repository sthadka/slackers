use serde_json::Value;

use super::{extract_mrkdwn_from_attachments, mrkdwn_to_markdown};

/// Render a Slack message's content to Markdown
///
/// Priority:
/// 1. blocks (modern Block Kit format)
/// 2. attachments (legacy format)
/// 3. text (plain fallback)
pub fn render_message_content(msg: &Value) -> String {
    // Try blocks first (modern format)
    if let Some(blocks) = msg.get("blocks").and_then(|v| v.as_array()) {
        if !blocks.is_empty() {
            let mrkdwn = render_blocks(blocks);
            if !mrkdwn.is_empty() {
                return mrkdwn_to_markdown(&mrkdwn);
            }
        }
    }

    // Fall back to attachments (legacy format)
    if let Some(attachments) = msg.get("attachments").and_then(|v| v.as_array()) {
        if !attachments.is_empty() {
            let mrkdwn = extract_mrkdwn_from_attachments(attachments);
            if !mrkdwn.is_empty() {
                return mrkdwn_to_markdown(&mrkdwn);
            }
        }
    }

    // Final fallback to plain text
    if let Some(text) = msg.get("text").and_then(|v| v.as_str()) {
        return mrkdwn_to_markdown(text);
    }

    String::new()
}

/// Render an array of Slack blocks to mrkdwn format
fn render_blocks(blocks: &[Value]) -> String {
    blocks
        .iter()
        .filter_map(|block| render_block(block))
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// Render a single Slack block to mrkdwn format
fn render_block(block: &Value) -> Option<String> {
    let block_type = block.get("type")?.as_str()?;

    match block_type {
        "section" => render_section_block(block),
        "rich_text" => render_rich_text_block(block),
        "actions" => render_actions_block(block),
        "context" => render_context_block(block),
        "image" => render_image_block(block),
        "header" => render_header_block(block),
        "divider" => Some("---".to_string()),
        _ => None,
    }
}

/// Render a section block
fn render_section_block(block: &Value) -> Option<String> {
    let mut parts = Vec::new();

    // Main text
    if let Some(text) = block.get("text") {
        if let Some(rendered) = render_text_object(text) {
            parts.push(rendered);
        }
    }

    // Fields (rendered as list)
    if let Some(fields) = block.get("fields").and_then(|v| v.as_array()) {
        for field in fields {
            if let Some(rendered) = render_text_object(field) {
                parts.push(rendered);
            }
        }
    }

    // Accessory (e.g., button)
    if let Some(accessory) = block.get("accessory") {
        if let Some(rendered) = render_element(accessory) {
            parts.push(rendered);
        }
    }

    if parts.is_empty() {
        None
    } else {
        Some(parts.join("\n"))
    }
}

/// Render a text object (mrkdwn or plain_text)
fn render_text_object(text: &Value) -> Option<String> {
    let text_str = text.get("text")?.as_str()?;
    Some(text_str.to_string())
}

/// Render a rich_text block (contains rich_text elements)
fn render_rich_text_block(block: &Value) -> Option<String> {
    let elements = block.get("elements")?.as_array()?;

    let rendered = elements
        .iter()
        .filter_map(|element| render_rich_text_element(element))
        .collect::<Vec<_>>()
        .join("\n");

    if rendered.is_empty() {
        None
    } else {
        Some(rendered)
    }
}

/// Render a rich_text element (section, preformatted, quote, list)
fn render_rich_text_element(element: &Value) -> Option<String> {
    let element_type = element.get("type")?.as_str()?;

    match element_type {
        "rich_text_section" => render_rich_text_section(element),
        "rich_text_preformatted" => render_rich_text_preformatted(element),
        "rich_text_quote" => render_rich_text_quote(element),
        "rich_text_list" => render_rich_text_list(element),
        _ => None,
    }
}

/// Render inline elements in a rich_text_section
fn render_rich_text_section(element: &Value) -> Option<String> {
    let elements = element.get("elements")?.as_array()?;

    let rendered = elements
        .iter()
        .filter_map(|e| render_inline_element(e))
        .collect::<Vec<_>>()
        .join("");

    if rendered.is_empty() {
        None
    } else {
        Some(rendered)
    }
}

/// Render a preformatted code block
fn render_rich_text_preformatted(element: &Value) -> Option<String> {
    let elements = element.get("elements")?.as_array()?;

    let content = elements
        .iter()
        .filter_map(|e| render_inline_element(e))
        .collect::<Vec<_>>()
        .join("");

    if content.is_empty() {
        None
    } else {
        Some(format!("```\n{}\n```", content))
    }
}

/// Render a quote block
fn render_rich_text_quote(element: &Value) -> Option<String> {
    let elements = element.get("elements")?.as_array()?;

    let content = elements
        .iter()
        .filter_map(|e| render_inline_element(e))
        .collect::<Vec<_>>()
        .join("");

    if content.is_empty() {
        None
    } else {
        Some(format!("> {}", content))
    }
}

/// Render a list (ordered or bullet)
fn render_rich_text_list(element: &Value) -> Option<String> {
    let elements = element.get("elements")?.as_array()?;
    let style = element.get("style")?.as_str()?;

    let items: Vec<String> = elements
        .iter()
        .enumerate()
        .filter_map(|(i, item)| {
            let item_elements = item.get("elements")?.as_array()?;
            let content = item_elements
                .iter()
                .filter_map(|e| render_inline_element(e))
                .collect::<Vec<_>>()
                .join("");

            if content.is_empty() {
                None
            } else {
                match style {
                    "ordered" => Some(format!("{}. {}", i + 1, content)),
                    "bullet" => Some(format!("• {}", content)),
                    _ => Some(format!("• {}", content)),
                }
            }
        })
        .collect();

    if items.is_empty() {
        None
    } else {
        Some(items.join("\n"))
    }
}

/// Render inline elements (text, link, emoji, user, channel)
fn render_inline_element(element: &Value) -> Option<String> {
    let element_type = element.get("type")?.as_str()?;

    match element_type {
        "text" => render_inline_text(element),
        "link" => render_inline_link(element),
        "emoji" => render_inline_emoji(element),
        "user" => render_inline_user(element),
        "channel" => render_inline_channel(element),
        "usergroup" => render_inline_usergroup(element),
        "date" => render_inline_date(element),
        _ => None,
    }
}

/// Render styled text (bold, italic, strike, code)
fn render_inline_text(element: &Value) -> Option<String> {
    let mut text = element.get("text")?.as_str()?.to_string();

    // Apply styles using mrkdwn syntax
    if let Some(style) = element.get("style").and_then(|v| v.as_object()) {
        if style.get("bold").and_then(|v| v.as_bool()).unwrap_or(false) {
            text = format!("*{}*", text);
        }
        if style.get("italic").and_then(|v| v.as_bool()).unwrap_or(false) {
            text = format!("_{}_", text);
        }
        if style
            .get("strike")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
        {
            text = format!("~{}~", text);
        }
        if style.get("code").and_then(|v| v.as_bool()).unwrap_or(false) {
            text = format!("`{}`", text);
        }
    }

    Some(text)
}

/// Render a link
fn render_inline_link(element: &Value) -> Option<String> {
    let url = element.get("url")?.as_str()?;

    if let Some(text) = element.get("text").and_then(|v| v.as_str()) {
        Some(format!("<{}|{}>", url, text))
    } else {
        Some(format!("<{}>", url))
    }
}

/// Render an emoji
fn render_inline_emoji(element: &Value) -> Option<String> {
    let name = element.get("name")?.as_str()?;
    Some(format!(":{}:", name))
}

/// Render a user mention
fn render_inline_user(element: &Value) -> Option<String> {
    let user_id = element.get("user_id")?.as_str()?;
    Some(format!("<@{}>", user_id))
}

/// Render a channel mention
fn render_inline_channel(element: &Value) -> Option<String> {
    let channel_id = element.get("channel_id")?.as_str()?;
    Some(format!("<#{}>", channel_id))
}

/// Render a usergroup mention
fn render_inline_usergroup(element: &Value) -> Option<String> {
    let usergroup_id = element.get("usergroup_id")?.as_str()?;
    Some(format!("<!subteam^{}>", usergroup_id))
}

/// Render a date
fn render_inline_date(element: &Value) -> Option<String> {
    let timestamp = element.get("timestamp")?.as_i64()?;
    if let Some(fallback) = element.get("fallback").and_then(|v| v.as_str()) {
        Some(fallback.to_string())
    } else {
        Some(timestamp.to_string())
    }
}

/// Render an actions block (buttons)
fn render_actions_block(block: &Value) -> Option<String> {
    let elements = block.get("elements")?.as_array()?;

    let rendered = elements
        .iter()
        .filter_map(|element| render_element(element))
        .collect::<Vec<_>>()
        .join(" | ");

    if rendered.is_empty() {
        None
    } else {
        Some(format!("[{}]", rendered))
    }
}

/// Render an interactive element (button, etc.)
fn render_element(element: &Value) -> Option<String> {
    let element_type = element.get("type")?.as_str()?;

    match element_type {
        "button" => {
            let text = element.get("text")?;
            let text_str = render_text_object(text)?;
            Some(format!("[{}]", text_str))
        }
        "image" => {
            let alt_text = element.get("alt_text")?.as_str()?;
            let image_url = element.get("image_url")?.as_str()?;
            Some(format!("![{}]({})", alt_text, image_url))
        }
        _ => None,
    }
}

/// Render a context block
fn render_context_block(block: &Value) -> Option<String> {
    let elements = block.get("elements")?.as_array()?;

    let rendered = elements
        .iter()
        .filter_map(|element| {
            if element.get("type")?.as_str()? == "mrkdwn"
                || element.get("type")?.as_str()? == "plain_text"
            {
                render_text_object(element)
            } else {
                render_element(element)
            }
        })
        .collect::<Vec<_>>()
        .join(" ");

    if rendered.is_empty() {
        None
    } else {
        Some(format!("_{}_", rendered))
    }
}

/// Render an image block
fn render_image_block(block: &Value) -> Option<String> {
    let image_url = block.get("image_url")?.as_str()?;
    let alt_text = block
        .get("alt_text")
        .and_then(|v| v.as_str())
        .unwrap_or("image");

    Some(format!("![{}]({})", alt_text, image_url))
}

/// Render a header block
fn render_header_block(block: &Value) -> Option<String> {
    let text = block.get("text")?;
    let text_str = render_text_object(text)?;
    Some(format!("# {}", text_str))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_render_simple_text_message() {
        let msg = json!({
            "text": "Hello, world!"
        });

        let result = render_message_content(&msg);
        assert_eq!(result, "Hello, world!");
    }

    #[test]
    fn test_render_section_block() {
        let msg = json!({
            "blocks": [
                {
                    "type": "section",
                    "text": {
                        "type": "mrkdwn",
                        "text": "This is a section block"
                    }
                }
            ]
        });

        let result = render_message_content(&msg);
        assert_eq!(result, "This is a section block");
    }

    #[test]
    fn test_render_rich_text_with_styles() {
        let msg = json!({
            "blocks": [
                {
                    "type": "rich_text",
                    "elements": [
                        {
                            "type": "rich_text_section",
                            "elements": [
                                {
                                    "type": "text",
                                    "text": "Bold text",
                                    "style": {
                                        "bold": true
                                    }
                                },
                                {
                                    "type": "text",
                                    "text": " and "
                                },
                                {
                                    "type": "text",
                                    "text": "italic text",
                                    "style": {
                                        "italic": true
                                    }
                                }
                            ]
                        }
                    ]
                }
            ]
        });

        let result = render_message_content(&msg);
        assert!(result.contains("Bold text"));
        assert!(result.contains("italic text"));
    }

    #[test]
    fn test_render_rich_text_list() {
        let msg = json!({
            "blocks": [
                {
                    "type": "rich_text",
                    "elements": [
                        {
                            "type": "rich_text_list",
                            "style": "bullet",
                            "elements": [
                                {
                                    "type": "rich_text_section",
                                    "elements": [
                                        {
                                            "type": "text",
                                            "text": "Item 1"
                                        }
                                    ]
                                },
                                {
                                    "type": "rich_text_section",
                                    "elements": [
                                        {
                                            "type": "text",
                                            "text": "Item 2"
                                        }
                                    ]
                                }
                            ]
                        }
                    ]
                }
            ]
        });

        let result = render_message_content(&msg);
        assert!(result.contains("• Item 1"));
        assert!(result.contains("• Item 2"));
    }

    #[test]
    fn test_render_code_block() {
        let msg = json!({
            "blocks": [
                {
                    "type": "rich_text",
                    "elements": [
                        {
                            "type": "rich_text_preformatted",
                            "elements": [
                                {
                                    "type": "text",
                                    "text": "const x = 42;"
                                }
                            ]
                        }
                    ]
                }
            ]
        });

        let result = render_message_content(&msg);
        assert!(result.contains("```"));
        assert!(result.contains("const x = 42;"));
    }

    #[test]
    fn test_render_user_mention() {
        let msg = json!({
            "blocks": [
                {
                    "type": "rich_text",
                    "elements": [
                        {
                            "type": "rich_text_section",
                            "elements": [
                                {
                                    "type": "text",
                                    "text": "Hello "
                                },
                                {
                                    "type": "user",
                                    "user_id": "U0123456789"
                                }
                            ]
                        }
                    ]
                }
            ]
        });

        let result = render_message_content(&msg);
        assert!(result.contains("@U0123456789"));
    }

    #[test]
    fn test_render_link() {
        let msg = json!({
            "blocks": [
                {
                    "type": "rich_text",
                    "elements": [
                        {
                            "type": "rich_text_section",
                            "elements": [
                                {
                                    "type": "link",
                                    "url": "https://example.com",
                                    "text": "Example"
                                }
                            ]
                        }
                    ]
                }
            ]
        });

        let result = render_message_content(&msg);
        assert_eq!(result, "[Example](https://example.com)");
    }

    #[test]
    fn test_fallback_to_attachments() {
        let msg = json!({
            "attachments": [
                {
                    "text": "Attachment text"
                }
            ]
        });

        let result = render_message_content(&msg);
        assert_eq!(result, "Attachment text");
    }

    #[test]
    fn test_render_image_block() {
        let msg = json!({
            "blocks": [
                {
                    "type": "image",
                    "image_url": "https://example.com/image.png",
                    "alt_text": "Example image"
                }
            ]
        });

        let result = render_message_content(&msg);
        assert_eq!(result, "![Example image](https://example.com/image.png)");
    }

    #[test]
    fn test_render_header_block() {
        let msg = json!({
            "blocks": [
                {
                    "type": "header",
                    "text": {
                        "type": "plain_text",
                        "text": "Main Title"
                    }
                }
            ]
        });

        let result = render_message_content(&msg);
        assert_eq!(result, "# Main Title");
    }
}
