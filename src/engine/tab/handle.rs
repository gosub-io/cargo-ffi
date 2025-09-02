use tokio::sync::mpsc::Sender;
use crate::EngineError;
use crate::events::TabCommand;
use crate::tab::TabId;

#[derive(Clone)]
pub struct TabHandle {
    /// Id of the tab
    tab_id: TabId,
    /// Sender part of the channel to send Engine commands to.
    cmd_tx: Sender<TabCommand>,
}

impl std::fmt::Debug for TabHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TabHandle")
            .field("tab_id", &self.tab_id)
            .finish()
    }
}

impl TabHandle {
    pub fn new(tab_id: TabId, cmd_tx: Sender<TabCommand>) -> Self {
        Self { tab_id, cmd_tx }
    }

    pub fn id(&self) -> TabId {
        self.tab_id
    }

    pub fn cmd_tx(&self) -> Sender<TabCommand> {
        self.cmd_tx.clone()
    }

    pub async fn send(&self, command: TabCommand) -> anyhow::Result<(), EngineError> {
        self.cmd_tx
            .send(command)
            .await
            .map_err(|_| EngineError::ChannelClosed)
    }
}