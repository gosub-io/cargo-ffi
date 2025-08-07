use std::sync::{Arc, RwLock};
use crate::zone::cookies::CookieJar;
use url::Url;
use http::HeaderMap;
use crate::zone::cookies::cookie_store::CookieStore;
use crate::zone::zone::ZoneId;

/// Wraps a real `CookieJar` and triggers persistence after changes
pub struct PersistentCookieJar {
    zone_id: ZoneId,
    pub inner: Arc<RwLock<dyn CookieJar + Send + Sync>>,
    store: Arc<dyn CookieStore + Send + Sync>,
}

impl PersistentCookieJar {
    pub fn new(
        zone_id: ZoneId,
        jar: Arc<RwLock<dyn CookieJar + Send + Sync>>,
        store: Arc<dyn CookieStore + Send + Sync>,
    ) -> Self {
        dbg!("Creating PersistentCookieJar for zone: {}", zone_id);

        Self { zone_id, inner: jar, store }
    }

    fn persist(&self) {
        dbg!("Persisting cookies for zone: {}", self.zone_id);

        self.store.persist_zone(self.zone_id);
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
        dbg!("Storing response cookies for URL: {}", url);

        let mut inner = self.inner.write().expect("Failed to acquire write lock on cookie jar");
        inner.store_response_cookies(url, headers);
        self.persist();
    }

    fn get_request_cookies(&self, url: &Url) -> Option<String> {
        dbg!("Retrieving request cookies for URL: {}", url);

        let inner = self.inner.read().expect("Failed to acquire read lock on cookie jar");
        inner.get_request_cookies(url)
    }

    fn clear(&mut self) {
        dbg!("Clearing cookie jar for zone: {}", self.zone_id);

        let mut inner = self.inner.write().expect("Failed to acquire write lock on cookie jar");
        inner.clear();

        self.persist();
    }

    fn get_all_cookies(&self) -> Vec<(Url, String)> {
        dbg!("Retrieving all cookies for zone: {}", self.zone_id);

        let inner = self.inner.read().expect("Failed to acquire read lock on cookie jar");
        inner.get_all_cookies()
    }

    fn remove_cookie(&mut self, url: &Url, cookie_name: &str) {
        dbg!("Removing cookie '{}' for URL: {}", cookie_name, url);

        let mut inner = self.inner.write().expect("Failed to acquire write lock on cookie jar");
        inner.remove_cookie(url, cookie_name);
        self.persist();
    }

    fn remove_cookies_for_url(&mut self, url: &Url) {
        dbg!("Removing all cookies for URL: {}", url);

        let mut inner = self.inner.write().expect("Failed to acquire write lock on cookie jar");
        inner.remove_cookies_for_url(url);
        self.persist();
    }
}
