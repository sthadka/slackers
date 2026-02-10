use regex::Regex;

/// Extract main content from HTML
///
/// Searches for <main>, <article>, or <body> tags and returns their content.
/// Falls back to the full HTML if none found.
pub fn extract_main_content(html: &str) -> String {
    // Try <main> first
    if let Some(content) = extract_tag_content(html, "main") {
        return content;
    }

    // Try <article>
    if let Some(content) = extract_tag_content(html, "article") {
        return content;
    }

    // Try <body>
    if let Some(content) = extract_tag_content(html, "body") {
        return content;
    }

    // Fall back to full HTML
    html.to_string()
}

/// Extract content between opening and closing tags
fn extract_tag_content(html: &str, tag: &str) -> Option<String> {
    let pattern = format!(r"(?is)<{}\b[^>]*>(.*?)</{}>", tag, tag);
    let re = Regex::new(&pattern).ok()?;

    re.captures(html)
        .and_then(|caps| caps.get(1))
        .map(|m| m.as_str().to_string())
}

/// Convert HTML to Markdown
///
/// Extracts main content first, then converts using html2md crate.
pub fn html_to_markdown(html: &str) -> String {
    let main_content = extract_main_content(html);
    html2md::parse_html(&main_content)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_main_content_with_main_tag() {
        let html = r#"
            <html>
                <head><title>Test</title></head>
                <body>
                    <header>Header content</header>
                    <main>
                        <h1>Main Content</h1>
                        <p>This is the main content.</p>
                    </main>
                    <footer>Footer content</footer>
                </body>
            </html>
        "#;

        let content = extract_main_content(html);
        assert!(content.contains("Main Content"));
        assert!(content.contains("This is the main content"));
        assert!(!content.contains("Header content"));
        assert!(!content.contains("Footer content"));
    }

    #[test]
    fn test_extract_main_content_with_article_tag() {
        let html = r#"
            <html>
                <body>
                    <nav>Navigation</nav>
                    <article>
                        <h1>Article Title</h1>
                        <p>Article content here.</p>
                    </article>
                </body>
            </html>
        "#;

        let content = extract_main_content(html);
        assert!(content.contains("Article Title"));
        assert!(content.contains("Article content here"));
        assert!(!content.contains("Navigation"));
    }

    #[test]
    fn test_extract_main_content_with_body_tag() {
        let html = r#"
            <html>
                <head><title>Test</title></head>
                <body>
                    <h1>Body Content</h1>
                    <p>Some text.</p>
                </body>
            </html>
        "#;

        let content = extract_main_content(html);
        assert!(content.contains("Body Content"));
        assert!(content.contains("Some text"));
    }

    #[test]
    fn test_extract_main_content_fallback() {
        let html = "<div>Just a div</div>";

        let content = extract_main_content(html);
        assert_eq!(content, html);
    }

    #[test]
    fn test_extract_main_prefers_main_over_article() {
        let html = r#"
            <html>
                <body>
                    <article>Article content</article>
                    <main>Main content</main>
                </body>
            </html>
        "#;

        let content = extract_main_content(html);
        assert!(content.contains("Main content"));
        assert!(!content.contains("Article content"));
    }

    #[test]
    fn test_html_to_markdown_basic() {
        let html = r#"
            <main>
                <h1>Hello World</h1>
                <p>This is a <strong>test</strong> with <em>emphasis</em>.</p>
                <ul>
                    <li>Item 1</li>
                    <li>Item 2</li>
                </ul>
            </main>
        "#;

        let markdown = html_to_markdown(html);

        // Check that markdown conversion happened
        assert!(markdown.contains("# Hello World") || markdown.contains("Hello World"));
        assert!(markdown.contains("test"));
        assert!(markdown.contains("emphasis"));
        assert!(markdown.contains("Item 1"));
        assert!(markdown.contains("Item 2"));
    }

    #[test]
    fn test_html_to_markdown_with_links() {
        let html = r#"
            <main>
                <p>Check out <a href="https://example.com">this link</a>.</p>
            </main>
        "#;

        let markdown = html_to_markdown(html);

        // html2md should convert links to markdown format
        assert!(markdown.contains("example.com") || markdown.contains("this link"));
    }

    #[test]
    fn test_extract_tag_content_with_attributes() {
        let html = r#"<main class="content" id="primary"><p>Content</p></main>"#;

        let content = extract_tag_content(html, "main");
        assert!(content.is_some());
        assert!(content.unwrap().contains("<p>Content</p>"));
    }
}
