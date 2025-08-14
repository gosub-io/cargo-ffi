//! Minimal HTTP response model.
//!
//! This struct represents a **fully buffered** HTTP response returned by the
//! network layer. It contains the final URL (after redirects, if the client
//! follows them), status code + reason, response headers, and the raw body bytes.
//!
//! ## Notes
//! - The body is stored as raw `Vec<u8>`. For text responses, convert with
//!   `String::from_utf8_lossy(&resp.body)` or similar. For JSON, parse with
//!   `serde_json::from_slice::<T>(&resp.body)`.
//! - `headers` is an `http::HeaderMap`, which is **case-insensitive** for
//!   header names.
//! - `status_text` is typically derived from the status code’s canonical
//!   reason phrase and may be `"Unknown"` for non-standard codes.
//!
use http::HeaderMap;

/// Simple structure for HTTP responses.
///
/// All fields reflect the **received** response as-is; no additional parsing
/// or transformation is performed by this type.
#[derive(Debug)]
pub struct Response {
    /// Final URL of the response (after redirects, if any).
    pub url: url::Url,

    /// Numeric HTTP status code (e.g., `200`, `404`).
    pub status: u16,

    /// Human-readable reason phrase (e.g., `"OK"`, `"Not Found"`).
    ///
    /// May be `"Unknown"` for non-standard codes.
    pub status_text: String,

    /// Response headers as a case-insensitive map.
    pub headers: HeaderMap,

    /// Raw response body bytes.
    ///
    /// Convert to text with `String::from_utf8_lossy`, or parse as binary/JSON
    /// depending on the `Content-Type`.
    pub body: Vec<u8>,
}