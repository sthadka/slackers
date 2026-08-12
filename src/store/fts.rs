use crate::error::Result;
use rusqlite::params;
use serde::Serialize;

use super::Store;

/// A search result from FTS5 full-text search.
#[derive(Debug, Clone, Serialize)]
pub struct SearchResult {
    pub channel_id: String,
    pub ts: String,
    pub user_id: Option<String>,
    pub text: Option<String>,
    pub rendered: Option<String>,
    pub snippet: String,
    pub rank: f64,
    pub thread_ts: Option<String>,
}

/// Generate a snippet from source text by finding the first occurrence of any query term
/// and extracting surrounding context, wrapped in the given markers.
///
/// This is needed because FTS5 `snippet()` and `highlight()` functions do not work
/// with contentless tables (`content = ''`).
fn build_snippet(
    source: &str,
    query: &str,
    max_tokens: usize,
    before_mark: &str,
    after_mark: &str,
) -> String {
    // Extract simple terms from the FTS5 query (strip operators, quotes, etc.)
    let terms: Vec<String> = query
        .split(|c: char| !c.is_alphanumeric() && c != '*')
        .filter(|s| !s.is_empty())
        .filter(|s| {
            let upper = s.to_uppercase();
            upper != "AND" && upper != "OR" && upper != "NOT" && upper != "NEAR"
        })
        .map(|s| s.trim_end_matches('*').to_lowercase())
        .filter(|s| !s.is_empty())
        .collect();

    if terms.is_empty() {
        // No usable terms, return truncated source
        let words: Vec<&str> = source.split_whitespace().collect();
        return if words.len() <= max_tokens {
            source.to_string()
        } else {
            format!("{}...", words[..max_tokens].join(" "))
        };
    }

    let lower_source = source.to_lowercase();

    // Find the first matching term position (by character index)
    let mut best_pos: Option<usize> = None;
    let mut best_term = &terms[0];
    for term in &terms {
        if let Some(pos) = lower_source.find(term.as_str()) {
            if best_pos.is_none() || pos < best_pos.unwrap() {
                best_pos = Some(pos);
                best_term = term;
            }
        }
    }

    if best_pos.is_none() {
        // No match found in source text, return truncated source
        let words: Vec<&str> = source.split_whitespace().collect();
        return if words.len() <= max_tokens {
            source.to_string()
        } else {
            format!("{}...", words[..max_tokens].join(" "))
        };
    }

    // Build snippet with highlighting around matched terms
    let words: Vec<&str> = source.split_whitespace().collect();
    let mut result_words: Vec<String> = Vec::new();

    // Find the word index containing the match
    let match_byte_pos = best_pos.unwrap();
    let mut current_byte = 0;
    let mut match_word_idx = 0;
    for (i, word) in words.iter().enumerate() {
        let word_start = source[current_byte..].find(word).unwrap_or(0) + current_byte;
        let word_end = word_start + word.len();
        if word_start <= match_byte_pos && match_byte_pos < word_end {
            match_word_idx = i;
            break;
        }
        current_byte = word_end;
    }

    // Calculate window around match
    let half_window = max_tokens / 2;
    let start = match_word_idx.saturating_sub(half_window);
    let end = (match_word_idx + half_window + 1).min(words.len());

    let prefix = if start > 0 { "..." } else { "" };
    let suffix = if end < words.len() { "..." } else { "" };

    for word in &words[start..end] {
        let lower_word = word.to_lowercase();
        let mut highlighted = false;
        for term in &terms {
            if lower_word.starts_with(term.as_str()) || lower_word == *term {
                result_words.push(format!("{}{}{}", before_mark, word, after_mark));
                highlighted = true;
                break;
            }
        }
        if !highlighted {
            result_words.push(word.to_string());
        }
    }

    let _ = best_term; // suppress unused warning

    format!("{}{}{}", prefix, result_words.join(" "), suffix)
}

impl Store {
    /// Search messages using FTS5 MATCH with BM25 ranking.
    ///
    /// Supports FTS5 query syntax: prefix (`deploy*`), phrase (`"release candidate"`),
    /// boolean (`bug NOT wontfix`), column filters (`rendered:kubernetes`),
    /// and proximity (`NEAR(deploy prod, 5)`).
    ///
    /// Returns results ordered by BM25 rank (most relevant first).
    /// Snippets are generated from the `rendered` column with matched terms wrapped
    /// in `>>>` / `<<<` markers.
    pub fn search_messages(
        &self,
        query: &str,
        channel_id: Option<&str>,
        limit: u32,
    ) -> Result<Vec<SearchResult>> {
        self.search_internal(query, channel_id, limit, ">>>", "<<<")
    }

