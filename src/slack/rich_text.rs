use regex::Regex;
use serde_json::{json, Value};
use std::sync::LazyLock;

static BULLET_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(\s*)[•◦▪▫▸‣●○◆◇\-*]\s+(.*)$").unwrap());
static ORDERED_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(\s*)\d+[.)]\s+(.*)$").unwrap());
static CODE_BLOCK_START: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^```").unwrap());
static BLOCKQUOTE_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^> (.*)$").unwrap());

// Separate regexes for patterns that need word-boundary awareness, to avoid
// look-behind which the `regex` crate does not support. Each captures an
// optional preceding character (group 1) that must be non-word or absent.
static EMOJI_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?:^|([^A-Za-z0-9_])):([a-zA-Z0-9_+\-]+):").unwrap()
});
static BARE_USER_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?:^|([^A-Za-z0-9_]))@([UWB][A-Z0-9]{6,})\b").unwrap()
});
static BARE_BROADCAST_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?:^|([^A-Za-z0-9_]))@(here|channel|everyone)\b").unwrap()
});

// Core inline regex for patterns that don't need look-behind.
static INLINE_CORE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(concat!(
        r"`([^`]+)`",                            // 1: code
        r"|\*([^*]+)\*",                          // 2: bold
        r"|_([^_]+)_",                            // 3: italic
        r"|~([^~]+)~",                            // 4: strike
        r"|<@([UWB][A-Z0-9]+)(?:\|[^>]*)?>",     // 5: user mention
        r"|<#([CG][A-Z0-9]+)(?:\|[^>]*)?>",      // 6: channel mention
        r"|<!subteam\^([A-Z0-9]+)(?:\|[^>]*)?>",  // 7: usergroup
        r"|<!(here|channel|everyone)(?:\|[^>]*)?>", // 8: broadcast
        r"|<([^>|]+)\|([^>]+)>",                   // 9,10: link with label
        r"|<([^>|]+)>",                            // 11: bare link
    ))
    .unwrap()
});

fn is_slack_manual_link_url(value: &str) -> bool {
    value.starts_with("http://")
        || value.starts_with("https://")
        || value.starts_with("mailto:")
}

#[derive(Debug)]
struct InlineMatch {
    start: usize,
    end: usize,
    element: Value,
}

fn parse_inline_elements(text: &str) -> Vec<Value> {
    let mut matches: Vec<InlineMatch> = Vec::new();

    for caps in INLINE_CORE_RE.captures_iter(text) {
        let m = caps.get(0).unwrap();
        let element = if let Some(code) = caps.get(1) {
            json!({"type": "text", "text": code.as_str(), "style": {"code": true}})
        } else if let Some(bold) = caps.get(2) {
            json!({"type": "text", "text": bold.as_str(), "style": {"bold": true}})
        } else if let Some(italic) = caps.get(3) {
            json!({"type": "text", "text": italic.as_str(), "style": {"italic": true}})
        } else if let Some(strike) = caps.get(4) {
            json!({"type": "text", "text": strike.as_str(), "style": {"strike": true}})
        } else if let Some(user) = caps.get(5) {
            json!({"type": "user", "user_id": user.as_str()})
        } else if let Some(channel) = caps.get(6) {
            json!({"type": "channel", "channel_id": channel.as_str()})
        } else if let Some(usergroup) = caps.get(7) {
            json!({"type": "usergroup", "usergroup_id": usergroup.as_str()})
        } else if let Some(broadcast) = caps.get(8) {
            json!({"type": "broadcast", "range": broadcast.as_str()})
        } else if let (Some(link_url), Some(link_text)) = (caps.get(9), caps.get(10)) {
            if is_slack_manual_link_url(link_url.as_str()) {
                json!({"type": "link", "url": link_url.as_str(), "text": link_text.as_str()})
            } else {
                json!({"type": "text", "text": format!("<{}|{}>", link_url.as_str(), link_text.as_str())})
            }
        } else if let Some(bare_url) = caps.get(11) {
            if is_slack_manual_link_url(bare_url.as_str()) {
                json!({"type": "link", "url": bare_url.as_str()})
            } else {
                json!({"type": "text", "text": format!("<{}>", bare_url.as_str())})
            }
        } else {
            continue;
        };
        matches.push(InlineMatch {
            start: m.start(),
            end: m.end(),
            element,
        });
    }

    // Emoji: the match may include a preceding non-word char that we must
    // leave in the text stream rather than swallow.
    for caps in EMOJI_RE.captures_iter(text) {
        let m = caps.get(0).unwrap();
        let pre = caps.get(1);
        let name = caps.get(2).unwrap();
        let start = pre.map_or(m.start(), |p| p.end());
        if matches.iter().any(|im| im.start < m.end() && im.end > m.start()) {
            continue;
        }
        matches.push(InlineMatch {
            start,
            end: m.end(),
            element: json!({"type": "emoji", "name": name.as_str()}),
        });
    }

    for caps in BARE_USER_RE.captures_iter(text) {
        let m = caps.get(0).unwrap();
        let pre = caps.get(1);
        let user_id = caps.get(2).unwrap();
        let start = pre.map_or(m.start(), |p| p.end());
        if matches.iter().any(|im| im.start < m.end() && im.end > m.start()) {
            continue;
        }
        matches.push(InlineMatch {
            start,
            end: m.end(),
            element: json!({"type": "user", "user_id": user_id.as_str()}),
        });
    }

    for caps in BARE_BROADCAST_RE.captures_iter(text) {
        let m = caps.get(0).unwrap();
        let pre = caps.get(1);
        let range = caps.get(2).unwrap();
        let start = pre.map_or(m.start(), |p| p.end());
        if matches.iter().any(|im| im.start < m.end() && im.end > m.start()) {
            continue;
        }
        matches.push(InlineMatch {
            start,
            end: m.end(),
            element: json!({"type": "broadcast", "range": range.as_str()}),
        });
    }

    matches.sort_by_key(|m| m.start);

    let mut elements = Vec::new();
    let mut last_index = 0;

    for im in &matches {
        if im.start > last_index {
            let slice = &text[last_index..im.start];
            if !slice.is_empty() {
                elements.push(json!({"type": "text", "text": slice}));
            }
        }
        elements.push(im.element.clone());
        last_index = im.end;
    }

    if last_index < text.len() {
        let slice = &text[last_index..];
        if !slice.is_empty() {
            elements.push(json!({"type": "text", "text": slice}));
        }
    }

    if elements.is_empty() {
        vec![json!({"type": "text", "text": text})]
    } else {
        elements
    }
}

