//! Network fetch utility.
//!
//! This module provides a simple asynchronous [`fetch`] function that
//! performs an HTTP GET request for a given [`Url`] and returns a
//! [`Response`](crate::net::Response).
//!
//! Currently this is a minimal wrapper around [`reqwest`]:
//!
//! - Always performs a GET request.
//! - Downloads the full response body into memory (no streaming yet).
//! - Returns status code, status text, headers, final URL, and body bytes.
//!
//! # Example
//!
//! ```rust,no_run
//! use gosub_engine::net::fetch;
//! use url::Url;
//!
//! #[tokio::main]
//! async fn main() {
//!     let url = Url::parse("https://example.org").unwrap();
//!     match fetch(url).await {
//!         Ok(response) => {
//!             println!("Status: {} {}", response.status, response.status_text);
//!             println!("Body length: {}", response.body.len());
//!         }
//!         Err(e) => eprintln!("Fetch failed: {e:?}"),
//!     }
//! }
//! ```
//!
use crate::net::Response;
use url::Url;

/// Loads a URL using an HTTP GET request and returns the response.
///
/// This is a convenience wrapper around [`reqwest::Client`].
/// It performs the request, collects the headers, status code,
/// status text, final resolved URL, and the full response body.
///
/// # Arguments
///
/// * `url` - A fully parsed [`Url`] to fetch.
///
/// # Returns
///
/// On success, returns a [`Response`] containing:
/// - `url`: Final resolved URL (after redirects).
/// - `status`: Numeric HTTP status code.
/// - `status_text`: Human-readable reason phrase.
/// - `headers`: HTTP headers.
/// - `body`: Full response body as bytes.
///
/// # Errors
///
/// Returns a [`reqwest::Error`] if the request fails or the body
/// cannot be read.
///
/// # Notes
///
/// - This function does **not** yet support streaming bodies; the
///   entire response is buffered in memory.
/// - Only HTTP GET is supported. Other methods may be added later.
pub async fn fetch(url: Url) -> Result<Response, reqwest::Error> {
    let client = reqwest::Client::new();
    let res = client.get(url).send().await?;

    // Fetch results
    let final_url = res.url().clone();
    let status = res.status().as_u16();
    let status_text = res
        .status()
        .canonical_reason()
        .unwrap_or("Unknown")
        .to_string();
    let headers = res.headers().clone();

    // Fetch body. We don't do streaming yet
    let body = res.bytes().await?.to_vec();

    Ok(Response {
        url: final_url,
        status,
        status_text,
        headers,
        body,
    })
}
