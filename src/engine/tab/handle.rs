use crate::events::TabCommand;
use crate::render::Viewport;
use crate::tab::{TabId, TabSink};
use crate::EngineError;
use std::sync::Arc;
use tokio::sync::mpsc;

#[derive(Clone)]
pub struct TabHandle {
    pub tab_id: TabId,
    pub cmd_tx: mpsc::Sender<TabCommand>,
    pub sink: Arc<TabSink>,
}

impl std::fmt::Debug for TabHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TabHandle")
            .field("tab_id", &self.tab_id)
            .finish()
    }
}

impl TabHandle {
    pub async fn send(&self, cmd: TabCommand) -> Result<(), EngineError> {
        self.cmd_tx
            .send(cmd)
            .await
            .map_err(|_| EngineError::ChannelClosed)?;
        Ok(())
    }

    pub async fn set_title(&self, title: impl Into<String>) -> Result<(), EngineError> {
        self.send(TabCommand::SetTitle {
            title: title.into(),
        })
        .await
    }

    pub async fn set_viewport(&self, viewport: Viewport) -> Result<(), EngineError> {
        self.send(TabCommand::SetViewport {
            x: viewport.x,
            y: viewport.y,
            width: viewport.width,
            height: viewport.height,
        })
        .await
    }

    pub async fn navigate(&self, url: impl Into<String>) -> Result<(), EngineError> {
        self.send(TabCommand::Navigate { url: url.into() }).await
    }
}
