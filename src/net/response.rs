use http::HeaderMap;

// Simple structure for HTTP responses
#[derive(Debug)]
pub struct Response {
    pub url: url::Url,              // Url of the response
    pub status: u16,                // HTTP status code
    pub status_text: String,        // HTTP status text
    pub headers: HeaderMap,         // Headers of the response
    pub body: Vec<u8>,              // Body of the response
}