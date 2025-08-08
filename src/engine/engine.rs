use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use gtk4::cairo;
use tokio::runtime::Runtime;
use crate::{EngineCommand, EngineConfig, EngineError, EngineEvent, ZoneConfig};
use crate::engine::tab::{Tab, TabId};
use crate::engine::tick::TickResult;
use crate::viewport::Viewport;
use crate::engine::zone::ZoneManager;
use crate::engine::zone::{ZoneId, Zone};

/// Entry point to the Gosub engine.
///
/// Create an engine, then create zones and open tabs.
///
/// See [`Viewport`], [`ZoneId`], [`TabId`], [`EngineEvent`], [`EngineCommand`].
pub struct GosubEngine {
    /// Configuration for the whole engine
    _config: EngineConfig,
    /// Manages zones
    zone_manager: ZoneManager,
    /// Tokio runtime for async operations
    pub runtime: Arc<Runtime>,
}

impl GosubEngine {
    /// Create a new engine.
    ///
    /// If `config` is `None`, defaults are used.
    ///
    /// ```
    /// # use gosub_engine::prelude::*;
    /// let engine = GosubEngine::new(None);
    /// ```
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


    /// Create a new zone and return its [`ZoneId`].
    ///
    /// ```
    /// # use gosub_engine::prelude::*;
    /// # let mut engine = GosubEngine::new(None);
    /// let zone_id = engine.create_zone(None, None).unwrap();
    /// ```
    pub fn create_zone(&mut self, zone_id: Option<ZoneId>, config: Option<ZoneConfig>) -> Result<ZoneId, EngineError> {
        self.zone_manager.create_zone(zone_id, config)
    }

    /// Get a mutable handle to a zone.
    ///
    /// This returns an [`Arc<Mutex<Zone>>`]; lock it before use.
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

    /// Open a new tab in a zone and return its [`TabId`].
    ///
    /// ```
    /// # use gosub_engine::prelude::*;
    /// # let mut engine = GosubEngine::new(None);
    /// # let zone_id = engine.create_zone(None, None).unwrap();
    /// # let vp = Viewport::new(800, 600);
    /// let tab_id = engine.open_tab(zone_id, &vp).unwrap();
    /// ```
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

            for (tab_id, result) in zone.tick_all_tabs() {
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