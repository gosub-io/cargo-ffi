use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::runtime::Runtime;
use uuid::Uuid;
use crate::engine::errors::EngineError;
use crate::tab::{Tab, TabId, TabMode};
use crate::tick::TickResult;
use crate::viewport::Viewport;
use crate::ZoneConfig;

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct ZoneId(Uuid);

impl ZoneId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
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

    tabs: HashMap<TabId, Tab>,  // Tabs in the zone
}

impl Zone {
    pub fn new(config: ZoneConfig) -> Self {
        let random_color = [
            rand::random::<u8>(),
            rand::random::<u8>(),
            rand::random::<u8>(),
            0xff, // Fully opaque
        ];

        Self {
            id: ZoneId::new(),
            title: "Untitled Zone".to_string(),
            icon: vec![],
            description: "".to_string(),
            color: random_color,
            tabs: HashMap::new(),
            config,
        }
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

    // Open a new tab into the zone
    pub fn open_tab(&mut self, runtime: Arc<Runtime>, viewport: &Viewport) -> Result<TabId, EngineError> {
        if self.tabs.len() >= self.config.max_tabs {
            return Err(EngineError::TabLimitExceeded);
        }

        let tab_id = TabId::new();
        self.tabs.insert(tab_id, Tab::new(runtime, viewport));
        Ok(tab_id)
    }

    pub fn get_tab(&self, tab_id: TabId) -> Option<&Tab> {
        self.tabs.get(&tab_id)
    }

    pub fn get_tab_mut(&mut self, tab_id: TabId) -> Option<&mut Tab> {
        self.tabs.get_mut(&tab_id)
    }

    pub fn tick_tabs(&mut self) -> BTreeMap<TabId, TickResult> {
        let now = Instant::now();
        let mut results = BTreeMap::new();

        for (tab_id, tab) in self.tabs.iter_mut() {
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