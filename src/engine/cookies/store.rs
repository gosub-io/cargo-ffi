//! Cookie store infrastructure.
//!
//! A **cookie store** is a provisioner and persistence layer for per-zone cookie jars.
//! - A **Zone** only *holds a [`CookieJarHandle`]*, never a store.
//! - A **CookieStore** can *mint* a jar for a given [`ZoneId`] and optionally
//!   persist/flush all zone jars in one place (e.g., a single JSON file or SQLite DB).
//!
//! Typical usage patterns:
//! - Set an engine-wide default store so new zones automatically get a jar from it.
//! - Or, during zone building, pass a specific store to mint that zone’s jar.
//! - For ephemeral/private zones, skip the store and use an in-memory jar.
//!
//! This module exports two reference implementations:
//! - [`JsonCookieStore`]: file-backed JSON store (good for simple setups).
//! - [`SqliteCookieStore`]: SQLite-backed store (good for concurrency and scale).
//!
//! ## Design notes
//! - Stores are **not** kept in zones; they are *only used at build time* to obtain a jar.
//! - Implementations should be `Send + Sync` and safe for concurrent access.
//! - `CookieStore::jar_for(zone_id)` should return the *same logical jar instance* for a zone for
//!   the lifetime of the store, so all handles observe consistent state.
//!
//! ## Example: per-zone store override
//! ```rust,no_run
//!
//! use gosub_engine::GosubEngine;
//! use gosub_engine::cookies::{JsonCookieStore, SqliteCookieStore};
//!
//! let backend = gosub_engine::render::backends::null::NullBackend::new().expect("null renderer cannot be created (!?)");
//! let mut engine = GosubEngine::new(None, Box::new(backend));
//!
//! let cookie_store = SqliteCookieStore::new("cookies.db".into());
//! let zone_id = engine.zone_builder().cookie_store(cookie_store).create().unwrap();
//!
//! let cookie_store = JsonCookieStore::new("private-cookies.json".into());
//! let private_zone_id = engine.zone_builder().cookie_store(cookie_store).create().unwrap();
//! ```
mod json;
mod sqlite;

use crate::engine::cookies::cookie_jar::DefaultCookieJar;
use crate::engine::cookies::cookies::CookieJarHandle;
use crate::engine::zone::ZoneId;

/// File-backed JSON cookie store (one file for all zones).
pub use json::JsonCookieStore;
/// SQLite-backed cookie store (one database for all zones).
pub use sqlite::SqliteCookieStore;

/// A cookie **store** mints per-zone cookie **jars** and (optionally) persists them.
///
/// Zones never store a `CookieStore`; they only hold a [`CookieJarHandle`].
/// The store exists to:
/// 1) provide the jar for a given [`ZoneId`], and
/// 2) write/read cookie state to/from durable storage.
///
/// Implementations must be `Send + Sync` and safe for concurrent use.
pub trait CookieStore: Send + Sync {
    /// Returns (or creates and returns) the cookie jar handle for `zone_id`.
    ///
    /// ### Expectations
    /// - Should return the *same logical jar instance* for a given `zone_id`
    ///   across calls, so all holders observe consistent state.
    /// - May create the jar lazily on first request.
    /// - Return `None` if the store no longer manages this zone (e.g., after removal)
    ///   or if provisioning fails irrecoverably.
    fn jar_for(&self, zone_id: ZoneId) -> Option<CookieJarHandle>;

    /// Persists the cookie state for `zone_id` from a provided snapshot.
    ///
    /// This allows the engine to push the current in-memory state (captured in
    /// a [`DefaultCookieJar`] snapshot) into the store without requiring the store
    /// to hold a direct reference to the live jar.
    ///
    /// Implementations may choose to:
    /// - Replace the stored state, or
    /// - Merge it (e.g., last-write-wins), depending on policy.
    ///
    /// This should be **best-effort** and must not panic.
    fn persist_zone_from_snapshot(&self, zone_id: ZoneId, snapshot: &DefaultCookieJar);

    /// Removes all persisted cookie data for `zone_id` from the store.
    ///
    /// Implementations should also drop any internal cache for this zone so that
    /// subsequent calls to [`CookieStore::jar_for`] can recreate a fresh, empty jar (or return `None`).
    ///
    /// This operation should be **idempotent** and must not panic.
    fn remove_zone(&self, zone_id: ZoneId);

    /// Persists all known zone jars to durable storage.
    ///
    /// Called during graceful shutdown or at explicit flush points. Implementations
    /// should make a **best-effort** to write all dirty state and avoid panicking.
    fn persist_all(&self);
}
