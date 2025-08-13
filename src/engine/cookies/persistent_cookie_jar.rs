use crate::engine::cookies::{CookieJar, CookieJarHandle, CookieStoreHandle};
use url::Url;
use http::HeaderMap;
use crate::engine::cookies::cookie_jar::DefaultCookieJar;
use crate::engine::zone::ZoneId;

/// Wraps a real `CookieJar` and triggers persistence after changes
pub struct PersistentCookieJar {
    /// Zone ID of the cookie jar
    zone_id: ZoneId,
    /// Inner cookier jar that holds the actual cookie data
    pub inner: CookieJarHandle,
    /// Handle to the cocokie store to persist data
    store: CookieStoreHandle,
}

impl PersistentCookieJar {
    pub fn new(
        zone_id: ZoneId,
        jar: CookieJarHandle,
        store: CookieStoreHandle,
    ) -> Self {
        dbg!("Creating PersistentCookieJar for zone: {}", zone_id);

        Self { zone_id, inner: jar, store }
    }

    fn persist(&self) {
        dbg!("Persisting cookies for zone: {}", self.zone_id);

        // Create a snapshot of the current state of the cookie jar. This is what we will store with "persist()"
        let snapshot = {
            let inner = self.inner.read().unwrap();
            let jar = inner.as_any()
                .downcast_ref::<DefaultCookieJar>()
                .expect("inner must be DefaultCookieJar");
            jar.clone()
        };

        self.store.persist_zone_from_snapshot(self.zone_id, &snapshot);
    }
}

impl CookieJar for PersistentCookieJar {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn store_response_cookies(&mut self, url: &Url, headers: &HeaderMap) {
        dbg!("Storing response cookies for URL");
        dbg!(&url);
        dbg!(&headers);
        {
            let mut inner = self.inner.write().expect("Failed to acquire write lock on cookie jar");
            inner.store_response_cookies(url, headers);
        }

        self.persist();
    }

    fn get_request_cookies(&self, url: &Url) -> Option<String> {
        dbg!("Retrieving request cookies for URL: {}", url);

        let inner = self.inner.read().expect("Failed to acquire read lock on cookie jar");
        inner.get_request_cookies(url)
    }

    fn clear(&mut self) {
        dbg!("Clearing cookie jar for zone: {}", self.zone_id);

        {
            let mut inner = self.inner.write().expect("Failed to acquire write lock on cookie jar");
            inner.clear();
        }
        self.persist();
    }

    fn get_all_cookies(&self) -> Vec<(Url, String)> {
        dbg!("Retrieving all cookies for zone: {}", self.zone_id);

        let inner = self.inner.read().expect("Failed to acquire read lock on cookie jar");
        inner.get_all_cookies()
    }

    fn remove_cookie(&mut self, url: &Url, cookie_name: &str) {
        dbg!("Removing cookie '{}' for URL: {}", cookie_name, url);

        {
            let mut inner = self.inner.write().expect("Failed to acquire write lock on cookie jar");
            inner.remove_cookie(url, cookie_name);
        }
        self.persist();
    }

    fn remove_cookies_for_url(&mut self, url: &Url) {
        dbg!("Removing all cookies for URL: {}", url);

        {
            let mut inner = self.inner.write().expect("Failed to acquire write lock on cookie jar");
            inner.remove_cookies_for_url(url);
        }
        self.persist();
    }
}
