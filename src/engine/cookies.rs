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
//! let mut engine = GosubEngine::new(None);
//!
//! // Create the zone
//! let zone_id = engine.zone().id(ZoneId::new()).create().unwrap();
//!
//! // Open or create the SQLite cookie store
//! let store = SqliteCookieStore::new("cookies.db".into());
//! let inner_jar = DefaultCookieJar::new();
//! let arc_jar: CookieJarHandle = Arc::new(RwLock::new(inner_jar));
//!
//! // Wrap it in a persistent cookie jar
//! let jar = PersistentCookieJar::new(zone_id, arc_jar.clone(), store.clone());
//!
//! // Attach to the zone
//! let zone = engine.get_zone_mut(zone_id).unwrap();
//! zone.lock().unwrap().set_cookie_jar(jar);
//! ```
//!
//! For ephemeral sessions:
//!
//! ```no_run
//! use gosub_engine::cookies::DefaultCookieJar;
//! let jar = DefaultCookieJar::new();
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
mod cookies;
mod cookie_jar;
mod store;
mod persistent_cookie_jar;

pub use cookies::Cookie;
pub use cookies::CookieJarHandle;
pub(crate) use cookies::CookieStoreHandle;

pub use cookie_jar::CookieJar;
pub use cookie_jar::DefaultCookieJar;
pub use persistent_cookie_jar::PersistentCookieJar;

pub use store::CookieStore;
pub use store::JsonCookieStore;
pub use store::SqliteCookieStore;