    /// Search messages with custom highlight markers.
    ///
    /// Like `search_messages`, but wraps matched terms in the given
    /// `before_mark`/`after_mark` strings in the snippet.
    pub fn search_with_highlight(
        &self,
        query: &str,
        channel_id: Option<&str>,
        limit: u32,
        before_mark: &str,
        after_mark: &str,
    ) -> Result<Vec<SearchResult>> {
        self.search_internal(query, channel_id, limit, before_mark, after_mark)
    }

    /// Internal search implementation shared by both public methods.
    fn search_internal(
        &self,
        query: &str,
        channel_id: Option<&str>,
        limit: u32,
        before_mark: &str,
        after_mark: &str,
    ) -> Result<Vec<SearchResult>> {
        let conn = self.conn.lock().map_err(|e| {
            crate::error::SlackersError::Store(format!("lock poisoned: {}", e))
        })?;

        let (sql, use_channel_filter) = if channel_id.is_some() {
            (
                "SELECT m.channel_id, m.ts, m.user_id, m.text, m.rendered, m.thread_ts,
                        rank
                 FROM messages_fts
                 JOIN messages_rowid_map map ON messages_fts.rowid = map.rowid
                 JOIN messages m ON m.channel_id = map.channel_id AND m.ts = map.ts
                 WHERE messages_fts MATCH ?1 AND m.channel_id = ?2 AND m.is_deleted = 0
                 ORDER BY rank
                 LIMIT ?3",
                true,
            )
        } else {
            (
                "SELECT m.channel_id, m.ts, m.user_id, m.text, m.rendered, m.thread_ts,
                        rank
                 FROM messages_fts
                 JOIN messages_rowid_map map ON messages_fts.rowid = map.rowid
                 JOIN messages m ON m.channel_id = map.channel_id AND m.ts = map.ts
                 WHERE messages_fts MATCH ?1 AND m.is_deleted = 0
                 ORDER BY rank
                 LIMIT ?2",
                false,
            )
        };

        let mut stmt = conn.prepare(sql)?;

        // Read raw rows (without snippet — not available for contentless FTS5)
        struct RawRow {
            channel_id: String,
            ts: String,
            user_id: Option<String>,
            text: Option<String>,
            rendered: Option<String>,
            thread_ts: Option<String>,
            rank: f64,
        }

        let map_row = |row: &rusqlite::Row<'_>| -> rusqlite::Result<RawRow> {
            Ok(RawRow {
                channel_id: row.get(0)?,
                ts: row.get(1)?,
                user_id: row.get(2)?,
                text: row.get(3)?,
                rendered: row.get(4)?,
                thread_ts: row.get(5)?,
                rank: row.get(6)?,
            })
        };

        let mut raw_rows = Vec::new();
        if use_channel_filter {
            let rows = stmt.query_map(params![query, channel_id.unwrap(), limit], map_row)?;
            for row in rows {
                raw_rows.push(row?);
            }
        } else {
            let rows = stmt.query_map(params![query, limit], map_row)?;
            for row in rows {
                raw_rows.push(row?);
            }
        }

