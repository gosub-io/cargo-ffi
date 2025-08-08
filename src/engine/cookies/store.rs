mod json;
mod sqlite;

use crate::engine::cookies::cookies::CookieJarHandle;
use crate::engine::cookies::cookie_jar::DefaultCookieJar;
use crate::engine::zone::ZoneId;

// Cookie store exports
pub use json::JsonCookieStore;
pub use sqlite::SqliteCookieStore;

/// A cookie store allows to persist multiple cookie jars for different zones.
#[allow(unused)]
pub trait CookieStore: Send + Sync {
    /// Requests a jar for a given zone
    fn get_jar(&self, zone_id: ZoneId) -> Option<CookieJarHandle>;
    /// Persists the jar for the given zone based on the snapshot
    fn persist_zone_from_snapshot(&self, zone_id: ZoneId, snapshot: &DefaultCookieJar);
    /// Removes the jar for that zone from the store
    fn remove_zone(&self, zone_id: ZoneId);
    /// Persist all jars in the store
    fn persist_all(&self);
}