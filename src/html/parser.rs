use std::io;

use once_cell::sync::Lazy;
use regex::Regex;
use tokio::io::{AsyncRead, AsyncReadExt, ReadBuf};
use tokio_util::sync::CancellationToken;
use url::Url;

/// What kind of resource we discovered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ResourceKind {
    Stylesheet,
    Script,
    Image,
}

/// A hint to the engine/IO layer that a subresource should be fetched.
#[derive(Debug, Clone)]
pub struct ResourceHint {
    pub url: Url,                 // fully resolved
    pub kind: ResourceKind,
    pub rel: Option<String>,      // e.g., "stylesheet"
    pub from_attr: &'static str,  // e.g., "href" or "src"
}

/// The "document" we "parsed".
#[derive(Debug, Clone)]
pub struct DummyDocument {
    pub final_url: Url,
    pub title: Option<String>,
    /// Whole HTML as UTF-8 (best-effort).
    pub raw_html: String,
}

/// Error type for this dummy parser.
#[derive(thiserror::Error, Debug)]
pub enum DocumentError {
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    #[error("URL error: {0}")]
    Url(#[from] url::ParseError),

    #[error("Cancelled")]
    Cancelled,
}

/// Configuration knobs (can wire these into your runtime config later).
#[derive(Debug, Clone)]
pub struct DummyHtml5Config {
    /// Max bytes to buffer from the stream. We read the entire stream up to this limit.
    pub max_bytes: usize,
}

impl Default for DummyHtml5Config {
    fn default() -> Self {
        Self { max_bytes: 1 * 1024 * 1024 } // 1 MiB
    }
}

/// Main entry point: read stream, synthesize a doc, and report discovered subresources.
///
/// - `base_url`: used to resolve relative URLs.
/// - `reader`: the response body stream (already after UA has chosen Render).
/// - `cancel`: cancellation token (tab/nav cancellation).
/// - `on_discover`: callback invoked for each resource we find (enqueue fetch from here).
pub async fn parse_main_document_stream<R, F>(
    base_url: Url,
    mut reader: R,
    cancel: CancellationToken,
    cfg: DummyHtml5Config,
    mut on_discover: F,
) -> Result<DummyDocument, DocumentError>
where
    R: AsyncRead + Unpin + Send + 'static,
    F: FnMut(ResourceHint) + Send,
{
    // Read the stream into a bounded buffer; bail if cancelled.
    let mut buf = Vec::with_capacity(32 * 1024);
    let mut tmp = [0u8; 16 * 1024];

    loop {
        if cancel.is_cancelled() {
            return Err(DocumentError::Cancelled);
        }
        let n = reader.read(&mut tmp).await?;
        if n == 0 {
            break;
        }
        let remaining = cfg
            .max_bytes
            .saturating_sub(buf.len())
            .min(n);
        if remaining > 0 {
            buf.extend_from_slice(&tmp[..remaining]);
        }
        // If we hit the cap, we still drain the stream to EOF quickly
        // to avoid keeping the connection open unnecessarily.
        if buf.len() >= cfg.max_bytes {
            // Drain (non-blocking-ish) without growing memory
            // We don't strictly need to, but it's polite to the transport.
            let mut drain = [0u8; 16 * 1024];
            while reader.read(&mut drain).await? != 0 {
                if cancel.is_cancelled() {
                    return Err(DocumentError::Cancelled);
                }
            }
            break;
        }
    }

    // Best-effort UTF-8 for discovery and title extraction.
    let html = String::from_utf8_lossy(&buf).into_owned();

    // Discover resources (css/js/img) and fire callbacks.
    for hint in discover_resources(&html, &base_url) {
        on_discover(hint);
    }

    // Synthesize a document (optionally extract <title>…</title>).
    let title = discover_title(&html);

    Ok(DummyDocument {
        final_url: base_url,
        title,
        raw_html: html,
    })
}

// ======== Forgiving resource discovery (regex-based) ========

static RE_LINK_STYLESHEET: Lazy<Regex> = Lazy::new(|| {
    // <link ... rel="stylesheet" ... href="...">
    // - allow single or double quotes
    // - allow attributes in any order
    Regex::new(
        r#"(?is)<\s*link\b[^>]*\brel\s*=\s*(['"])stylesheet\1[^>]*\bhref\s*=\s*(['"])(?P<href>[^"']+)\2[^>]*>"#
    ).unwrap()
});

static RE_SCRIPT_SRC: Lazy<Regex> = Lazy::new(|| {
    // <script ... src="...">
    Regex::new(
        r#"(?is)<\s*script\b[^>]*\bsrc\s*=\s*(['"])(?P<src>[^"']+)\1[^>]*>"#
    ).unwrap()
});

static RE_IMG_SRC: Lazy<Regex> = Lazy::new(|| {
    // <img ... src="...">
    Regex::new(
        r#"(?is)<\s*img\b[^>]*\bsrc\s*=\s*(['"])(?P<src>[^"']+)\1[^>]*>"#
    ).unwrap()
});

static RE_TITLE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?is)<\s*title\s*>\s*(?P<title>.*?)\s*<\s*/\s*title\s*>"#).unwrap()
});

fn discover_title(html: &str) -> Option<String> {
    RE_TITLE
        .captures(html)
        .and_then(|c| c.name("title").map(|m| m.as_str().trim().to_string()))
}

fn discover_resources(html: &str, base: &Url) -> Vec<ResourceHint> {
    let mut out = Vec::new();

    // Stylesheets
    for cap in RE_LINK_STYLESHEET.captures_iter(html) {
        if let Some(m) = cap.name("href") {
            if let Ok(u) = resolve(base, m.as_str()) {
                out.push(ResourceHint {
                    url: u,
                    kind: ResourceKind::Stylesheet,
                    rel: Some("stylesheet".to_string()),
                    from_attr: "href",
                });
            }
        }
    }

    // Scripts
    for cap in RE_SCRIPT_SRC.captures_iter(html) {
        if let Some(m) = cap.name("src") {
            if let Ok(u) = resolve(base, m.as_str()) {
                out.push(ResourceHint {
                    url: u,
                    kind: ResourceKind::Script,
                    rel: None,
                    from_attr: "src",
                });
            }
        }
    }

    // Images
    for cap in RE_IMG_SRC.captures_iter(html) {
        if let Some(m) = cap.name("src") {
            if let Ok(u) = resolve(base, m.as_str()) {
                out.push(ResourceHint {
                    url: u,
                    kind: ResourceKind::Image,
                    rel: None,
                    from_attr: "src",
                });
            }
        }
    }

    out
}

fn resolve(base: &Url, candidate: &str) -> Result<Url, url::ParseError> {
    // Tolerate whitespace, no-op fragments, etc.
    let trimmed = candidate.trim();
    if trimmed.is_empty() {
        return Err(url::ParseError::EmptyHost);
    }
    base.join(trimmed)
}