use tokio::sync::mpsc::Sender;
use crate::engine::events::EngineCommand;
use crate::tab::TabId;

#[derive(Clone)]
pub struct TabHandle {
    pub tab_id: TabId,
    pub engine_tx: Sender<EngineCommand>,
}

impl TabHandle {
    pub fn new(tab_id: TabId, engine_tx: Sender<EngineCommand>) -> Self {
        Self { tab_id, engine_tx }
    }
}