fn has_rich_inline_formatting(elements: &[Value]) -> bool {
    elements.iter().any(|el| {
        let el_type = el.get("type").and_then(|v| v.as_str()).unwrap_or("");
        el_type != "text" || el.get("style").is_some()
    })
}

fn collect_list(
    lines: &[&str],
    start_idx: usize,
    style: &str,
    pattern: &Regex,
    elements: &mut Vec<Value>,
) -> usize {
    let mut idx = start_idx;

    let first_match = pattern.captures(lines[start_idx]).unwrap();
    let base_indent = first_match.get(1).map_or(0, |m| m.as_str().len());

    let mut current_indent: i32 = -1;
    let mut current_items: Vec<Value> = Vec::new();

    while idx < lines.len() {
        let Some(caps) = pattern.captures(lines[idx]) else {
            break;
        };

        let indent_len = caps.get(1).map_or(0, |m| m.as_str().len());
        let indent: i32 = if indent_len >= base_indent + 2 { 1 } else { 0 };
        let content = caps.get(2).map_or("", |m| m.as_str());

        if current_indent != -1 && indent != current_indent {
            let mut list_block = json!({
                "type": "rich_text_list",
                "style": style,
                "elements": current_items,
            });
            if current_indent > 0 {
                list_block["indent"] = json!(current_indent);
            }
            elements.push(list_block);
            current_items = Vec::new();
        }

        current_indent = indent;
        current_items.push(json!({
            "type": "rich_text_section",
            "elements": parse_inline_elements(content),
        }));
        idx += 1;
    }

    if !current_items.is_empty() {
        let mut list_block = json!({
            "type": "rich_text_list",
            "style": style,
            "elements": current_items,
        });
        if current_indent > 0 {
            list_block["indent"] = json!(current_indent);
        }
        elements.push(list_block);
    }

    idx
}

