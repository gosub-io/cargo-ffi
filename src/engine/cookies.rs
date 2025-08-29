//! Cookie management system for the Gosub engine.
//!
//! This module provides the core traits and implementations for storing,
//! retrieving, and persisting HTTP cookies. It defines the main
//! [`CookieJar`] interface used by zones, various backend [`CookieStore`]
//! implementations, and persistent wrappers.
//!
//! # Overview
//!
//! - [`Cookie`] — Represents a single HTTP cookie (name, value, domain, path, expiry, etc.).
//! - [`CookieJar`] — In-memory cookie jar with full RFC 6265 handling.
//! - [`DefaultCookieJar`] — The engine's default [`CookieJar`] implementation.
//! - [`PersistentCookieJar`] — A [`CookieJar`] wrapper that persists its state
//!   to a [`CookieStore`] backend.
//! - [`CookieStore`] — Abstract trait for reading/writing cookies to persistent storage.
//! - [`JsonCookieStore`] — Simple JSON-based cookie store (human-readable, easy to debug).
//! - [`SqliteCookieStore`] — SQLite-based cookie store (efficient for large sets).
//!
//! Internally, `CookieJarHandle` and `CookieStoreHandle` are reference-counted handles
//! used to share jars and stores safely between threads/zones.
//!
//! # Integration with Zones
//!
//! Each [`Zone`](crate::zone::Zone) has its own [`CookieJar`]. You can provide a
//! [`PersistentCookieJar`] to store cookies between sessions, or use an in-memory
//! [`DefaultCookieJar`] for ephemeral cookies (e.g., private mode).
//!
//! Example of attaching a persistent SQLite cookie jar to a zone:
//!
//! ```no_run
//! use std::sync::{Arc, RwLock};
//! use gosub_engine::GosubEngine;
//! use gosub_engine::zone::ZoneId;
//! use gosub_engine::cookies::{CookieJarHandle, SqliteCookieStore, PersistentCookieJar, DefaultCookieJar};
//!
//! let backend = gosub_engine::render::backends::null::NullBackend::new().expect("null renderer cannot be created (!?)");
//! let mut engine = GosubEngine::new(None, Box::new(backend));
//!
//! // Open or create the SQLite cookie store
//! let cookie_store = SqliteCookieStore::new("cookies.db".into());
//!
//! // Create the zone with a persistent cookie jar taken from the sqlite cookie store
//! let zone_id = engine.zone_builder().cookie_store(cookie_store).create().unwrap();
//!
//! ```
//!
//! # Choosing a backend
//!
//! - [`JsonCookieStore`]: Easy to inspect and debug, slower for large volumes.
//! - [`SqliteCookieStore`]: Scales well to thousands of cookies, supports indexing,
//!   suitable for long-lived profiles.
//!
//! # See also
//!
//! - [`Zone`](crate::zone::Zone) for how cookie jars are stored and used.
//! - [`CookieStore`] for implementing your own storage backend.
//! - RFC 6265 for HTTP cookie semantics.
//!
//! # Available types
//!
//! - [`Cookie`]
//! - [`CookieJar`], [`DefaultCookieJar`], [`PersistentCookieJar`]
//! - [`CookieStore`], [`JsonCookieStore`], [`SqliteCookieStore`]
//!
mod cookie_jar;
mod cookies;
mod persistent_cookie_jar;
mod store;

pub use cookies::Cookie;
pub use cookies::CookieJarHandle;
pub(crate) use cookies::CookieStoreHandle;

pub use cookie_jar::CookieJar;
pub use cookie_jar::DefaultCookieJar;
pub use persistent_cookie_jar::PersistentCookieJar;

pub use store::CookieStore;
pub use store::JsonCookieStore;
pub use store::SqliteCookieStore;
pub use store::InMemoryCookieStore;
