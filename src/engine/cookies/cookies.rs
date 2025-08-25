//! Cookie core types.
//!
//! This module defines the **type-erased handles** used throughout the engine
//! and the serializable [`Cookie`] data structure.
//!
//! # Concurrency model
//! - [`CookieJarHandle`] is `Arc<RwLock<dyn CookieJar + Send + Sync>>`.
//!   - Callers take a **read lock** for non-mutating operations and a **write lock**
//!     for mutating operations on the underlying jar.
//! - [`CookieStoreHandle`] is `Arc<dyn CookieStore + Send + Sync>`.
//!   - Stores are expected to manage their **own internal synchronization** (e.g. via
//!     `parking_lot`, `Mutex`, connection pools, etc.). The trait methods take `&self`.
//!
//! # Typical usage
//! ```ignore
//! // Acquire cookies for a request
//! let jar = zone.cookie_jar(); // -> CookieJarHandle
//! let cookies_header = {
//!     let guard = jar.read().unwrap();
//!     guard.get_request_cookies(&url)
//! };
//!
//! // Store cookies from a response
//! {
//!     let mut guard = jar.write().unwrap();
//!     guard.store_response_cookies(&url, &headers);
//! }
//! ```
//!
//! The [`Cookie`] struct is used for persistence/inspection and can be (de)serialized
//! via `serde` to JSON or other formats.
//!
//! ```rust,no_run
//! use gosub_engine::cookies::Cookie;
//!
//! let c = Cookie {
//!     name: "session".into(),
//!     value: "abc123".into(),
//!     path: Some("/".into()),
//!     domain: Some("example.com".into()),
//!     secure: true,
//!     expires: Some("2025-12-31T23:59:59Z".into()), // ISO 8601 recommended
//!     same_site: Some("Lax".into()),                 // "Strict" | "Lax" | "None"
//!     http_only: true,
//! };
//! ```

use crate::engine::cookies::store::CookieStore;
use crate::engine::cookies::CookieJar;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, RwLock};

/// A handle to a cookie jar trait.
///
/// This is a reference-counted, read/write-locked pointer to a type-erased
/// [`CookieJar`]. Obtain a **read lock** for queries and a **write lock** for
/// mutations.
///
/// ### Example
/// ```ignore
/// let jar: CookieJarHandle = zone.cookie_jar();
/// {
///     let cookies = jar.read().unwrap().get_request_cookies(&url);
/// }
/// {
///     let mut guard = jar.write().unwrap();
///     guard.clear();
/// }
/// ```
pub type CookieJarHandle = Arc<RwLock<dyn CookieJar + Send + Sync>>;

/// A handle to a cookie store trait.
///
/// This is a reference-counted pointer to a type-erased [`CookieStore`].
/// Store implementations must be **`Send + Sync` and internally synchronized**,
/// since callers hold only `&self` when invoking trait methods.
///
/// Typical use is at **build/initialization time** to mint a per-zone jar.
pub type CookieStoreHandle = Arc<dyn CookieStore + Send + Sync>;

/// A cookie as stored/serialized by the engine.
///
/// This structure captures the essential attributes of an HTTP cookie and
/// is suitable for persistence (e.g., JSON, SQLite) via `serde`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cookie {
    /// Cookie name (case-sensitive).
    pub name: String,

    /// Raw cookie value (not URL-decoded).
    pub value: String,

    /// Path scoping (e.g., `"/"`). If `None`, path-matching follows RFC defaults.
    pub path: Option<String>,

    /// Domain scoping (host-only if `None`). When present, should be a registrable domain
    /// or subdomain (e.g., `"example.com"`).
    pub domain: Option<String>,

    /// If `true`, cookie is sent only over HTTPS.
    pub secure: bool,

    /// Expiration timestamp, if any.
    ///
    /// Prefer **ISO 8601** (`YYYY-MM-DDThh:mm:ssZ`) for portability.
    /// Session cookies have `None`.
    pub expires: Option<String>,

    /// SameSite policy (`"Strict"`, `"Lax"`, or `"None"`).
    ///
    /// `None` implies cross-site allowed (must also set `secure=true` in modern browsers).
    /// Consider modeling as an enum in the future.
    pub same_site: Option<String>,

    /// If `true`, cookie is blocked from access by client-side scripts (`document.cookie`).
    pub http_only: bool,
}
