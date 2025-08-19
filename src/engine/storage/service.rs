use std::sync::{Arc, Mutex, mpsc};
use anyhow::Result;
use crate::tab::TabId;
use crate::zone::ZoneId;
use super::area::{LocalStore, SessionStore, StorageArea};
use super::event::{StorageEvent, StorageScope};
use super::types::PartitionKey;

/// A handle for receiving storage change notifications.
pub type Subscription = mpsc::Receiver<StorageEvent>;

/// Internal bus that fans out StorageEvent to subscribers.
#[derive(Default)]
struct StorageBus {
    subs: Mutex<Vec<mpsc::Sender<StorageEvent>>>,
}
impl StorageBus {
    fn subscribe(&self) -> Subscription {
        let (tx, rx) = mpsc::channel();
        self.subs.lock().unwrap().push(tx);
        rx
    }
    fn publish(&self, ev: StorageEvent) {
        let mut subs = self.subs.lock().unwrap();
        subs.retain(|tx| tx.send(ev.clone()).is_ok());
    }
}

/// Public service used by the engine/DOM to access storage and receive events.
pub struct StorageService {
    local: Arc<dyn LocalStore>,
    session: Arc<dyn SessionStore>,
    bus: Arc<StorageBus>,
}

impl StorageService {
    /// Create a new StorageService with the given local and session stores.
    pub fn new(local: Arc<dyn LocalStore>, session: Arc<dyn SessionStore>) -> Self {
        Self { local, session, bus: Arc::new(StorageBus::default()) }
    }

    /// Subscribe to storage changes (engine can dispatch DOM `storage` events).
    pub fn subscribe(&self) -> Subscription { self.bus.subscribe() }

    /// Get a localStorage area (wrapped to emit notifications).
    pub fn local_for(&self, zone: ZoneId, part: &PartitionKey, origin: &url::Origin) -> Result<Arc<dyn StorageArea>>
    {
        let inner = self.local.area(zone, part, origin)?;
        Ok(self.wrap_notifying(inner, zone, None, part.clone(), origin.clone(), StorageScope::Local))
    }

    /// Get a sessionStorage area (wrapped to emit notifications).
    pub fn session_for(&self, zone: ZoneId, tab: TabId, part: &PartitionKey, origin: &url::Origin) -> Arc<dyn StorageArea>
    {
        let inner = self.session.area(zone, tab, part, origin);
        self.wrap_notifying(inner, zone, Some(tab), part.clone(), origin.clone(), StorageScope::Session)
    }

    /// Drops a tab from sessionStorage.
    pub fn drop_tab(&self, zone: ZoneId, tab: TabId) {
        self.session.drop_tab(zone, tab);
    }

    fn wrap_notifying(
        &self,
        inner: Arc<dyn StorageArea>,
        zone: ZoneId,
        source_tab: Option<TabId>,
        partition: PartitionKey,
        origin: url::Origin,
        scope: StorageScope
    ) -> Arc<dyn StorageArea> {
        Arc::new(NotifyingArea {
            inner,
            zone,
            partition,
            origin,
            source_tab,
            bus: self.bus.clone(),
            scope,
        })
    }
}

/// Decorator that publishes StorageEvent on mutations.
struct NotifyingArea {
    inner: Arc<dyn StorageArea>,
    zone: ZoneId,
    partition: PartitionKey,
    origin: url::Origin,
    source_tab: Option<TabId>,
    bus: Arc<StorageBus>,
    scope: StorageScope,
}

