use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use gtk4::cairo;
use tokio::runtime::Runtime;
use crate::{EngineCommand, EngineConfig, EngineError, EngineEvent, ZoneConfig};
use crate::tab::{Tab, TabId};
use crate::tick::TickResult;
use crate::viewport::Viewport;
use crate::zone::manager::ZoneManager;
use crate::zone::zone::{ZoneId, Zone};

pub struct GosubEngine {
    _config: EngineConfig,               // Configuration for the whole engine
    zone_manager: ZoneManager,          // Manages zones
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

        // I don't like that we have to clone the config but we need it in the "engine" and the zone manager as well.
        let resolved_config = config.unwrap_or_else(EngineConfig::default);

        Self {
            _config: resolved_config.clone(),
            zone_manager: ZoneManager::new(resolved_config),
            runtime,
        }
    }

    pub fn create_zone(&mut self, config: Option<ZoneConfig>) -> Result<ZoneId, EngineError> {
        self.zone_manager.create_zone(config)
    }

    // Retrieves a reference to a zone by its ID
    pub fn get_zone_mut(&mut self, zone_id: ZoneId) -> Option<Arc<Mutex<Zone>>> {
        self.zone_manager.get_zone_mut(&zone_id)
    }

    // Retrieves a reference to a tab regardless of its zone
    pub fn get_tab(&self, tab_id: TabId) -> Option<Arc<Mutex<Tab>>> {
        for zone_id in self.zone_manager.iter() {
            let zone = self.zone_manager.get_zone_mut(&zone_id)?;
            let zone = zone.lock().ok()?;

            if let Some(tab) = zone.get_tab(tab_id) {
                return Some(tab);
            }
        }

        None
    }

    // Opens a new tab in the specified zone, returning its ID
    pub fn open_tab(&mut self, zone_id: ZoneId, viewport: &Viewport) -> Result<TabId, EngineError> {
        let zone_arc = self.zone_manager.get_zone(zone_id).ok_or(EngineError::ZoneNotFound)?;
        let mut zone = zone_arc.lock().map_err(|_| EngineError::ZoneLocked)?;

        zone.open_tab(self.runtime.clone(), viewport)
    }

    // Do an engine tick, processing all zones and tabs
    pub fn tick(&mut self) -> BTreeMap<TabId, TickResult> {
        let mut results = BTreeMap::new();

        for zone_id in self.zone_manager.iter() {
            let Some(zone_arc) = self.zone_manager.get_zone(zone_id) else {
                continue;
            };

            let Ok(mut zone) = zone_arc.lock() else {
                continue;
            };

            for (tab_id, result) in zone.tick_tabs() {
                results.insert(tab_id, result);
            }
        }

        results
    }

    // Handle an event for a specific tab
    pub fn handle_event(&mut self, tab_id: TabId, event: EngineEvent) -> Result<(), EngineError> {
        let tab_arc = self.get_tab(tab_id).ok_or(EngineError::InvalidTabId)?;
        let mut tab = tab_arc.lock().map_err(|_| EngineError::ZoneLocked)?;

        tab.handle_event(event);
        Ok(())
    }

    // Executes a command for a specific tab
    pub fn execute_command(&mut self, tab_id: TabId, command: EngineCommand) -> Result<(), EngineError> {
        let tab_arc = self.get_tab(tab_id).ok_or(EngineError::InvalidTabId)?;
        let mut tab = tab_arc.lock().map_err(|_| EngineError::ZoneLocked)?;

        tab.execute_command(command);
        Ok(())
    }

    // Retrieves the rendered surface for a specific tab
    pub fn get_surface(&self, tab_id: TabId) -> Option<cairo::ImageSurface> {
        let tab_arc = self.get_tab(tab_id)?;
        let tab = tab_arc.lock().ok()?;

        tab.get_surface().cloned()
    }
}