pub fn text_to_rich_text_blocks(text: &str) -> Option<Vec<Value>> {
    let lines: Vec<&str> = text.split('\n').collect();
    let mut elements: Vec<Value> = Vec::new();
    let mut has_lists = false;
    let mut has_formatting = false;
    let mut idx = 0;

    while idx < lines.len() {
        let line = lines[idx];

        // Code block
        if CODE_BLOCK_START.is_match(line) {
            idx += 1;
            let mut code_lines: Vec<&str> = Vec::new();
            while idx < lines.len() && !CODE_BLOCK_START.is_match(lines[idx]) {
                code_lines.push(lines[idx]);
                idx += 1;
            }
            if idx < lines.len() {
                idx += 1;
            }
            elements.push(json!({
                "type": "rich_text_preformatted",
                "elements": [{"type": "text", "text": code_lines.join("\n")}],
            }));
            has_formatting = true;
            continue;
        }

        // Blockquote
        if BLOCKQUOTE_RE.is_match(line) {
            let mut quote_lines: Vec<String> = Vec::new();
            while idx < lines.len() {
                if let Some(qm) = BLOCKQUOTE_RE.captures(lines[idx]) {
                    quote_lines.push(qm.get(1).map_or("", |m| m.as_str()).to_string());
                    idx += 1;
                } else {
                    break;
                }
            }
            elements.push(json!({
                "type": "rich_text_quote",
                "elements": parse_inline_elements(&quote_lines.join("\n")),
            }));
            has_formatting = true;
            continue;
        }

        // Bullet list
        if BULLET_RE.is_match(line) {
            has_lists = true;
            idx = collect_list(&lines, idx, "bullet", &BULLET_RE, &mut elements);
            continue;
        }

        // Ordered list
        if ORDERED_RE.is_match(line) {
            has_lists = true;
            idx = collect_list(&lines, idx, "ordered", &ORDERED_RE, &mut elements);
            continue;
        }

        // Plain text
        let mut text_lines: Vec<&str> = Vec::new();
        while idx < lines.len() {
            let l = lines[idx];
            if BULLET_RE.is_match(l)
                || ORDERED_RE.is_match(l)
                || CODE_BLOCK_START.is_match(l)
                || BLOCKQUOTE_RE.is_match(l)
            {
                break;
            }
            text_lines.push(l);
            idx += 1;
        }
        let content = text_lines.join("\n");
        if !content.trim().is_empty() {
            let content_with_newline = if content.ends_with('\n') {
                content.clone()
            } else {
                format!("{}\n", content)
            };
            let inline_elements = parse_inline_elements(&content_with_newline);
            if has_rich_inline_formatting(&inline_elements) {
                has_formatting = true;
            }
            elements.push(json!({
                "type": "rich_text_section",
                "elements": inline_elements,
            }));
        }
    }

    if !has_lists && !has_formatting {
        return None;
    }

    Some(vec![json!({"type": "rich_text", "elements": elements})])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plain_text_returns_none() {
        assert!(text_to_rich_text_blocks("hello world").is_none());
    }

    #[test]
    fn test_bullet_list() {
        let blocks = text_to_rich_text_blocks("- item one\n- item two").unwrap();
        assert_eq!(blocks.len(), 1);
        let rt = &blocks[0];
        assert_eq!(rt["type"], "rich_text");
        let elements = rt["elements"].as_array().unwrap();
        assert_eq!(elements[0]["type"], "rich_text_list");
        assert_eq!(elements[0]["style"], "bullet");
        let items = elements[0]["elements"].as_array().unwrap();
        assert_eq!(items.len(), 2);
    }

    #[test]
    fn test_ordered_list() {
        let blocks = text_to_rich_text_blocks("1. first\n2. second\n3. third").unwrap();
        let elements = blocks[0]["elements"].as_array().unwrap();
        assert_eq!(elements[0]["type"], "rich_text_list");
        assert_eq!(elements[0]["style"], "ordered");
        let items = elements[0]["elements"].as_array().unwrap();
        assert_eq!(items.len(), 3);
    }

    #[test]
    fn test_code_block() {
        let text = "```\nfn main() {\n    println!(\"hello\");\n}\n```";
        let blocks = text_to_rich_text_blocks(text).unwrap();
        let elements = blocks[0]["elements"].as_array().unwrap();
        assert_eq!(elements[0]["type"], "rich_text_preformatted");
        let code_text = elements[0]["elements"][0]["text"].as_str().unwrap();
        assert!(code_text.contains("fn main()"));
    }

    #[test]
    fn test_blockquote() {
        let text = "> quoted text\n> more quoted";
        let blocks = text_to_rich_text_blocks(text).unwrap();
        let elements = blocks[0]["elements"].as_array().unwrap();
        assert_eq!(elements[0]["type"], "rich_text_quote");
    }

    #[test]
    fn test_bold_formatting() {
        let text = "this is *bold* text";
        let blocks = text_to_rich_text_blocks(text).unwrap();
        let elements = blocks[0]["elements"].as_array().unwrap();
        assert_eq!(elements[0]["type"], "rich_text_section");
        let inline = elements[0]["elements"].as_array().unwrap();
        let bold_elem = inline.iter().find(|e| {
            e.get("style")
                .and_then(|s| s.get("bold"))
                .and_then(|b| b.as_bool())
                == Some(true)
        });
        assert!(bold_elem.is_some());
        assert_eq!(bold_elem.unwrap()["text"], "bold");
    }

    #[test]
    fn test_inline_code() {
        let text = "use `foo()` here";
        let blocks = text_to_rich_text_blocks(text).unwrap();
        let elements = blocks[0]["elements"].as_array().unwrap();
        let inline = elements[0]["elements"].as_array().unwrap();
        let code_elem = inline.iter().find(|e| {
            e.get("style")
                .and_then(|s| s.get("code"))
                .and_then(|b| b.as_bool())
                == Some(true)
        });
        assert!(code_elem.is_some());
        assert_eq!(code_elem.unwrap()["text"], "foo()");
    }

    #[test]
    fn test_user_mention() {
        let text = "hey <@U12345678>";
        let blocks = text_to_rich_text_blocks(text).unwrap();
        let elements = blocks[0]["elements"].as_array().unwrap();
        let inline = elements[0]["elements"].as_array().unwrap();
        let user_elem = inline.iter().find(|e| e["type"] == "user");
        assert!(user_elem.is_some());
        assert_eq!(user_elem.unwrap()["user_id"], "U12345678");
    }

    #[test]
    fn test_link() {
        let text = "visit <https://example.com|Example>";
        let blocks = text_to_rich_text_blocks(text).unwrap();
        let elements = blocks[0]["elements"].as_array().unwrap();
        let inline = elements[0]["elements"].as_array().unwrap();
        let link_elem = inline.iter().find(|e| e["type"] == "link");
        assert!(link_elem.is_some());
        assert_eq!(link_elem.unwrap()["url"], "https://example.com");
        assert_eq!(link_elem.unwrap()["text"], "Example");
    }

    #[test]
    fn test_emoji() {
        let text = "hello :wave: world";
        let blocks = text_to_rich_text_blocks(text).unwrap();
        let elements = blocks[0]["elements"].as_array().unwrap();
        let inline = elements[0]["elements"].as_array().unwrap();
        let emoji_elem = inline.iter().find(|e| e["type"] == "emoji");
        assert!(emoji_elem.is_some());
        assert_eq!(emoji_elem.unwrap()["name"], "wave");
    }

    #[test]
    fn test_mixed_content() {
        let text = "Intro text\n- bullet one\n- bullet two\n1. ordered one\n2. ordered two";
        let blocks = text_to_rich_text_blocks(text).unwrap();
        let elements = blocks[0]["elements"].as_array().unwrap();
        assert_eq!(elements[0]["type"], "rich_text_section");
        assert_eq!(elements[1]["type"], "rich_text_list");
        assert_eq!(elements[1]["style"], "bullet");
        assert_eq!(elements[2]["type"], "rich_text_list");
        assert_eq!(elements[2]["style"], "ordered");
    }

    #[test]
    fn test_broadcast() {
        let text = "attention <!here>";
        let blocks = text_to_rich_text_blocks(text).unwrap();
        let elements = blocks[0]["elements"].as_array().unwrap();
        let inline = elements[0]["elements"].as_array().unwrap();
        let bc_elem = inline.iter().find(|e| e["type"] == "broadcast");
        assert!(bc_elem.is_some());
        assert_eq!(bc_elem.unwrap()["range"], "here");
    }

    #[test]
    fn test_italic_and_strike() {
        let text = "_italic_ and ~strike~";
        let blocks = text_to_rich_text_blocks(text).unwrap();
        let inline = blocks[0]["elements"][0]["elements"].as_array().unwrap();
        let italic = inline.iter().find(|e| {
            e.get("style")
                .and_then(|s| s.get("italic"))
                .and_then(|b| b.as_bool())
                == Some(true)
        });
        let strike = inline.iter().find(|e| {
            e.get("style")
                .and_then(|s| s.get("strike"))
                .and_then(|b| b.as_bool())
                == Some(true)
        });
        assert!(italic.is_some());
        assert!(strike.is_some());
    }

    #[test]
    fn test_parse_inline_elements_plain() {
        let elements = parse_inline_elements("just text");
        assert_eq!(elements.len(), 1);
        assert_eq!(elements[0]["type"], "text");
        assert_eq!(elements[0]["text"], "just text");
    }

    #[test]
    fn test_channel_mention() {
        let elements = parse_inline_elements("see <#C12345678>");
        let ch = elements.iter().find(|e| e["type"] == "channel");
        assert!(ch.is_some());
        assert_eq!(ch.unwrap()["channel_id"], "C12345678");
    }

    #[test]
    fn test_bare_url() {
        let elements = parse_inline_elements("go to <https://example.com>");
        let link = elements.iter().find(|e| e["type"] == "link");
        assert!(link.is_some());
        assert_eq!(link.unwrap()["url"], "https://example.com");
    }

    #[test]
    fn test_indented_bullet_list() {
        let text = "- top\n  - nested\n- back";
        let blocks = text_to_rich_text_blocks(text).unwrap();
        let elements = blocks[0]["elements"].as_array().unwrap();
        let indented = elements.iter().find(|e| e.get("indent").is_some());
        assert!(indented.is_some());
    }
}
