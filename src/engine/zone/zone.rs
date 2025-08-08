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
use crate::engine::zone::password_store::PasswordStore;
use crate::engine::zone::storage::Storage;

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
    pub id: ZoneId,             // ID of the zone
    config: ZoneConfig,         // Configuration for the zone (like max tabs allowed)
    pub title: String,          // Title of the zone (ie: Home, Work)
    pub icon: Vec<u8>,          // Icon of the zone (could be a base64 encoded image)
    pub description: String,    // Description of the zone
    pub color: [u8; 4],         // Tab color (RGBA)

    tabs: HashMap<TabId, Arc<Mutex<Tab>>>,  // Tabs in the zone

    pub session_storage: Storage,
    pub local_storage: Storage,
    pub cookie_jar: CookieJarHandle,     // Where to load/store cookies within this zone
    pub password_store: PasswordStore,
    pub shared_flags: SharedFlags,
}

pub struct SharedFlags {
    pub share_autocomplete: bool,       // Other zones are allowed to read this autocomplete elements
    pub share_bookmarks: bool,          // Other zones are allowed to read bookmarks
    pub share_passwords: bool,          // Other zones are allowd to read password entries
    pub share_cookiejar: bool,          // Other zones are allowed to read cookies
}

impl Zone {
    // Creates a new zone with a specific zone ID
    pub fn new_with_id(zone_id: ZoneId, config: ZoneConfig) -> Self {
        let random_color = [
            rand::random::<u8>(),
            rand::random::<u8>(),
            rand::random::<u8>(),
            0xff, // Fully opaque
        ];

        Self {
            id: zone_id,
            title: "Untitled Zone".to_string(),
            icon: vec![],
            description: "".to_string(),
            color: random_color,
            tabs: HashMap::new(),
            config,

            session_storage: Storage::new(),
            local_storage: Storage::new(),
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
    pub fn new(config: ZoneConfig) -> Self {
        let zone_id = ZoneId::new();
        Zone::new_with_id(zone_id, config)
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
}