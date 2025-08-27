use crate::events::EngineEvent;
use crate::events::TabCommand;
use crate::tab::TabId;
use tokio::sync::{broadcast, mpsc};
use crate::tab::services::EffectiveTabServices;

/// Arguments required to spawn a new tab task.
#[derive(Debug)]
pub struct TabSpawnArgs {
    /// Tab Id
    pub tab_id: TabId,
    /// Receive channel for commands for the tab
    pub cmd_rx: mpsc::Receiver<TabCommand>,
    /// Send channel for events from the tab to the UA
    pub event_tx: broadcast::Sender<EngineEvent>,
    /// Services available to the tab
    pub services: EffectiveTabServices,
}

