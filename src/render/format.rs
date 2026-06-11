//! Output formatting module for slackers CLI.
//!
//! Provides [`OutputFormat`] enum and the [`Formattable`] trait so command
//! handlers can render data in JSON, Table, Markdown, or Plain text.
//!
//! # Usage
//!
//! ```rust,ignore
//! use crate::render::format::{OutputFormat, Formattable};
//!
//! let rows = vec![
//!     vec!["channel".to_string(), "#general".to_string()],
//! ];
//! let output = OutputFormat::Table.render_rows(&["Name", "Value"], &rows);
//! println!("{}", output);
//! ```

use comfy_table::{Cell, Table};
use serde::Serialize;
use serde_json::Value;

/// Output format variants supported by CLI commands.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum OutputFormat {
    /// Pretty-printed JSON with empty fields pruned (default).
    #[default]
    Json,
    /// ASCII table via comfy-table.
    Table,
    /// GitHub-flavoured Markdown table.
    Markdown,
    /// Plain key=value lines, one per field.
    Plain,
}

impl OutputFormat {
    /// Parse a `--format` CLI string into an [`OutputFormat`].
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "json" => Some(Self::Json),
            "table" => Some(Self::Table),
            "markdown" | "md" => Some(Self::Markdown),
            "plain" | "text" => Some(Self::Plain),
            _ => None,
        }
    }

    /// Render a serialisable value according to this format.
    ///
    /// For `Json` this mirrors [`crate::output::to_json_output`].
    /// For the other formats the value is flattened into key/value rows.
    pub fn render<T: Serialize + ?Sized>(&self, value: &T) -> String {
        let json_val = serde_json::to_value(value).unwrap_or(Value::Null);
        match self {
            Self::Json => {
                let pruned = crate::output::prune_empty(json_val);
                serde_json::to_string_pretty(&pruned).unwrap_or_else(|_| "{}".to_string())
            }
            Self::Table => {
                let rows = value_to_rows(&json_val);
                format_table(&["Key", "Value"], &rows)
            }
            Self::Markdown => {
                let rows = value_to_rows(&json_val);
                format_markdown(&["Key", "Value"], &rows)
            }
            Self::Plain => {
                let rows = value_to_rows(&json_val);
                format_plain(&rows)
            }
        }
    }

    /// Render tabular data (headers + rows) according to this format.
    ///
    /// `Json` falls back to a JSON array of objects keyed by headers.
    pub fn render_rows(&self, headers: &[&str], rows: &[Vec<String>]) -> String {
        match self {
            Self::Json => {
                let objects: Vec<Value> = rows
                    .iter()
                    .map(|row| {
                        let mut obj = serde_json::Map::new();
                        for (h, v) in headers.iter().zip(row.iter()) {
                            obj.insert(h.to_string(), Value::String(v.clone()));
                        }
                        Value::Object(obj)
                    })
                    .collect();
                serde_json::to_string_pretty(&Value::Array(objects))
                    .unwrap_or_else(|_| "[]".to_string())
            }
            Self::Table => format_table(headers, rows),
            Self::Markdown => format_markdown(headers, rows),
            Self::Plain => format_plain(rows),
        }
    }
}

// ── Trait ────────────────────────────────────────────────────────────────────

/// A type that can format itself for CLI output.
pub trait Formattable: Serialize {
    /// Render `self` using the given [`OutputFormat`].
    fn format(&self, fmt: &OutputFormat) -> String where Self: Sized {
        fmt.render(self)
    }
}

/// Blanket implementation: any `Serialize` type is `Formattable`.
impl<T: Serialize> Formattable for T {}

// ── Helpers ──────────────────────────────────────────────────────────────────

/// Render an ASCII table using comfy-table.
pub fn format_table(headers: &[&str], rows: &[Vec<String>]) -> String {
    let mut table = Table::new();
    table.set_header(headers.iter().map(|h| Cell::new(h)));
    for row in rows {
        table.add_row(row.iter().map(|c| Cell::new(c)));
    }
    table.to_string()
}

/// Render a GFM Markdown table.
pub fn format_markdown(headers: &[&str], rows: &[Vec<String>]) -> String {
    let mut out = String::new();

    // Header row
    out.push('|');
    for h in headers {
        out.push(' ');
        out.push_str(h);
        out.push_str(" |");
    }
    out.push('\n');

    // Separator row
    out.push('|');
    for _ in headers {
        out.push_str(" --- |");
    }
    out.push('\n');

    // Data rows
    for row in rows {
        out.push('|');
        for cell in row {
            out.push(' ');
            // Escape pipe characters inside cells
            out.push_str(&cell.replace('|', "\\|"));
            out.push_str(" |");
        }
        out.push('\n');
    }

    out
}