        // Build snippets from the rendered content (or text as fallback)
        let results = raw_rows
            .into_iter()
            .map(|r| {
                let source = r.rendered.as_deref().or(r.text.as_deref()).unwrap_or("");
                let snippet = build_snippet(source, query, 32, before_mark, after_mark);
                SearchResult {
                    channel_id: r.channel_id,
                    ts: r.ts,
                    user_id: r.user_id,
                    text: r.text,
                    rendered: r.rendered,
                    snippet,
                    rank: r.rank,
                    thread_ts: r.thread_ts,
                }
            })
            .collect();

        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::params;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn now_epoch() -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64
    }

    /// Helper: insert a channel so foreign key constraints are satisfied.
    fn insert_channel(store: &Store, id: &str) {
        let conn = store.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO channels (id, name, synced_at) VALUES (?1, ?2, ?3)",
            params![id, format!("chan-{}", id), now_epoch()],
        )
        .unwrap();
    }

    /// Helper: insert a message into the store (triggers FTS indexing).
    fn insert_message(store: &Store, channel_id: &str, ts: &str, text: &str, rendered: &str) {
        store
            .upsert_message(channel_id, ts, Some("U001"), None, Some(text), Some(rendered), None, 0, None)
            .unwrap();
    }

    /// Helper: insert a message with a thread_ts.
    fn insert_threaded_message(
        store: &Store,
        channel_id: &str,
        ts: &str,
        thread_ts: &str,
        text: &str,
        rendered: &str,
    ) {
        store
            .upsert_message(
                channel_id,
                ts,
                Some("U001"),
                Some(thread_ts),
                Some(text),
                Some(rendered),
                None,
                0,
                None,
            )
            .unwrap();
    }

    #[test]
    fn test_search_basic() {
        let store = Store::open_in_memory().unwrap();
        insert_channel(&store, "C001");

        insert_message(&store, "C001", "1700000000.000001", "hello world", "hello world");
        insert_message(&store, "C001", "1700000000.000002", "goodbye world", "goodbye world");
        insert_message(&store, "C001", "1700000000.000003", "something else", "something else");

        let results = store.search_messages("hello", None, 10).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].ts, "1700000000.000001");
        assert_eq!(results[0].channel_id, "C001");
    }

    #[test]
    fn test_search_multiple_results() {
        let store = Store::open_in_memory().unwrap();
        insert_channel(&store, "C001");

        insert_message(&store, "C001", "1700000000.000001", "deploy to prod", "deploy to prod");
        insert_message(&store, "C001", "1700000000.000002", "deploy to staging", "deploy to staging");
        insert_message(&store, "C001", "1700000000.000003", "unrelated message", "unrelated message");

        let results = store.search_messages("deploy", None, 10).unwrap();
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_search_with_channel_filter() {
        let store = Store::open_in_memory().unwrap();
        insert_channel(&store, "C001");
        insert_channel(&store, "C002");

        insert_message(&store, "C001", "1700000000.000001", "kubernetes deploy", "kubernetes deploy");
        insert_message(&store, "C002", "1700000000.000002", "kubernetes issue", "kubernetes issue");

        let results = store.search_messages("kubernetes", Some("C001"), 10).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].channel_id, "C001");

        // Without filter, both appear
        let all = store.search_messages("kubernetes", None, 10).unwrap();
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn test_search_limit() {
        let store = Store::open_in_memory().unwrap();
        insert_channel(&store, "C001");

        for i in 1..=10 {
            insert_message(
                &store,
                "C001",
                &format!("1700000000.0000{:02}", i),
                &format!("deploy version {}", i),
                &format!("deploy version {}", i),
            );
        }

        let results = store.search_messages("deploy", None, 3).unwrap();
        assert_eq!(results.len(), 3);
    }

    #[test]
    fn test_search_prefix_query() {
        let store = Store::open_in_memory().unwrap();
        insert_channel(&store, "C001");

        insert_message(&store, "C001", "1700000000.000001", "deployment started", "deployment started");
        insert_message(&store, "C001", "1700000000.000002", "deployed successfully", "deployed successfully");
        insert_message(&store, "C001", "1700000000.000003", "unrelated message", "unrelated message");

        let results = store.search_messages("deploy*", None, 10).unwrap();
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_search_phrase_query() {
        let store = Store::open_in_memory().unwrap();
        insert_channel(&store, "C001");

        insert_message(
            &store,
            "C001",
            "1700000000.000001",
            "the release candidate is ready",
            "the release candidate is ready",
        );
        insert_message(
            &store,
            "C001",
            "1700000000.000002",
            "the candidate for release",
            "the candidate for release",
        );

        let results = store.search_messages("\"release candidate\"", None, 10).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].ts, "1700000000.000001");
    }

    #[test]
    fn test_search_boolean_query() {
        let store = Store::open_in_memory().unwrap();
        insert_channel(&store, "C001");

        insert_message(&store, "C001", "1700000000.000001", "bug found in prod", "bug found in prod");
        insert_message(&store, "C001", "1700000000.000002", "bug wontfix", "bug wontfix");
        insert_message(&store, "C001", "1700000000.000003", "feature request", "feature request");

        // FTS5 uses "NOT" (not "AND NOT") for boolean negation
        let results = store.search_messages("bug NOT wontfix", None, 10).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].ts, "1700000000.000001");
    }

    #[test]
    fn test_search_column_filter() {
        let store = Store::open_in_memory().unwrap();
        insert_channel(&store, "C001");

        // Text has "kubernetes" but rendered does not
        insert_message(
            &store,
            "C001",
            "1700000000.000001",
            "kubernetes cluster setup",
            "cluster setup guide",
        );
        // Rendered has "kubernetes"
        insert_message(
            &store,
            "C001",
            "1700000000.000002",
            "cluster setup",
            "kubernetes cluster setup",
        );

        let results = store.search_messages("rendered:kubernetes", None, 10).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].ts, "1700000000.000002");
    }

    #[test]
    fn test_search_proximity_query() {
        let store = Store::open_in_memory().unwrap();
        insert_channel(&store, "C001");

        insert_message(
            &store,
            "C001",
            "1700000000.000001",
            "we need to deploy to prod today",
            "we need to deploy to prod today",
        );
        insert_message(
            &store,
            "C001",
            "1700000000.000002",
            "deploy the fix we talked about last week then check prod",
            "deploy the fix we talked about last week then check prod",
        );

        let results = store.search_messages("NEAR(deploy prod, 5)", None, 10).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].ts, "1700000000.000001");
    }

    #[test]
    fn test_search_returns_snippet() {
        let store = Store::open_in_memory().unwrap();
        insert_channel(&store, "C001");

        insert_message(&store, "C001", "1700000000.000001", "hello world", "hello beautiful world");

        let results = store.search_messages("beautiful", None, 10).unwrap();
        assert_eq!(results.len(), 1);
        // snippet should contain the marker delimiters around "beautiful"
        assert!(
            results[0].snippet.contains(">>>beautiful<<<"),
            "snippet was: {}",
            results[0].snippet
        );
    }

    #[test]
    fn test_search_returns_rank() {
        let store = Store::open_in_memory().unwrap();
        insert_channel(&store, "C001");

        insert_message(&store, "C001", "1700000000.000001", "deploy deploy deploy", "deploy deploy deploy");
        insert_message(&store, "C001", "1700000000.000002", "deploy once", "deploy once");

        let results = store.search_messages("deploy", None, 10).unwrap();
        assert_eq!(results.len(), 2);
        // BM25 rank values are negative (lower = better match)
        // The message with more "deploy" occurrences should rank better (lower rank value)
        assert!(results[0].rank <= results[1].rank);
    }

    #[test]
    fn test_search_returns_thread_ts() {
        let store = Store::open_in_memory().unwrap();
        insert_channel(&store, "C001");

        // Parent message
        insert_message(&store, "C001", "1700000000.000001", "parent message", "parent message");
        // Threaded reply
        insert_threaded_message(
            &store,
            "C001",
            "1700000000.000002",
            "1700000000.000001",
            "threaded reply about deploy",
            "threaded reply about deploy",
        );

        let results = store.search_messages("deploy", None, 10).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].thread_ts.as_deref(), Some("1700000000.000001"));
    }

    #[test]
    fn test_search_excludes_deleted_messages() {
        let store = Store::open_in_memory().unwrap();
        insert_channel(&store, "C001");

        insert_message(&store, "C001", "1700000000.000001", "keep this message", "keep this message");
        insert_message(&store, "C001", "1700000000.000002", "delete this message", "delete this message");

        store.soft_delete_message("C001", "1700000000.000002").unwrap();

        // FTS still has the entry, but the join filters deleted messages
        let results = store.search_messages("message", None, 10).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].ts, "1700000000.000001");
    }

    #[test]
    fn test_search_no_results() {
        let store = Store::open_in_memory().unwrap();
        insert_channel(&store, "C001");

        insert_message(&store, "C001", "1700000000.000001", "hello world", "hello world");

        let results = store.search_messages("nonexistent", None, 10).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_search_with_highlight_basic() {
        let store = Store::open_in_memory().unwrap();
        insert_channel(&store, "C001");

        insert_message(&store, "C001", "1700000000.000001", "hello world", "hello beautiful world");

        let results = store
            .search_with_highlight("beautiful", None, 10, "<b>", "</b>")
            .unwrap();
        assert_eq!(results.len(), 1);
        assert!(
            results[0].snippet.contains("<b>beautiful</b>"),
            "snippet was: {}",
            results[0].snippet
        );
    }

    #[test]
    fn test_search_with_highlight_channel_filter() {
        let store = Store::open_in_memory().unwrap();
        insert_channel(&store, "C001");
        insert_channel(&store, "C002");

        insert_message(&store, "C001", "1700000000.000001", "important update", "important update");
        insert_message(&store, "C002", "1700000000.000002", "important notice", "important notice");

        let results = store
            .search_with_highlight("important", Some("C001"), 10, "**", "**")
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].channel_id, "C001");
        assert!(
            results[0].snippet.contains("**important**"),
            "snippet was: {}",
            results[0].snippet
        );
    }

    #[test]
    fn test_search_with_highlight_custom_markers() {
        let store = Store::open_in_memory().unwrap();
        insert_channel(&store, "C001");

        insert_message(&store, "C001", "1700000000.000001", "search term here", "search term here");

        let results = store
            .search_with_highlight("term", None, 10, "[[", "]]")
            .unwrap();
        assert_eq!(results.len(), 1);
        assert!(
            results[0].snippet.contains("[[term]]"),
            "snippet was: {}",
            results[0].snippet
        );
    }

    #[test]
    fn test_search_after_message_update() {
        let store = Store::open_in_memory().unwrap();
        insert_channel(&store, "C001");

        insert_message(&store, "C001", "1700000000.000001", "original text", "original text");

        // Verify original is searchable
        let results = store.search_messages("original", None, 10).unwrap();
        assert_eq!(results.len(), 1);

        // Update the message
        store
            .mark_edited("C001", "1700000000.000001", "updated text", Some("updated text"))
            .unwrap();

        // Old text should no longer match
        let results = store.search_messages("original", None, 10).unwrap();
        assert_eq!(results.len(), 0);

        // New text should match
        let results = store.search_messages("updated", None, 10).unwrap();
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_search_after_message_hard_delete() {
        let store = Store::open_in_memory().unwrap();
        insert_channel(&store, "C001");

        insert_message(&store, "C001", "1700000000.000001", "ephemeral message", "ephemeral message");

        let results = store.search_messages("ephemeral", None, 10).unwrap();
        assert_eq!(results.len(), 1);

        store.delete_message("C001", "1700000000.000001").unwrap();

        let results = store.search_messages("ephemeral", None, 10).unwrap();
        assert_eq!(results.len(), 0);
    }

    #[test]
    fn test_search_cross_channel() {
        let store = Store::open_in_memory().unwrap();
        insert_channel(&store, "C001");
        insert_channel(&store, "C002");
        insert_channel(&store, "C003");

        insert_message(&store, "C001", "1700000000.000001", "deploy to production", "deploy to production");
        insert_message(&store, "C002", "1700000000.000002", "deploy rollback", "deploy rollback");
        insert_message(&store, "C003", "1700000000.000003", "unrelated topic", "unrelated topic");

        let results = store.search_messages("deploy", None, 10).unwrap();
        assert_eq!(results.len(), 2);

        let channels: Vec<&str> = results.iter().map(|r| r.channel_id.as_str()).collect();
        assert!(channels.contains(&"C001"));
        assert!(channels.contains(&"C002"));
    }

    #[test]
    fn test_build_snippet_highlights_term() {
        let snippet = build_snippet("hello beautiful world", "beautiful", 32, ">>>", "<<<");
        assert!(
            snippet.contains(">>>beautiful<<<"),
            "snippet was: {}",
            snippet
        );
    }

    #[test]
    fn test_build_snippet_prefix_query() {
        let snippet = build_snippet("deployment was successful", "deploy*", 32, "[", "]");
        assert!(
            snippet.contains("[deployment]"),
            "snippet was: {}",
            snippet
        );
    }

    #[test]
    fn test_build_snippet_no_match() {
        let snippet = build_snippet("hello world", "missing", 32, ">>>", "<<<");
        assert_eq!(snippet, "hello world");
    }

    #[test]
    fn test_build_snippet_long_text_truncates() {
        let long_text = (0..50).map(|i| format!("word{}", i)).collect::<Vec<_>>().join(" ");
        let snippet = build_snippet(&long_text, "word25", 10, "[", "]");
        // Should contain the match and be shorter than the full text
        assert!(snippet.contains("[word25]"), "snippet was: {}", snippet);
        // Should have ellipsis indicating truncation
        assert!(snippet.contains("..."), "snippet was: {}", snippet);
    }
}
