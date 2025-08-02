use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::runtime::Runtime;
use uuid::Uuid;
use crate::config::ZoneConfig;
use crate::errors::EngineError;
use crate::tab::{Tab, TabId, TabMode};
use crate::tick::TickResult;

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct ZoneId(Uuid);

impl ZoneId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

pub struct Zone {
    pub id: ZoneId,            // ID of the group
    config: ZoneConfig,     // Configuration for the group (like max tabs allowed)
    pub title: String,          // Title of the group (ie: Home, Work)
    pub icon: Vec<u8>,          // Icon of the group (could be a base64 encoded image)
    pub description: String,    // Description of the group
    pub color: [u8; 4],         // Tab color (RGBA)

    tabs: HashMap<TabId, Tab>,  // Tabs in the group, indexed by TabId

    // @TODO: We probably want to isolate the tabs from other groups. We need cookiejars, storage etc
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
            title: "Untitled Group".to_string(),
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

    pub fn open_tab(&mut self, runtime: Arc<Runtime>) -> Result<TabId, EngineError> {
        if self.tabs.len() >= self.config.max_tabs {
            return Err(EngineError::TabLimitExceeded);
        }

        let tab_id = TabId::new();
        self.tabs.insert(tab_id, Tab::new(runtime));
        Ok(tab_id)
    }

    pub fn get_tab(&self, tab_id: TabId) -> Option<&Tab> {
        self.tabs.get(&tab_id)
    }

    pub fn get_tab_mut(&mut self, tab_id: TabId) -> Option<&mut Tab> {
        self.tabs.get_mut(&tab_id)
    }

    pub fn tick(&mut self) {
        for tab in self.tabs.values_mut() {
            tab.tick();
        }
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