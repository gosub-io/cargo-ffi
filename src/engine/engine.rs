use crate::cookies::CookieJarHandle;
use crate::engine::storage::StorageService;
use crate::engine::tab::{Tab, TabId};
use crate::engine::tick::TickResult;
use crate::engine::zone::ZoneManager;
use crate::render::backend::{CompositorSink, RenderBackend};
use crate::render::Viewport;
use crate::zone::{ZoneConfig, ZoneHandle};
use crate::zone::{Zone, ZoneId};
use crate::{EngineCommand, EngineConfig, EngineError, EngineEvent};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
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
            runtime,
            backend,
        }
    }

    pub fn create_event_channel(&self, cap: usize) -> (Sender<EngineEvent>, Receiver<EngineEvent>) {
        tokio::sync::mpsc::channel(cap)
    }

    pub fn create_zone(&self, event_tx: Sender<EngineEvent>) -> ZoneHandle {
        ZoneHandle::new(event_tx)
    }

    /// Get a mutable reference to the zone manager.pub
    /// Create a new zone and return its [`ZoneId`].
    pub(crate) fn create_zone_obs(
        &mut self,
        zone_id: Option<ZoneId>,
        config: Option<ZoneConfig>,
        storage_service: Option<Arc<StorageService>>,
        cookie_jar: Option<CookieJarHandle>,
    ) -> Result<ZoneId, EngineError> {
    }
}
