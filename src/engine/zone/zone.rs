// src/engine/zone.rs
//! Zone system: [`ZoneManager`], [`Zone`], and [`ZoneId`].
//!
use std::collections::{BTreeMap, HashMap};
use std::fmt::Display;
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};
use serde::{Deserialize, Serialize};
use tokio::runtime::Runtime;
use crate::engine::tab::{Tab, TabId, TabMode};
use crate::viewport::Viewport;
use crate::{EngineError, ZoneConfig};
use crate::engine::tick::TickResult;
use uuid::Uuid;
use crate::engine::cookies::CookieJarHandle;
use crate::engine::cookies::DefaultCookieJar;
use crate::engine::storage::{PartitionKey, StorageArea, StorageEvent, StorageHandles, StorageService, Subscription};
use crate::engine::storage::event::StorageScope;
use crate::engine::zone::password_store::PasswordStore;
use rand::{Rng, SeedableRng};
use rand::rngs::StdRng;
use crate::engine::storage::types::compute_partition_key;

/// A unique identifier for a zone, represented as a UUID.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ZoneId(Uuid);

impl ZoneId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl From<Uuid> for ZoneId {
    fn from(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

impl From<&str> for ZoneId {
    fn from(s: &str) -> Self {
        Self(Uuid::parse_str(s).unwrap_or_else(|_| Uuid::new_v4()))
    }
}

impl Display for ZoneId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

// A zone is a "container" where all tabs share its session storage, local storage, cookie jars, bookmarks,
// autocomplete etc. A zone can be marked as "shared", in which case other zones can also read (and sometimes
// write) data back into the zone.
pub struct Zone {
    /// ID of the zone
    pub id: ZoneId,
    /// Configuration for the zone (like max tabs allowed)
    config: ZoneConfig,
    /// Title of the zone (ie: Home, Work)
    pub title: String,
    /// Icon of the zone (could be a base64 encoded image)
    pub icon: Vec<u8>,
    /// Description of the zone
    pub description: String,
    /// Tab color (RGBA)
    pub color: [u8; 4],

    /// Tabs in the zone
    tabs: HashMap<TabId, Arc<Mutex<Tab>>>,

    /// Session storage for the zone (shared between all tabs in the zone)
    pub storage: Arc<StorageService>,
    /// Subscription for session storage changes
    storage_rx: Subscription,

    pub cookie_jar: CookieJarHandle,        // Where to load/store cookies within this zone
    pub password_store: PasswordStore,
    pub shared_flags: SharedFlags,
}

pub struct SharedFlags {
    pub share_autocomplete: bool,       // Other zones are allowed to read this autocomplete elements
    pub share_bookmarks: bool,          // Other zones are allowed to read bookmarks
    pub share_passwords: bool,          // Other zones are allowed to read password entries
    pub share_cookiejar: bool,          // Other zones are allowed to read cookies
}

impl Zone {
    // Creates a new zone with a specific zone ID
    pub fn new_with_id(
        zone_id: ZoneId,
        config: ZoneConfig,
        storage: Arc<StorageService>,
    ) -> Self {

        // We generate the color by using the zone id as a seed
        let mut rng = StdRng::seed_from_u64(zone_id.0.as_u64_pair().0);
        let random_color = [
            rng.random::<u8>(),
            rng.random::<u8>(),
            rng.random::<u8>(),
            0xff, // Fully opaque
        ];

        let storage_rx = storage.subscribe();

        Self {
            id: zone_id,
            title: "Untitled Zone".to_string(),
            icon: vec![],
            description: "".to_string(),
            color: random_color,
            tabs: HashMap::new(),
            config,

            storage,
            storage_rx,

            cookie_jar: Arc::new(RwLock::new(DefaultCookieJar::new())),
            password_store: PasswordStore::new(),
            shared_flags: SharedFlags {
                share_autocomplete: false,
                share_bookmarks: false,
                share_passwords: false,
                share_cookiejar: false,
            },
        }
    }

    // Creates a new zone with a random ID and the provided configuration
    pub fn new(config: ZoneConfig, storage: Arc<StorageService>) -> Self {
        let zone_id = ZoneId::new();
        Zone::new_with_id(zone_id, config, storage)
    }

    pub fn set_title(&mut self, title: &str) {
        self.title = title.to_string();
    }

    pub fn set_icon(&mut self, icon: Vec<u8>) {
        self.icon = icon;
    }

    pub fn set_description(&mut self, description: &str) {
        self.description = description.to_string();
    }

    pub fn set_color(&mut self, color: [u8; 4]) {
        self.color = color;
    }

    pub fn set_cookie_jar(&mut self, cookie_jar: CookieJarHandle) {
        self.cookie_jar = cookie_jar;
    }

    // Open a new tab into the zone
    pub fn open_tab(&mut self, runtime: Arc<Runtime>, viewport: &Viewport) -> Result<TabId, EngineError> {
        if self.tabs.len() >= self.config.max_tabs {
            return Err(EngineError::TabLimitExceeded);
        }

        let tab_id = TabId::new();
        self.tabs.insert(tab_id, Arc::new(Mutex::new(Tab::new(self.id, runtime, viewport, Some(self.cookie_jar.clone())))));
        Ok(tab_id)
    }

    pub fn get_tab(&self, tab_id: TabId) -> Option<Arc<Mutex<Tab>>> {
        self.tabs.get(&tab_id).cloned()
    }

    pub fn get_tab_mut(&mut self, tab_id: TabId) -> Option<Arc<Mutex<Tab>>> {
        self.tabs.get_mut(&tab_id).cloned()
    }

    // Ticks all tabs in the zone, returning a map of TabId to TickResult
    pub fn tick_all_tabs(&mut self) -> BTreeMap<TabId, TickResult> {
        let now = Instant::now();
        let mut results = BTreeMap::new();

        for (tab_id, tab_arc) in self.tabs.iter_mut() {
            let mut tab = tab_arc.lock().unwrap();

            let interval = match tab.mode {
                TabMode::Active => Duration::from_secs(0),              // Always run
                TabMode::BackgroundLive => Duration::from_millis(100),  // Run at 10Hz
                TabMode::BackgroundIdle => Duration::from_secs(1),      // Run at 1Hz
                TabMode::Suspended => continue,                              // Skip suspended tabs
            };

            // Check if enough time has passed since the last tick
            if !interval.is_zero() && now.duration_since(tab.last_tick) < interval {
                continue; // Skip if not time to tick
            }
            tab.last_tick = now;

            let result = tab.tick();
            results.insert(*tab_id, result);
        }

        results
    }


    /// Get the shared localStorage area for this (zone × partition × origin).
    pub fn local_area(&self, pk: &PartitionKey, origin: &url::Origin) -> anyhow::Result<Arc<dyn StorageArea>> {
        self.storage.local_for(self.id, pk, origin)
    }

    /// Get the per-tab sessionStorage area for (zone × tab × partition × origin).
    pub fn session_area(&self, tab: TabId, pk: &PartitionKey, origin: &url::Origin) -> Arc<dyn StorageArea> {
        self.storage.session_for(self.id, tab, pk, origin)
    }

    /// Tell the storage layer a tab is gone (cleans its sessionStorage).
    pub fn on_tab_closed(&self, tab: TabId) {
        self.storage.drop_tab(self.id, tab);
    }

    // Read the storage channel and process storage events
    pub fn pump_storage_events(&mut self) {
        // Drain the queue without blocking.
        while let Ok(ev) = self.storage_rx.try_recv() {
            self.dispatch_storage_event(ev);
        }
    }

    /// Dispatches the storage event to the correct tabs based on the event's scope.
    fn dispatch_storage_event(&mut self, ev: StorageEvent) {
        match ev.scope {
            StorageScope::Local => {
                // Deliver to *other* same-origin documents in the same zone/partition.
                for (tab_id, tab) in &self.tabs {
                    // Skip the tab that caused it (spec behavior)
                    if Some(*tab_id) == ev.source_tab { continue; }

                    let mut tab = tab.lock().unwrap();
                    tab.dispatch_storage_event_to_same_origin_docs(
                        &ev.origin, /*include_iframes=*/true, &ev
                    );
                }
            }
            StorageScope::Session => {
                // sessionStorage is per top-level browsing context (tab).
                // Optionally deliver to *same tab* same-origin iframes (not other tabs).
                if let Some(tab_id) = ev.source_tab {
                    if let Some(tab) = self.tabs.get(&tab_id) {
                        let mut tab = tab.lock().unwrap();
                        tab.dispatch_storage_event_to_same_origin_docs(
                            &ev.origin, /*include_iframes=*/true, &ev
                        );
                    }
                }
            }
        }
    }


    pub fn on_tab_commit(&self, tab: &mut Tab, final_url: &url::Url) -> anyhow::Result<()> {
        tab.partition_key = compute_partition_key(final_url, tab.partition_policy);

        // 2) bind storage
        let origin = final_url.origin().clone();
        let local   = self.local_area(&tab.partition_key, &origin)?;
        let session = self.session_area(tab.id, &tab.partition_key, &origin);
        tab.bind_storage(StorageHandles{ local, session }); // add on EngineInstance
        Ok(())
    }
}