/// Render rows as plain `key=value` lines (one row per record).
///
/// When there are exactly 2 columns the output is `key=value`.
/// Otherwise columns are joined with a tab character.
pub fn format_plain(rows: &[Vec<String>]) -> String {
    rows.iter()
        .map(|row| {
            if row.len() == 2 {
                format!("{}={}", row[0], row[1])
            } else {
                row.join("\t")
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

// ── Internal helpers ──────────────────────────────────────────────────────────

/// Flatten a JSON value into `(key, value)` string pairs for tabular display.
///
/// Objects are iterated over their fields; arrays become `[0]`, `[1]`, …;
/// scalars produce a single row with key `"value"`.
fn value_to_rows(val: &Value) -> Vec<Vec<String>> {
    match val {
        Value::Object(map) => map
            .iter()
            .map(|(k, v)| vec![k.clone(), scalar_to_string(v)])
            .collect(),
        Value::Array(arr) => arr
            .iter()
            .enumerate()
            .map(|(i, v)| vec![format!("[{}]", i), scalar_to_string(v)])
            .collect(),
        other => vec![vec!["value".to_string(), scalar_to_string(other)]],
    }
}

/// Convert a JSON value to a compact display string.
fn scalar_to_string(val: &Value) -> String {
    match val {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_format_from_str() {
        assert_eq!(OutputFormat::from_str("json"), Some(OutputFormat::Json));
        assert_eq!(OutputFormat::from_str("JSON"), Some(OutputFormat::Json));
        assert_eq!(OutputFormat::from_str("table"), Some(OutputFormat::Table));
        assert_eq!(OutputFormat::from_str("markdown"), Some(OutputFormat::Markdown));
        assert_eq!(OutputFormat::from_str("md"), Some(OutputFormat::Markdown));
        assert_eq!(OutputFormat::from_str("plain"), Some(OutputFormat::Plain));
        assert_eq!(OutputFormat::from_str("text"), Some(OutputFormat::Plain));
        assert_eq!(OutputFormat::from_str("unknown"), None);
    }

    #[test]
    fn test_format_json() {
        let val = json!({"name": "Alice", "score": 42});
        let out = OutputFormat::Json.render(&val);
        assert!(out.contains("\"name\": \"Alice\""));
        assert!(out.contains("\"score\": 42"));
    }

    #[test]
    fn test_format_table() {
        let headers = ["Name", "Value"];
        let rows = vec![
            vec!["channel".to_string(), "#general".to_string()],
            vec!["members".to_string(), "120".to_string()],
        ];
        let out = format_table(&headers, &rows);
        assert!(out.contains("Name"));
        assert!(out.contains("channel"));
        assert!(out.contains("#general"));
    }

    #[test]
    fn test_format_markdown() {
        let headers = ["Name", "Value"];
        let rows = vec![
            vec!["channel".to_string(), "#general".to_string()],
        ];
        let out = format_markdown(&headers, &rows);
        assert!(out.contains("| Name | Value |"));
        assert!(out.contains("| --- |"));
        assert!(out.contains("| channel | #general |"));
    }

    #[test]
    fn test_format_plain_two_cols() {
        let rows = vec![
            vec!["key".to_string(), "value".to_string()],
            vec!["foo".to_string(), "bar".to_string()],
        ];
        let out = format_plain(&rows);
        assert_eq!(out, "key=value\nfoo=bar");
    }

    #[test]
    fn test_format_plain_multi_col() {
        let rows = vec![vec!["a".to_string(), "b".to_string(), "c".to_string()]];
        let out = format_plain(&rows);
        assert_eq!(out, "a\tb\tc");
    }

    #[test]
    fn test_render_rows_json() {
        let headers = ["id", "name"];
        let rows = vec![
            vec!["U123".to_string(), "Alice".to_string()],
        ];
        let out = OutputFormat::Json.render_rows(&headers, &rows);
        let parsed: Value = serde_json::from_str(&out).unwrap();
        assert!(parsed.is_array());
        assert_eq!(parsed[0]["name"], "Alice");
    }

    #[test]
    fn test_formattable_trait() {
        use serde::Serialize;

        #[derive(Serialize)]
        struct Item {
            id: u32,
            label: String,
        }

        let item = Item { id: 1, label: "hello".to_string() };
        let out = item.format(&OutputFormat::Plain);
        // Object is flattened to key=value lines
        assert!(out.contains("id=1") || out.contains("label=hello"));
    }

    #[test]
    fn test_markdown_pipe_escape() {
        let headers = ["Col"];
        let rows = vec![vec!["a|b".to_string()]];
        let out = format_markdown(&headers, &rows);
        assert!(out.contains("a\\|b"));
    }

    #[test]
    fn test_value_to_rows_array() {
        let val = json!([10, 20]);
        let rows = value_to_rows(&val);
        assert_eq!(rows[0], vec!["[0]", "10"]);
        assert_eq!(rows[1], vec!["[1]", "20"]);
    }
}
