//! Persistence-enabled cookie jar wrapper.
//!
//! `PersistentCookieJar` decorates a real [`CookieJar`] so that any **mutation**
//! to the jar is followed by a persistence step into a backing
//! [`CookieStoreHandle`] (e.g., JSON or SQLite).
//!
//! ## How it works
//! - The wrapper holds:
//!   - the zone identifier ([`ZoneId`]),
//!   - an inner jar ([`CookieJarHandle`]) where cookies actually live, and
//!   - a store handle ([`CookieStoreHandle`]) used to persist state.
//! - On mutating calls (`store_response_cookies`, `clear`, `remove_*`), it first
//!   writes to the inner jar, then calls [`persist`](#method.persist) to snapshot
//!   and flush the state to the store.
//! - Non-mutating calls (`get_request_cookies`, `get_all_cookies`) simply proxy
//!   to the inner jar.
//!
//! ## Concurrency
//! - The inner jar is an `Arc<RwLock<...>>`. Read operations take a read lock;
//!   write operations take a write lock.
//!
//! ## Snapshotting
//! - Persistence is performed using a **snapshot** of the current state. The
//!   snapshot is created by downcasting the inner jar to [`DefaultCookieJar`]
//!   and cloning it.
use crate::engine::cookies::cookie_jar::DefaultCookieJar;
use crate::engine::cookies::{CookieJar, CookieJarHandle, CookieStoreHandle};
use crate::engine::zone::ZoneId;
use http::HeaderMap;
use url::Url;

/// A `CookieJar` decorator that persists changes after each mutation.
///
/// This type is *transparent* for reads but *eagerly* persists after writes.
pub struct PersistentCookieJar {
    /// Zone ID associated with this jar (used to address the store).
    zone_id: ZoneId,
    /// Inner cookie jar that holds the actual cookie state.
    pub inner: CookieJarHandle,
    /// Handle to the cookie store responsible for persistence.
    store: CookieStoreHandle,
}

impl PersistentCookieJar {
    /// Creates a new persistence-enabled wrapper around an existing jar.
    ///
    /// The `store` will be used to persist snapshots after each mutation.
    pub fn new(zone_id: ZoneId, jar: CookieJarHandle, store: CookieStoreHandle) -> Self {
        Self {
            zone_id,
            inner: jar,
            store,
        }
    }

    /// Snapshots the inner jar and persists it to the backing store.
    ///
    /// # Panics
    /// Panics if the inner jar is not a [`DefaultCookieJar`], because the
    /// downcast is required to obtain a cloneable snapshot.
    fn persist(&self) {
        // Create a snapshot of the current state of the cookie jar. This is what we will store with "persist()"
        let snapshot = {
            let inner = self.inner.read().unwrap();
            let jar = inner
                .as_any()
                .downcast_ref::<DefaultCookieJar>()
                .expect("inner must be DefaultCookieJar");
            jar.clone()
        };

        self.store
            .persist_zone_from_snapshot(self.zone_id, &snapshot);
    }
}

impl CookieJar for PersistentCookieJar {
    /// Returns a type-erased reference to this jar (the wrapper itself).
    /// @TODO: check if we still need these.
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    /// Stores cookies from a response, then persists the updated state.
    fn store_response_cookies(&mut self, url: &Url, headers: &HeaderMap) {
        {
            let mut inner = self
                .inner
                .write()
                .expect("Failed to acquire write lock on cookie jar");
            inner.store_response_cookies(url, headers);
        }

        self.persist();
    }

    /// Returns the `Cookie` request header value for `url` without persisting.
    fn get_request_cookies(&self, url: &Url) -> Option<String> {
        let inner = self
            .inner
            .read()
            .expect("Failed to acquire read lock on cookie jar");
        inner.get_request_cookies(url)
    }

    /// Clears all cookies in the jar, then persists the updated state.
    fn clear(&mut self) {
        {
            let mut inner = self
                .inner
                .write()
                .expect("Failed to acquire write lock on cookie jar");
            inner.clear();
        }
        self.persist();
    }

    /// Returns all cookies (for debugging/inspection) without persisting.
    fn get_all_cookies(&self) -> Vec<(Url, String)> {
        let inner = self
            .inner
            .read()
            .expect("Failed to acquire read lock on cookie jar");
        inner.get_all_cookies()
    }

    /// Removes a single cookie by name for `url`, then persists the updated state.
    fn remove_cookie(&mut self, url: &Url, cookie_name: &str) {
        {
            let mut inner = self
                .inner
                .write()
                .expect("Failed to acquire write lock on cookie jar");
            inner.remove_cookie(url, cookie_name);
        }
        self.persist();
    }

    /// Removes all cookies for `url`, then persists the updated state.
    fn remove_cookies_for_url(&mut self, url: &Url) {
        {
            let mut inner = self
                .inner
                .write()
                .expect("Failed to acquire write lock on cookie jar");
            inner.remove_cookies_for_url(url);
        }
        self.persist();
    }
}
