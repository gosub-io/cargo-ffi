/// Mime type classification
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MimeKind {
    Html,
    Json,
    Image,
    Text,
    Binary,
}

// @TODO: ok for now.. we should use something like crate::mimetype_detector

pub fn classify_mime(header: Option<&str>, sniff: Option<&[u8]>) -> MimeKind {
    if let Some(ct) = header {
        let ct = ct.to_ascii_lowercase();
        if ct.starts_with("text/html") { return MimeKind::Html; }
        if ct.starts_with("application/json") { return MimeKind::Json; }
        if ct.starts_with("image/") { return MimeKind::Image; }
        if ct.starts_with("text/") { return MimeKind::Text; }
    }

    // fallback sniff
    if let Some(b) = sniff {
        if b.starts_with(b"{") || b.starts_with(b"[") { return MimeKind::Json; }
        if b.windows(6).any(|w| w.eq_ignore_ascii_case(b"<html>"))
            || b.windows(15).any(|w| w.eq_ignore_ascii_case(b"<!doctype html>")) {
            return MimeKind::Html;
        }
    }

    MimeKind::Binary
}

