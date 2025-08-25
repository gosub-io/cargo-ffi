use crate::cookies::CookieJarHandle;
use crate::engine::storage::StorageService;
use crate::engine::tab::{Tab, TabId};
use crate::engine::tick::TickResult;
use crate::engine::zone::ZoneManager;
use crate::render::backend::{CompositorSink, RenderBackend};
use crate::render::Viewport;
use crate::zone::ZoneConfig;
use crate::zone::{Zone, ZoneId};
use crate::{EngineCommand, EngineConfig, EngineError, EngineEvent};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use tokio::runtime::Runtime;

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
    // Render backend for the engine
    backend: Box<dyn RenderBackend>,
}

impl GosubEngine {
    pub fn update_backend_renderer(&mut self, new_backend: Box<dyn RenderBackend>) {
        self.backend = new_backend;
    }

    /// Create a new engine.
    ///
    /// If `config` is `None`, defaults are used.
    ///
    /// ```
    /// let backend = gosub_engine::render::backends::null::NullBackend::new().expect("null renderer cannot be created (!?)");
    /// let engine = gosub_engine::GosubEngine::new(None, Box::new(backend));
    /// ```
    pub fn new(config: Option<EngineConfig>, backend: Box<dyn RenderBackend>) -> Self {
        let runtime = Arc::new(
            tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .expect("Failed to create Tokio runtime"),
        );

        // I don't like that we have to clone the config but we need it in the "engine" and the zone manager as well.
        let resolved_config = config.unwrap_or_else(EngineConfig::default);

        Self {
            _config: resolved_config.clone(),
            zone_manager: ZoneManager::new(resolved_config),
            runtime,
            backend,
        }
    }

    /// Create a new zone and return its [`ZoneId`].
    pub(crate) fn create_zone(
        &mut self,
        zone_id: Option<ZoneId>,
        config: Option<ZoneConfig>,
        storage_service: Option<Arc<StorageService>>,
        cookie_jar: Option<CookieJarHandle>,
    ) -> Result<ZoneId, EngineError> {
        self.zone_manager
            .create_zone(zone_id, config, storage_service, cookie_jar)
    }

    /// Get a mutable handle to a zone.
    ///
    /// This returns an [`Arc<Mutex<Zone>>`]; lock it before use.
    pub fn get_zone_mut(&mut self, zone_id: ZoneId) -> Option<Arc<Mutex<Zone>>> {
        self.zone_manager.get_zone_mut(&zone_id)
    }

    /// Retrieves a reference to a tab regardless of its zone
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
    /// let backend = gosub_engine::render::backends::null::NullBackend::new().expect("null renderer cannot be created (!?)");
    /// let mut engine = gosub_engine::GosubEngine::new(None, Box::new(backend));
    ///
    /// let zone_id = engine.zone_builder().create().unwrap();
    ///
    /// let viewport = gosub_engine::render::Viewport::new(0, 0, 800, 600);
    /// let tab_id = engine.open_tab_in_zone(zone_id, viewport).unwrap();
    /// ```
    pub fn open_tab_in_zone(
        &mut self,
        zone_id: ZoneId,
        viewport: Viewport,
    ) -> Result<TabId, EngineError> {
        let zone_arc = self
            .zone_manager
            .get_zone(zone_id)
            .ok_or(EngineError::ZoneNotFound)?;
        let mut zone = zone_arc.lock().map_err(|_| EngineError::ZoneLocked)?;

        zone.open_tab(self.runtime.clone(), viewport)
    }

    /// Do an engine tick, processing all zones and tabs
    pub fn tick(&mut self, host: &mut impl CompositorSink) -> BTreeMap<TabId, TickResult> {
        let mut results = BTreeMap::new();

        for zone_id in self.zone_manager.iter() {
            let Some(zone_arc) = self.zone_manager.get_zone(zone_id) else {
                continue;
            };

            let Ok(mut zone) = zone_arc.lock() else {
                continue;
            };

            // Process and storage events currently pending in the zone
            zone.pump_storage_events();

            // Tick each tab and aggregate the results
            for (tab_id, result) in zone.tick_all_tabs(&mut *self.backend, host) {
                results.insert(tab_id, result);
            }
        }

        results
    }

    /// Handle an event for a specific tab
    pub fn handle_event(&mut self, tab_id: TabId, event: EngineEvent) -> Result<(), EngineError> {
        let tab_arc = self.get_tab(tab_id).ok_or(EngineError::InvalidTabId)?;
        let mut tab = tab_arc.lock().map_err(|_| EngineError::ZoneLocked)?;

        tab.handle_event(event);
        Ok(())
    }

    /// Executes a command for a specific tab
    pub fn execute_command(
        &mut self,
        tab_id: TabId,
        command: EngineCommand,
    ) -> Result<(), EngineError> {
        let tab_arc = self.get_tab(tab_id).ok_or(EngineError::InvalidTabId)?;
        let mut tab = tab_arc.lock().map_err(|_| EngineError::ZoneLocked)?;

        tab.execute_command(command);
        Ok(())
    }
}
