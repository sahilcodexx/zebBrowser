//! URL detection and search query handling.

/// A parsed user input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NavInput {
    Url(String),
    Search(String),
}

const SCHEMES: &[&str] = &["http://", "https://", "file://", "ftp://", "about:"];

/// Classify the user input. If it already looks like a URL it stays a URL;
/// otherwise it is treated as a search query.
pub fn parse(input: &str) -> NavInput {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return NavInput::Search(String::new());
    }

    // Already has a scheme.
    if SCHEMES.iter().any(|s| trimmed.starts_with(s)) {
        return NavInput::Url(trimmed.to_string());
    }

    // localhost / IP-like forms are URLs.
    if trimmed.eq_ignore_ascii_case("localhost")
        || trimmed.starts_with("localhost/")
        || trimmed.starts_with("127.")
        || trimmed.starts_with("192.168.")
        || trimmed.starts_with("10.")
    {
        return NavInput::Url(format!("http://{}", trimmed));
    }

    // Looks like a domain: contains a dot and only domain-safe chars.
    if looks_like_domain(trimmed) {
        return NavInput::Url(format!("https://{}", trimmed));
    }

    NavInput::Search(trimmed.to_string())
}

fn looks_like_domain(s: &str) -> bool {
    if !s.contains('.') {
        return false;
    }
    if s.chars().any(|c| c.is_whitespace()) {
        return false;
    }
    // Must have at least one character before and after the first dot,
    // and the TLD must be at least 2 letters.
    let mut parts = s.split('.');
    let first = parts.next().unwrap_or("");
    let last = parts.last().unwrap_or("");
    if first.is_empty() || last.len() < 2 {
        return false;
    }
    if !last.chars().all(|c| c.is_ascii_alphabetic()) {
        return false;
    }
    // All parts non-empty.
    let all_non_empty: bool = s.split('.').all(|p| !p.is_empty());
    all_non_empty
}

/// Build the final URI from a `NavInput`.
pub fn resolve(input: &NavInput) -> String {
    match input {
        NavInput::Url(u) => u.clone(),
        NavInput::Search(q) => {
            let encoded = simple_url_encode(q);
            crate::config::SEARCH_ENGINE_URL.replace("{query}", &encoded)
        }
    }
}

fn simple_url_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_urls() {
        assert_eq!(
            parse("https://example.com"),
            NavInput::Url("https://example.com".into())
        );
        assert_eq!(
            parse("http://example.com"),
            NavInput::Url("http://example.com".into())
        );
        assert_eq!(
            parse("example.com"),
            NavInput::Url("https://example.com".into())
        );
        assert_eq!(
            parse("www.example.com"),
            NavInput::Url("https://www.example.com".into())
        );
        assert_eq!(parse("localhost"), NavInput::Url("http://localhost".into()));
        assert_eq!(
            parse("127.0.0.1:8080"),
            NavInput::Url("http://127.0.0.1:8080".into())
        );
    }

    #[test]
    fn parses_searches() {
        assert_eq!(
            parse("rust gtk tutorial"),
            NavInput::Search("rust gtk tutorial".into())
        );
        assert_eq!(parse("foo"), NavInput::Search("foo".into()));
    }

    #[test]
    fn resolves_search() {
        let r = resolve(&NavInput::Search("hello world".into()));
        assert!(r.starts_with("https://duckduckgo.com/?q="));
        assert!(r.contains("hello+world"));
    }
}
