use std::sync::{Arc, RwLock};
use serde::{Deserialize, Serialize};
use crate::engine::cookies::store::CookieStore;
use crate::engine::cookies::CookieJar;

/// A handle to a cookie jar trait.
pub type CookieJarHandle = Arc<RwLock<dyn CookieJar + Send + Sync>>;
/// An handle to a cookie store trait
pub type CookieStoreHandle = Arc<dyn CookieStore + Send + Sync>;

/// A cookie structure that holds all information needed inside a cookie
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cookie {
    /// Cookie name
    pub name: String,
    /// Actual value
    pub value: String,
    /// Path (if available)
    pub path: Option<String>,
    /// Domain (if available)
    pub domain: Option<String>,
    /// Available on https only
    pub secure: bool,
    /// ISO8601 or timestamp for expiry (if any)
    pub expires: Option<String>,
    /// SameSite policy (e.g., "Strict", "Lax", "None")
    pub same_site: Option<String>,
    /// When true, the cookie cannot be accessed via JavaScript
    pub http_only: bool,
}
