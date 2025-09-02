use std::sync::{Arc, RwLock};
use tokio::sync::{oneshot, mpsc, broadcast};
use crate::engine::events::EngineCommand;
use crate::EngineError;
use crate::events::EngineEvent;
use crate::render::backend::RenderBackend;
use crate::zone::{ZoneConfig, ZoneHandle, ZoneId, ZoneServices};

#[allow(unused)]
pub struct EngineHandle {
    /// Engine-wide command sender (for e.g. shutdown, logging, etc.)
    cmd_tx: mpsc::Sender<EngineCommand>,
    /// Event sender to forward engine events to the caller
    event_tx: broadcast::Sender<EngineEvent>,
    /// Shared reference to the active backend
    backend: Arc<RwLock<Box<dyn RenderBackend + Send + Sync>>>,
}

impl std::fmt::Debug for EngineHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EngineHandle")
            .field("cmd_tx", &self.cmd_tx)
            .field("backend", &"Arc<RwLock<Box<dyn RenderBackend>>>")
            .finish()
    }
}

#[allow(unused)]
impl EngineHandle {
    pub fn new(
        cmd_tx: mpsc::Sender<EngineCommand>,
        event_tx: broadcast::Sender<EngineEvent>,
        backend: Arc<RwLock<Box<dyn RenderBackend + Send + Sync>>>,
    ) -> Self {
        Self {
            cmd_tx,
            event_tx,
            backend,
        }
    }

    /// Get the current backend (always reflects latest swap in engine).
    pub fn backend(&self) -> Arc<RwLock<Box<dyn RenderBackend + Send + Sync>>> {
        self.backend.clone()
    }

    pub fn subscribe_events(&self) -> broadcast::Receiver<EngineEvent> {
        self.event_tx.subscribe()
    }

    /// Send an engine-level command.
    async fn send(&self, cmd: EngineCommand) -> anyhow::Result<()> {
        self.cmd_tx.send(cmd).await?;
        Ok(())
    }

    /// Gracefully shutdown the engine, waiting for all tasks to finish.
    pub async fn shutdown(&self) -> anyhow::Result<(), EngineError> {
        let (tx, rx) = oneshot::channel();

        self.cmd_tx
            .send(EngineCommand::Shutdown { reply: tx })
            .await
            .map_err(|_| EngineError::ChannelClosed)?;

        rx.await.map_err(|e| EngineError::TaskInitFailed(e.into()))?
    }

    pub async fn create_zone(
        &self,
        config: ZoneConfig,
        services: ZoneServices,
        zone_id: Option<ZoneId>,
    ) -> anyhow::Result<ZoneHandle, EngineError> {
        let (tx, rx) = oneshot::channel::<Result<ZoneHandle, EngineError>>();

        self.cmd_tx
            .send(EngineCommand::CreateZone { config, services, zone_id, event_tx: self.event_tx.clone(), reply: tx })
            .await
            .map_err(|e| EngineError::CreateZone(e.into()))?;

        rx.await.map_err(|e| EngineError::TaskInitFailed(e.into()))?
    }
}
