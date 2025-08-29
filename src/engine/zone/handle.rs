use anyhow::Result;
use tokio::sync::mpsc::Sender;
use tokio::sync::oneshot;
use crate::engine::events::{EngineCommand, ZoneCommand};

use crate::render::Viewport;
use crate::tab::{TabHandle, TabId};
use crate::zone::ZoneId;

#[derive(Clone)]
pub struct ZoneHandle {
    zone: ZoneId,
    cmd_tx: Sender<EngineCommand>,
}

impl ZoneHandle {
    pub fn new(zone: ZoneId, cmd_tx: Sender<EngineCommand>) -> Self {
        Self { zone, cmd_tx }
    }

    pub fn id(&self) -> ZoneId { self.zone }

    pub async fn set_title(&self, title: impl Into<String>) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.cmd_tx.send(EngineCommand::Zone(ZoneCommand::SetTitle {
            zone: self.zone,
            title: title.into(),
            reply: tx,
        })).await?;
        rx.await?
    }

    pub async fn set_icon(&self, icon: Vec<u8>) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.cmd_tx.send(EngineCommand::Zone(ZoneCommand::SetIcon {
            zone: self.zone,
            icon,
            reply: tx,
        })).await?;
        rx.await?
    }

    pub async fn set_description(&self, description: impl Into<String>) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.cmd_tx.send(EngineCommand::Zone(ZoneCommand::SetDescription {
            zone: self.zone,
            description: description.into(),
            reply: tx,
        })).await?;
        rx.await?
    }

    pub async fn set_color(&self, color: [u8; 4]) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.cmd_tx.send(EngineCommand::Zone(ZoneCommand::SetColor {
            zone: self.zone,
            color,
            reply: tx,
        })).await?;
        rx.await?
    }

    pub async fn open_tab(&self, title: impl Into<String>, viewport: Viewport) -> Result<TabHandle> {
        let (tx, rx) = oneshot::channel();
        self.cmd_tx.send(EngineCommand::Zone(ZoneCommand::OpenTab {
            zone: self.zone,
            title: Some(title.into()),
            viewport: Some(viewport),
            url: None,
            reply: tx,
        })).await?;
        rx.await?
    }

    pub async fn close_tab(&self, tab: TabId) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.cmd_tx.send(EngineCommand::Zone(ZoneCommand::CloseTab {
            zone: self.zone,
            tab,
            reply: tx,
        })).await?;
        rx.await?
    }

    pub async fn list_tabs(&self) -> Result<Vec<TabId>> {
        let (tx, rx) = oneshot::channel();
        self.cmd_tx.send(EngineCommand::Zone(ZoneCommand::ListTabs {
            zone: self.zone,
            reply: tx,
        })).await?;
        rx.await?
    }

    pub async fn tab_title(&self, tab: TabId) -> Result<Option<String>> {
        let (tx, rx) = oneshot::channel();
        self.cmd_tx.send(EngineCommand::Zone(ZoneCommand::TabTitle {
            zone: self.zone,
            tab,
            reply: tx,
        })).await?;
        rx.await?
    }
}