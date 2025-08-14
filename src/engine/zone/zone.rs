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
use crate::EngineError;
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
use crate::zone::ZoneConfig;

/// A unique identifier for a [`Zone`](crate::zone::Zone) within a [`GosubEngine`](crate::GosubEngine).
///
/// Internally, a `ZoneId` wraps a [`Uuid`] to guarantee global uniqueness for
/// each zone created in the engine.
///
/// **Note:** The use of [`Uuid`] is an implementation detail and may change in
/// the future without notice. Always treat `ZoneId` as an opaque handle rather
/// than relying on its internal representation.
///
/// # Purpose
///
/// A `ZoneId` allows the engine and user code to unambiguously reference and
/// operate on a specific [`Zone`], even if multiple zones are created, closed,
/// or restored across sessions.
///
/// # Examples
///
/// Creating a new `ZoneId` manually:
/// ```
/// use gosub_engine::zone::ZoneId;
///
/// let id = ZoneId::new();
/// println!("New zone ID: {:?}", id);
/// ```
///
/// Creating a zone with a fixed ID:
/// ```no_run
/// use gosub_engine::GosubEngine;
/// use gosub_engine::zone::ZoneId;
///
/// let mut engine = GosubEngine::new(None);
/// let fixed_id = ZoneId::from("123e4567-e89b-12d3-a456-426614174000");
/// let zone_id = engine.zone_builder()
///     .id(fixed_id)
///     .create()
///     .unwrap();
/// assert_eq!(zone_id, fixed_id);
/// ```
///
/// Using `ZoneId` as a map key:
/// ```
/// use gosub_engine::zone::ZoneId;
/// use std::collections::HashMap;
///
/// let mut zones: HashMap<ZoneId, String> = HashMap::new();
/// let z1 = ZoneId::new();
/// zones.insert(z1, "Profile 1".to_string());
/// ```
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

/// A `Zone` is a self-contained browsing context within a [`GosubEngine`](crate::engine::GosubEngine).
///
/// All tabs opened in the same zone share the zone's **session storage**,
/// **local storage**, **cookie jar**, **bookmarks**, **autocomplete data**,
/// and other per-zone resources.
///
/// Zones are the Gosub equivalent of browser profiles. They can be:
///
/// - **Private**: Only tabs in that zone can read/write its data.
/// - **Shared**: Marked as `shared`, allowing other zones to read (and in
///   some cases write) data back into it. This is useful for sharing
///   credentials, bookmarks, or autocomplete entries between profiles.
///
/// # Key concepts
///
/// - **Isolation:** Tabs in different zones cannot access each other’s
///   storage unless sharing is explicitly enabled.
/// - **Persistence:** A zone can be associated with a [`StorageService`]
///   for persistent storage of cookies, local/session data, etc.
/// - **Identification:** Each zone has a stable [`ZoneId`] for lookups
///   and persistence across sessions.
/// - **UI metadata:** Title, icon, description, and tab color are available
///   for use in browser UIs.
///
/// # Typical usage
///
/// ```
/// use gosub_engine::GosubEngine;
/// use std::sync::Arc;
/// use gosub_engine::storage::{StorageService, InMemorySessionStore, SqliteLocalStore};
/// use gosub_engine::zone::{ZoneConfig, ZoneId};
///
/// let mut engine = GosubEngine::new(None);
///
/// // Create a persistent storage service
/// let storage = Arc::new(StorageService::new(
///     Arc::new(SqliteLocalStore::new("local.db").unwrap()),
///     Arc::new(InMemorySessionStore::new()),
/// ));
///
/// // Create a zone with custom config and storage
/// let zone_id = engine.zone_builder()
///     .id(ZoneId::new())
///     .storage(storage.clone())
///     .create()
///     .unwrap();
///
/// // Fetch the zone and inspect properties
/// let zone = engine.get_zone_mut(zone_id).unwrap();
/// println!("Zone title: {}", zone.lock().unwrap().title);
/// ```
///
/// # Fields
///
/// - `id`: The [`ZoneId`] that uniquely identifies this zone.
/// - `config`: Per-zone configuration (e.g., max number of tabs).
/// - `title`: Display title for the zone (e.g., "Home", "Work").
/// - `icon`: Icon bytes (may be base64-encoded or raw image data).
/// - `description`: Human-readable description.
/// - `color`: RGBA color for tabs in this zone.
/// - `tabs`: The set of [`Tab`]s currently open in the zone.
/// - `storage`: The [`StorageService`] used for local/session storage.
/// - `storage_rx`: Subscription for observing session storage changes.
/// - `cookie_jar`: Where cookies are stored/loaded for this zone.
/// - `password_store`: Per-zone password storage.
/// - `shared_flags`: Flags that define which data is shared with other zones.
///
/// **Note:** Internal details such as `tabs` and `storage_rx` are
/// engine-managed; user code typically interacts through the public API.
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

    /// Where to load/store cookies within this zone
    pub cookie_jar: CookieJarHandle,

    /// Per-zone password storage
    pub password_store: PasswordStore,

    /// Flags controlling which data is shared with other zones.
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
        cookie_jar: Option<CookieJarHandle>,
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

        let cookie_jar = cookie_jar.unwrap_or_else(|| Arc::new(RwLock::new(DefaultCookieJar::new())));

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

            cookie_jar,
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
    pub fn new(config: ZoneConfig, storage: Arc<StorageService>, cookie_jar: Option<CookieJarHandle>) -> Self {
        let zone_id = ZoneId::new();
        Zone::new_with_id(zone_id, config, storage, cookie_jar)
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