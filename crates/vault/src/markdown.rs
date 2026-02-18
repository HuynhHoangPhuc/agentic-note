use pulldown_cmark::{Event, Parser, Tag};
use regex::Regex;
use std::sync::LazyLock;

static WIKILINK_RE: LazyLock<Result<Regex, String>> = LazyLock::new(|| {
    Regex::new(r"\[\[([^\]]+)\]\]")
        .map_err(|e| format!("wikilink regex init failed: {e}"))
});

/// Extract `[[wikilink]]` targets from markdown body.
pub fn extract_wikilinks(body: &str) -> Vec<String> {
    let regex = match WIKILINK_RE.as_ref() {
        Ok(regex) => regex,
        Err(message) => {
            tracing::warn!(error = message, "wikilink regex unavailable");
            return Vec::new();
        }
    };

    regex
        .captures_iter(body)
        .map(|c| c[1].to_string())
        .collect()
}

/// Extract standard markdown link destinations using pulldown-cmark.
pub fn extract_markdown_links(body: &str) -> Vec<String> {
    let parser = Parser::new(body);
    let mut links = Vec::new();
    for event in parser {
        if let Event::Start(Tag::Link { dest_url, .. }) = event {
            links.push(dest_url.to_string());
        }
    }
    links
}

/// Extract all links (wikilinks + markdown links) from body.
pub fn extract_all_links(body: &str) -> Vec<String> {
    let mut links = extract_wikilinks(body);
    links.extend(extract_markdown_links(body));
    links
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wikilinks() {
        let body = "See [[Daily Notes]] and [[Project Plan]] for details.";
        let links = extract_wikilinks(body);
        assert_eq!(links, vec!["Daily Notes", "Project Plan"]);
    }

    #[test]
    fn test_markdown_links() {
        let body = "Check [docs](https://example.com) and [ref](./other.md).";
        let links = extract_markdown_links(body);
        assert_eq!(links, vec!["https://example.com", "./other.md"]);
    }

    #[test]
    fn test_all_links() {
        let body = "[[wiki]] and [md](./link.md)";
        let links = extract_all_links(body);
        assert_eq!(links, vec!["wiki", "./link.md"]);
    }
}
