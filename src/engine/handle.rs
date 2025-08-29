use tokio::sync::mpsc::Sender;
use std::sync::{Arc, RwLock};
use crate::engine::events::EngineCommand;
use crate::render::backend::RenderBackend;

#[derive(Clone)]
pub struct EngineHandle {
    /// Engine-wide command sender (for e.g. shutdown, logging, etc.)
    cmd_tx: Sender<EngineCommand>,

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

impl EngineHandle {
    pub fn new(
        cmd_tx: Sender<EngineCommand>,
        backend: Arc<RwLock<Box<dyn RenderBackend + Send + Sync>>>,
    ) -> Self {
        Self { cmd_tx, backend }
    }

    /// Get the current backend (always reflects latest swap in engine).
    pub fn backend(&self) -> Arc<RwLock<Box<dyn RenderBackend + Send + Sync>>> {
        self.backend.clone()
    }

    /// Send an engine-level command.
    pub async fn send(&self, cmd: EngineCommand) -> anyhow::Result<()> {
        self.cmd_tx.send(cmd).await?;
        Ok(())
    }
}
