mod config;
mod password_store;
mod zone;

use anyhow::Result;
use tokio::sync::mpsc::Sender;
use tokio::sync::oneshot;
use crate::engine::events::{EngineCommand, EngineEvent, ZoneCommand};

pub use config::ZoneConfig;
pub use zone::Zone;
pub use zone::ZoneId;
pub use zone::ZoneServices;
use crate::render::Viewport;
use crate::tab::{TabHandle, TabId};

#[derive(Clone)]
pub struct ZoneHandle {
    zone: ZoneId,
    engine_cmd_tx: Sender<EngineCommand>,
}

struct ZoneInner {
    engine_event_tx: Sender<EngineEvent>,
}

impl ZoneHandle {
    pub fn new(zone: ZoneId, engine_cmd_tx: Sender<EngineCommand>) -> Self {
        Self { zone, engine_cmd_tx }
    }

    pub fn id(&self) -> ZoneId { self.zone }

    pub async fn set_title(&self, title: impl Into<String>) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.engine_cmd_tx.send(EngineCommand::Zone(ZoneCommand::SetTitle {
            zone: self.zone,
            title: title.into(),
            reply: tx,
        })).await?;
        rx.await?
    }

    pub async fn set_icon(&self, icon: Vec<u8>) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.engine_cmd_tx.send(EngineCommand::Zone(ZoneCommand::SetIcon {
            zone: self.zone,
            icon,
            reply: tx,
        })).await?;
        rx.await?
    }

    pub async fn set_description(&self, description: impl Into<String>) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.engine_cmd_tx.send(EngineCommand::Zone(ZoneCommand::SetDescription {
            zone: self.zone,
            description: description.into(),
            reply: tx,
        })).await?;
        rx.await?
    }

    pub async fn set_color(&self, color: [u8; 4]) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.engine_cmd_tx.send(EngineCommand::Zone(ZoneCommand::SetColor {
            zone: self.zone,
            color,
            reply: tx,
        })).await?;
        rx.await?
    }

    pub async fn open_tab(&self, title: impl Into<String>, viewport: Viewport) -> Result<TabHandle> {
        let (tx, rx) = oneshot::channel();
        self.engine_cmd_tx.send(EngineCommand::Zone(ZoneCommand::OpenTab {
            zone: self.zone,
            title: title.into(),
            viewport,
            reply: tx,
        })).await?;
        rx.await?
    }

    pub async fn close_tab(&self, tab: TabId) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.engine_cmd_tx.send(EngineCommand::Zone(ZoneCommand::CloseTab {
            zone: self.zone,
            tab,
            reply: tx,
        })).await?;
        rx.await?
    }

    pub async fn list_tabs(&self) -> Result<Vec<TabId>> {
        let (tx, rx) = oneshot::channel();
        self.engine_cmd_tx.send(EngineCommand::Zone(ZoneCommand::ListTabs {
            zone: self.zone,
            reply: tx,
        })).await?;
        rx.await?
    }

    pub async fn tab_title(&self, tab: TabId) -> Result<Option<String>> {
        let (tx, rx) = oneshot::channel();
        self.engine_cmd_tx.send(EngineCommand::Zone(ZoneCommand::TabTitle {
            zone: self.zone,
            tab,
            reply: tx,
        })).await?;
        rx.await?
    }
}