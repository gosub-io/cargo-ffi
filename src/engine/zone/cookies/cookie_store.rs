mod json;
mod sqlite;

use std::sync::{Arc, RwLock};
use crate::zone::cookies::CookieJar;
use crate::zone::zone::ZoneId;

pub use json::JsonCookieStore;
pub use sqlite::SqliteCookieStore;

// A cookie store allows to persist multiple cookie jars for different zones.
pub trait CookieStore: Send + Sync {
    fn get_jar(&self, zone_id: ZoneId) -> Option<Arc<RwLock<dyn CookieJar + Send + Sync>>>;
    fn persist_zone(&self, zone_id: ZoneId);
    fn remove_zone(&self, zone_id: ZoneId);
    fn persist_all(&self);
}