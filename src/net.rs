//! Network utilities for making HTTP requests.
//!
//! This module provides a simple asynchronous [`crate::net::fetch`] function that
//! performs an HTTP GET request for a given [`Url`] and returns a
//! [`Response`].
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
mod fetch;
mod response;

pub use fetch::fetch;
pub use response::Response;
