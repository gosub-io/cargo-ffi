use tokio::sync::mpsc::Sender;
use crate::engine::events::EngineCommand;
use crate::tab::TabId;

#[derive(Clone)]
pub struct TabHandle {
    tab_id: TabId,
    engine_tx: Sender<EngineCommand>,
}

impl TabHandle {
    pub fn new(tab_id: TabId, engine_tx: Sender<EngineCommand>) -> Self {
        Self { tab_id, engine_tx }
    }

    pub fn id(&self) -> TabId {
        self.tab_id
    }

    pub fn engine_tx(&self) -> Sender<EngineCommand> {
        self.engine_tx.clone()
    }
}