impl StorageArea for NotifyingArea {
    fn get_item(&self, key: &str) -> Option<String> {
        self.inner.get_item(key)
    }
    fn set_item(&self, key: &str, value: &str) -> Result<()> {
        let old = self.inner.get_item(key);
        self.inner.set_item(key, value)?;
        self.bus.publish(StorageEvent {
            zone: self.zone,
            partition: self.partition.clone(),
            origin: self.origin.clone(),
            key: Some(key.to_string()),
            old_value: old,
            new_value: Some(value.to_string()),
            source_tab: self.source_tab,
            scope: self.scope,
        });
        Ok(())
    }
    fn remove_item(&self, key: &str) -> Result<()> {
        let old = self.inner.get_item(key);
        self.inner.remove_item(key)?;
        self.bus.publish(StorageEvent {
            zone: self.zone,
            partition: self.partition.clone(),
            origin: self.origin.clone(),
            key: Some(key.to_string()),
            old_value: old,
            new_value: None,
            source_tab: self.source_tab,
            scope: self.scope,
        });
        Ok(())
    }
    fn clear(&self) -> Result<()> {
        self.inner.clear()?;
        self.bus.publish(StorageEvent {
            zone: self.zone,
            partition: self.partition.clone(),
            origin: self.origin.clone(),
            key: None,
            old_value: None,
            new_value: None,
            source_tab: self.source_tab,
            scope: self.scope,
        });
        Ok(())
    }
    fn len(&self) -> usize { self.inner.len() }
    fn keys(&self) -> Vec<String> { self.inner.keys() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::time::Duration;

    use crate::zone::ZoneId;
    use crate::tab::TabId;
    use crate::storage::InMemorySessionStore;

    // --- Tiny in-memory StorageArea for tests ---
    #[derive(Default)]
    struct TestArea {
        map: Mutex<HashMap<String, String>>,
    }

    impl StorageArea for TestArea {
        fn get_item(&self, key: &str) -> Option<String> {
            self.map.lock().unwrap().get(key).cloned()
        }
        fn set_item(&self, key: &str, value: &str) -> Result<()> {
            self.map.lock().unwrap().insert(key.to_string(), value.to_string());
            Ok(())
        }
        fn remove_item(&self, key: &str) -> Result<()> {
            self.map.lock().unwrap().remove(key);
            Ok(())
        }
        fn clear(&self) -> Result<()> {
            self.map.lock().unwrap().clear();
            Ok(())
        }
        fn len(&self) -> usize {
            self.map.lock().unwrap().len()
        }
        fn keys(&self) -> Vec<String> {
            self.map.lock().unwrap().keys().cloned().collect()
        }
    }

    // --- In-memory LocalStore keyed by (zone, partition, origin) that shares areas ---
    #[derive(Default)]
    struct TestLocalStore {
        areas: Mutex<HashMap<(ZoneId, PartitionKey, url::Origin), Arc<dyn StorageArea>>>,
    }

    impl LocalStore for TestLocalStore {
        fn area(
            &self,
            zone: ZoneId,
            part: &PartitionKey,
            origin: &url::Origin
        ) -> Result<Arc<dyn StorageArea>> {
            let key = (zone, part.clone(), origin.clone());
            let mut g = self.areas.lock().unwrap();
            Ok(g.entry(key)
                .or_insert_with(|| Arc::new(TestArea::default()) as Arc<dyn StorageArea>)
                .clone())
        }
    }

    // --- helpers ---
    fn z() -> ZoneId { ZoneId::new() }
    fn t() -> TabId { TabId::new() }
    fn o(s: &str) -> url::Origin {
        let url = url::Url::parse(s).expect("valid URL");
        url.origin()
    }
    fn recv_ok(rx: &Subscription) -> StorageEvent {
        rx.recv_timeout(Duration::from_millis(200)).expect("expected event within 200ms")
    }
    fn recv_none(rx: &Subscription) {
        assert!(rx.recv_timeout(Duration::from_millis(100)).is_err(), "unexpected extra event");
    }

    // =============== Local (localStorage) =================

    #[test]
    fn local_set_emits_event_with_old_and_new_values() {
        let local = Arc::new(TestLocalStore::default());
        let session = Arc::new(InMemorySessionStore::new());
        let svc = StorageService::new(local, session);

        let zone = z();
        let part = PartitionKey::TopLevel(o("https://example.com"));
        let origin = o("https://example.com");

        let rx = svc.subscribe();
        let area = svc.local_for(zone, &part, &origin).expect("area");

        // first set: old=None, new=Some("1")
        area.set_item("k", "1").unwrap();
        let ev1 = recv_ok(&rx);
        assert!(matches!(ev1.scope, StorageScope::Local));
        assert!(ev1.source_tab.is_none());
        assert_eq!(ev1.zone, zone);
        assert_eq!(ev1.partition, part);
        assert_eq!(ev1.origin, origin);
        assert_eq!(ev1.key.as_deref(), Some("k"));
        assert_eq!(ev1.old_value, None);
        assert_eq!(ev1.new_value.as_deref(), Some("1"));

        // overwrite: old=Some("1"), new=Some("2")
        area.set_item("k", "2").unwrap();
        let ev2 = recv_ok(&rx);
        assert_eq!(ev2.key.as_deref(), Some("k"));
        assert_eq!(ev2.old_value.as_deref(), Some("1"));
        assert_eq!(ev2.new_value.as_deref(), Some("2"));

        recv_none(&rx);
    }

    #[test]
    fn local_remove_and_clear_emit_events() {
        let local = Arc::new(TestLocalStore::default());
        let session = Arc::new(InMemorySessionStore::new());
        let svc = StorageService::new(local, session);

        let zone = z();
        let part = PartitionKey::None;
        let origin = o("https://a.test");

        let rx = svc.subscribe();
        let area = svc.local_for(zone, &part, &origin).expect("area");

        area.set_item("x", "42").unwrap();
        let _ = recv_ok(&rx); // consume set event

        // remove -> old=Some("42"), new=None, key=Some("x")
        area.remove_item("x").unwrap();
        let ev = recv_ok(&rx);
        assert_eq!(ev.key.as_deref(), Some("x"));
        assert_eq!(ev.old_value.as_deref(), Some("42"));
        assert_eq!(ev.new_value, None);
        assert!(matches!(ev.scope, StorageScope::Local));

        // clear -> key=None, old=None, new=None
        area.set_item("y", "1").unwrap();
        let _ = recv_ok(&rx); // consume set event
        area.clear().unwrap();
        let evc = recv_ok(&rx);
        assert!(evc.key.is_none());
        assert!(evc.old_value.is_none());
        assert!(evc.new_value.is_none());
        assert!(matches!(evc.scope, StorageScope::Local));

        recv_none(&rx);
    }

    // =============== Session (sessionStorage) =================

    #[test]
    fn session_set_emits_event_with_source_tab_and_scope_session() {
        let local = Arc::new(TestLocalStore::default());
        let session = Arc::new(InMemorySessionStore::new());
        let svc = StorageService::new(local, session);

        let zone = z();
        let tab = t();
        let part = PartitionKey::TopLevel(o("https://site.test"));
        let origin = o("https://site.test");

        let rx = svc.subscribe();
        let area = svc.session_for(zone, tab, &part, &origin);

        area.set_item("s", "v").unwrap();
        let ev = recv_ok(&rx);
        assert!(matches!(ev.scope, StorageScope::Session));
        assert_eq!(ev.source_tab, Some(tab));
        assert_eq!(ev.zone, zone);
        assert_eq!(ev.partition, part);
        assert_eq!(ev.origin, origin);
        assert_eq!(ev.key.as_deref(), Some("s"));
        assert_eq!(ev.new_value.as_deref(), Some("v"));
        assert!(ev.old_value.is_none());

        recv_none(&rx);
    }

    // =============== Fanout & Ordering =================

    #[test]
    fn multiple_subscribers_receive_same_events_in_order() {
        let local = Arc::new(TestLocalStore::default());
        let session = Arc::new(InMemorySessionStore::new());
        let svc = StorageService::new(local, session);

        let zone = z();
        let part = PartitionKey::None;
        let origin = o("https://order.test");

        let rx1 = svc.subscribe();
        let rx2 = svc.subscribe();

        let area = svc.local_for(zone, &part, &origin).expect("area");
        area.set_item("a", "1").unwrap();
        area.set_item("b", "2").unwrap();
        area.set_item("c", "3").unwrap();

        // rx1 order
        let e1 = recv_ok(&rx1); assert_eq!(e1.key.as_deref(), Some("a"));
        let e2 = recv_ok(&rx1); assert_eq!(e2.key.as_deref(), Some("b"));
        let e3 = recv_ok(&rx1); assert_eq!(e3.key.as_deref(), Some("c"));
        recv_none(&rx1);

        // rx2 order
        let f1 = recv_ok(&rx2); assert_eq!(f1.key.as_deref(), Some("a"));
        let f2 = recv_ok(&rx2); assert_eq!(f2.key.as_deref(), Some("b"));
        let f3 = recv_ok(&rx2); assert_eq!(f3.key.as_deref(), Some("c"));
        recv_none(&rx2);
    }

    #[test]
    fn dropping_receiver_prunes_subscriber_on_next_publish() {
        // This verifies that sending to a dropped receiver doesn't panic and is pruned.
        let local = Arc::new(TestLocalStore::default());
        let session = Arc::new(InMemorySessionStore::new());
        let svc = StorageService::new(local, session);

        let zone = z();
        let part = PartitionKey::None;
        let origin = o("https://prune.test");

        let rx = svc.subscribe();
        let area = svc.local_for(zone, &part, &origin).expect("area");

        // Publish one, receive it.
        area.set_item("k", "1").unwrap();
        let _ = recv_ok(&rx);

        // Drop the receiver; the next publish should prune it internally without error.
        drop(rx);
        area.set_item("k", "2").unwrap();

        // Nothing to assert directly (subs list is private), but reaching here means no deadlock/panic.
    }
}