use std::sync::Arc;
use anyhow::Result;
use crate::{TabId, ZoneId};
use super::types::{Origin, PartitionKey};

/// Object-safe key/value storage area (DOM’s Storage).
pub trait StorageArea: Send + Sync {
    fn get_item(&self, key: &str) -> Option<String>;
    fn set_item(&self, key: &str, value: &str) -> Result<()>;
    fn remove_item(&self, key: &str) -> Result<()>;
    fn clear(&self) -> Result<()>;
    fn len(&self) -> usize;
    fn keys(&self) -> Vec<String>;
}

/// Store for localStorage-like areas (shared per (zone, partition, origin)).
pub trait LocalStore: Send + Sync {
    fn area(&self, zone: ZoneId, part: &PartitionKey, origin: &Origin)
            -> Result<Arc<dyn StorageArea>>;
}

/// Store for sessionStorage-like areas (isolated per (zone, tab, partition, origin)).
pub trait SessionStore: Send + Sync {
    fn area(&self, zone: ZoneId, tab: TabId, part: &PartitionKey, origin: &Origin)
            -> Arc<dyn StorageArea>;
    fn drop_tab(&self, zone: ZoneId, tab: TabId);
}


#[cfg(test)]
mod tests {
    use crate::storage::InMemorySessionStore;
    use super::*;

    fn set(area: &Arc<dyn StorageArea>, k: &str, v: &str) {
        area.set_item(k, v).unwrap();
    }

    #[test]
    fn storagearea_basic_contract() {
        let zone = ZoneId::new();
        let tab = TabId::new();
        let part = PartitionKey::None;
        let origin = Origin("https://example.com".into());

        let store = InMemorySessionStore::new();
        let area = store.area(zone, tab, &part, &origin);

        // starts empty
        assert_eq!(area.len(), 0);
        assert!(area.get_item("missing").is_none());

        // set + get
        set(&area, "a", "1");
        set(&area, "b", "2");
        assert_eq!(area.len(), 2);
        assert_eq!(area.get_item("a").as_deref(), Some("1"));
        assert_eq!(area.get_item("b").as_deref(), Some("2"));

        // overwrite keeps len()
        set(&area, "a", "ONE");
        assert_eq!(area.len(), 2);
        assert_eq!(area.get_item("a").as_deref(), Some("ONE"));

        // remove
        area.remove_item("b").unwrap();
        assert_eq!(area.len(), 1);
        assert!(area.get_item("b").is_none());

        // clear
        area.clear().unwrap();
        assert_eq!(area.len(), 0);
    }
}