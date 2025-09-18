use http::HeaderMap;

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
