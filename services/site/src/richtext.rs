//! Restricted rich text for committee page text blocks and project descriptions.

use std::collections::HashSet;

/// Longest description or body, counted in characters of visible text.
pub const MAX_TEXT_CHARS: usize = 4000;

/// Longest markup [`clean`] parses, in bytes. Parsing some malformed markup, such as thousands of unclosed tables, takes time that grows with the square of its length, so longer sources are refused before parsing. 4,000 characters with ordinary formatting stay far below it.
pub const MAX_SOURCE_BYTES: usize = 16 * MAX_TEXT_CHARS;
/// Most tags [`clean`] parses, for the same reason.
pub const MAX_SOURCE_TAGS: usize = MAX_TEXT_CHARS;

/// Markup [`clean`] refused to parse because it is too long or holds too many tags.
#[derive(Debug, PartialEq, Eq)]
pub struct TooMuchMarkup;

/// [`sanitize`] for markup someone sent: refuses sources over [`MAX_SOURCE_BYTES`] or [`MAX_SOURCE_TAGS`] without parsing them. CPU bound; handlers run it off the async runtime.
pub fn clean(raw: &str) -> Result<String, TooMuchMarkup> {
    let tags = raw.bytes().filter(|byte| *byte == b'<').count();
    if raw.len() > MAX_SOURCE_BYTES || tags > MAX_SOURCE_TAGS {
        return Err(TooMuchMarkup);
    }
    Ok(sanitize(raw))
}

/// Keeps only p, br, strong, em, a (https and mailto links), ul, ol and li. Links open in a new tab with `rel="noopener noreferrer nofollow"`. Everything else is removed, keeping its text.
pub fn sanitize(html: &str) -> String {
    let tags: HashSet<&str> = ["p", "br", "strong", "em", "a", "ul", "ol", "li"]
        .into_iter()
        .collect();
    let schemes: HashSet<&str> = ["https", "mailto"].into_iter().collect();
    let mut attributes = std::collections::HashMap::new();
    attributes.insert("a", ["href"].into_iter().collect::<HashSet<&str>>());
    ammonia::Builder::default()
        .tags(tags)
        .tag_attributes(attributes)
        .generic_attributes(HashSet::new())
        .url_schemes(schemes)
        .link_rel(Some("noopener noreferrer nofollow"))
        .set_tag_attribute_value("a", "target", "_blank")
        .url_relative(ammonia::UrlRelative::Deny)
        .strip_comments(true)
        .clean(html)
        .to_string()
}

/// The visible text of sanitized HTML, with tags removed and whitespace collapsed.
pub fn plain_text(html: &str) -> String {
    // Block boundaries become spaces so paragraphs don't run together.
    let mut spaced = html.to_owned();
    for tag in ["</p>", "</li>", "<br>", "<br/>", "<br />", "</ul>", "</ol>"] {
        spaced = spaced.replace(tag, &format!("{tag} "));
    }
    let stripped = ammonia::Builder::empty().clean(&spaced).to_string();
    let decoded = stripped
        .replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'");
    decoded.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The first `max` characters of the plain text, cut at a word boundary.
pub fn excerpt(html: &str, max: usize) -> String {
    let text = plain_text(html);
    if text.chars().count() <= max {
        return text;
    }
    let cut: String = text.chars().take(max).collect();
    let trimmed = cut.rsplit_once(' ').map_or(cut.as_str(), |(head, _)| head);
    format!("{}\u{2026}", trimmed.trim_end_matches([',', '.', ';', ':']))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_allowed_formatting() {
        let html = "<p>Hi <strong>there</strong> <em>you</em></p><ul><li>one</li></ul>";
        assert_eq!(sanitize(html), html);
    }

    #[test]
    fn removes_scripts_styles_and_unsafe_links() {
        let html = r#"<p onclick="x()" style="color:red">a<script>alert(1)</script><a href="javascript:alert(1)">b</a><img src=x onerror=y></p>"#;
        let clean = sanitize(html);
        assert!(!clean.contains("script"));
        assert!(!clean.contains("onclick"));
        assert!(!clean.contains("style"));
        assert!(!clean.contains("javascript"));
        assert!(!clean.contains("<img"));
    }

    #[test]
    fn links_open_safely() {
        let clean = sanitize(r#"<a href="https://scottylabs.org">site</a>"#);
        assert!(clean.contains(r#"href="https://scottylabs.org""#));
        assert!(clean.contains("noopener"));
        assert!(clean.contains(r#"target="_blank""#));
        assert!(!sanitize(r#"<a href="/relative">x</a>"#).contains("href"));
    }

    #[test]
    fn long_or_tag_heavy_sources_are_refused_before_parsing() {
        // Nested tables make the parser's work grow with the square of their number.
        let bomb = "<table>x".repeat(32_000);
        let started = std::time::Instant::now();
        assert_eq!(clean(&bomb), Err(TooMuchMarkup));
        assert_eq!(
            clean(&"<table>x".repeat(MAX_SOURCE_TAGS + 1)),
            Err(TooMuchMarkup)
        );
        assert_eq!(clean(&"x".repeat(MAX_SOURCE_BYTES + 1)), Err(TooMuchMarkup));
        assert!(started.elapsed() < std::time::Duration::from_millis(100));

        // The most markup allowed still parses quickly, and ordinary text is cleaned as before.
        let started = std::time::Instant::now();
        assert!(clean(&"<table>x".repeat(MAX_SOURCE_TAGS)).is_ok());
        assert!(
            started.elapsed() < std::time::Duration::from_secs(2),
            "{:?}",
            started.elapsed()
        );
        let text = format!("<p>{}</p>", "word ".repeat(800));
        assert_eq!(clean(&text), Ok(sanitize(&text)));
    }

    #[test]
    fn plain_text_and_excerpts() {
        assert_eq!(
            plain_text("<p>One <strong>two</strong></p><p>three</p>"),
            "One two three"
        );
        let long = format!("<p>{}</p>", "word ".repeat(100));
        let short = excerpt(&long, 30);
        assert!(short.chars().count() <= 31);
        assert!(short.ends_with('\u{2026}'));
    }
}
