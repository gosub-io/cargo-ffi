use crate::render::backend::{RenderBackend};
use crate::zone::{Zone, ZoneConfig, ZoneHandle, ZoneId};
use crate::EngineConfig;
use std::sync::Arc;
use tokio::runtime::Runtime;
use tokio::sync::mpsc::{Receiver, Sender};
use crate::engine::events::EngineEvent;

/// Entry point to the Gosub engine.
///
/// Create an engine, then create zones and open tabs.
///
/// See [`Viewport`], [`ZoneId`], [`TabId`], [`EngineEvent`], [`EngineCommand`].
pub struct GosubEngine {
    /// Configuration for the whole engine
    _config: EngineConfig,
    /// Tokio runtime for async operations
    pub runtime: Arc<Runtime>,
    // Render backend for the engine
    backend: Box<dyn RenderBackend>,
}

impl GosubEngine {
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
            runtime,
            backend,
        }
    }

    pub fn create_event_channel(&self, cap: usize) -> (Sender<EngineEvent>, Receiver<EngineEvent>) {
        tokio::sync::mpsc::channel(cap)
    }

    pub fn update_backend_renderer(&mut self, new_backend: Box<dyn RenderBackend>) {
        self.backend = new_backend;
    }


    pub fn create_zone(
        &mut self,
        config: ZoneConfig,
        zone_id: Option<ZoneId>
    ) -> anyhow::Result<ZoneHandle> {
        
        let zone_id = ZoneId::new();

        let zone = Arc::new(Zone::new_with_id(
            zone_id,
            config,             // 👈 passed in here
            self.storage.clone(),
            None,               // or Some(cookie jar handle)
            self.engine_event_tx.clone(),
        ));

        self.zones.insert(zone_id, zone);
        Ok(ZoneHandle::new(zone_id, self.engine_cmd_tx.clone()))
    }
}
}
