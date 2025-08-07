pub mod cookie_jar;
pub mod persistent_cookie_jar;
pub mod cookie_store;

use std::any::Any;
use http::HeaderMap;
use serde::{Deserialize, Serialize};
use url::Url;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CookieEntry {
    pub name: String,                   // Cookie name
    pub value: String,                  // Actual value
    pub path: Option<String>,           // Path (if available)
    pub domain: Option<String>,         // Domain (if available)
    pub secure: bool,                   // Available on https only
    pub expires: Option<String>,        // ISO8601 or timestamp for expiry (if any)
    pub same_site: Option<String>,
    pub http_only: bool,
}


// A cookie jar keeps the cookies for one single zone.
pub trait CookieJar: Send + Sync {
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;

    // Store the cookies found int he headers given into the jar
    fn store_response_cookies(&mut self, url: &Url, headers: &HeaderMap);

    // Returns the cookies to be added to a request based on a specific URL
    fn get_request_cookies(&self, url: &Url) -> Option<String>;

    // Clear the cookie jar
    fn clear(&mut self);

    // Retrieve all cookies from the jar
    fn get_all_cookies(&self) -> Vec<(Url, String)>;

    // REmove specific cookie name for an URL
    fn remove_cookie(&mut self, url: &Url, cookie_name: &str);

    // Remove all cookies for an url
    fn remove_cookies_for_url(&mut self, url: &Url);
}
