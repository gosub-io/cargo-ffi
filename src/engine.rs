use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use gtk4::cairo;
use tokio::runtime::Runtime;
use EngineError::ZoneNotFound;
use crate::config::{EngineConfig, ZoneConfig};
use crate::errors::EngineError;
use crate::event::{EngineCommand, EngineEvent};
use crate::zone::{ZoneId, Zone};
use crate::tab::{Tab, TabId};
use crate::tick::TickResult;

pub struct GosubEngine {
    config: EngineConfig,               // Configuration for the whole engine
    zones: HashMap<ZoneId, Zone>,       // List of zones
    pub runtime: Arc<Runtime>,          // Tokio runtime for async operations
}

impl GosubEngine {
    // Initializes a new Gosub Engine with the provided configuration. Can use None when using the
    // default configuration.
    pub fn new(config: Option<EngineConfig>) -> Self {
        let runtime = Arc::new(
            tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .expect("Failed to create Tokio runtime")
        );

        Self {
            config: config.unwrap_or_else(EngineConfig::default),
            zones: HashMap::new(),
            runtime,
        }
    }

    // Creates a new zone, returning its ID or error
    pub fn create_zone(&mut self, zone_config: Option<ZoneConfig>) -> Result<ZoneId, EngineError> {
        if self.zones.len() >= self.config.max_zones {
            return Err(EngineError::ZoneLimitExceeded);
        }

        let zone = Zone::new(zone_config.unwrap_or(self.config.default_zone_config.clone()));
        let id = zone.id;

        self.zones.insert(zone.id, zone);

        Ok(id)
    }

    // Retrieves a reference to a zone by its ID
    pub fn get_zone_mut(&mut self, zone_id: ZoneId) -> Option<&mut Zone> {
        self.zones.get_mut(&zone_id)
    }

    // Retrieves a reference to a tab regardless of its zone
    pub fn get_tab_mut(&mut self, tab_id: TabId) -> Option<&mut Tab> {
        for zone in self.zones.values_mut() {
            if let Some(tab) = zone.get_tab_mut(tab_id) {
                return Some(tab);
            }
        }

        None
    }

    // Opens a new tab in the specified zone, returning its ID
    pub fn open_tab(&mut self, zone_id: ZoneId) -> Result<TabId, EngineError> {
        let zone = self.zones.get_mut(&zone_id).ok_or(ZoneNotFound)?;
        zone.open_tab(self.runtime.clone())
    }

    // Do an engine tick, processing all zones and tabs
    pub fn tick(&mut self) -> BTreeMap<TabId, TickResult> {
        let mut results = BTreeMap::new();

        for zone in self.zones.values_mut() {
            for (tab_id, result) in zone.tick_tabs() {
                results.insert(tab_id, result);
            }
        }

        results
    }

    // Handle an event for a specific tab
    pub fn handle_event(&mut self, tab_id: TabId, event: EngineEvent) -> Result<(), EngineError> {
        let tab = self.get_tab_mut(tab_id).ok_or(EngineError::InvalidTabId)?;

        tab.handle_event(event);
        Ok(())
    }

    // Executes a command for a specific tab
    pub fn execute(&mut self, tab_id: TabId, command: EngineCommand) -> Result<(), EngineError> {
        let tab = self.get_tab_mut(tab_id).ok_or(EngineError::InvalidTabId)?;

        tab.execute_command(command);
        Ok(())
    }

    // Retrieves the rendered surface for a specific tab
    pub fn get_surface(&self, tab_id: TabId) -> Option<&cairo::ImageSurface> {
        for zone in self.zones.values() {
            if let Some(tab) = zone.get_tab(tab_id) {
                return tab.get_surface();
            }
        }
        None
    }
}