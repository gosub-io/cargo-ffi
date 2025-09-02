use anyhow::Result;
use tokio::sync::mpsc::Sender;
use tokio::sync::oneshot;
use crate::engine::events::{EngineCommand, ZoneCommand};
use crate::EngineError;
use crate::tab::{TabDefaults, TabHandle, TabId, TabOverrides};
use crate::zone::ZoneId;

#[derive(Clone)]
pub struct ZoneHandle {
    zone_id: ZoneId,
    cmd_tx: Sender<EngineCommand>,
}

impl ZoneHandle {
    pub fn new(zone_id: ZoneId, cmd_tx: Sender<EngineCommand>) -> Self {
        Self { zone_id, cmd_tx }
    }

    pub fn zone_id(&self) -> ZoneId { self.zone_id }

    pub async fn set_title(&self, title: impl Into<String>) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.cmd_tx.send(EngineCommand::Zone(ZoneCommand::SetTitle {
            zone_id: self.zone_id,
            title: title.into(),
            reply: tx,
        })).await?;
        rx.await?
    }

    pub async fn set_icon(&self, icon: Vec<u8>) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.cmd_tx.send(EngineCommand::Zone(ZoneCommand::SetIcon {
            zone_id: self.zone_id,
            icon,
            reply: tx,
        })).await?;
        rx.await?
    }

    pub async fn set_description(&self, description: impl Into<String>) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.cmd_tx.send(EngineCommand::Zone(ZoneCommand::SetDescription {
            zone_id: self.zone_id,
            description: description.into(),
            reply: tx,
        })).await?;
        rx.await?
    }

    pub async fn set_color(&self, color: [u8; 4]) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.cmd_tx.send(EngineCommand::Zone(ZoneCommand::SetColor {
            zone_id: self.zone_id,
            color,
            reply: tx,
        })).await?;
        rx.await?
    }

    pub async fn create_tab(&self, initial: TabDefaults, overrides: Option<TabOverrides>) -> Result<TabHandle, EngineError> {
        let (tx, rx) = oneshot::channel::<Result<TabHandle, EngineError>>();
        self.cmd_tx.send(EngineCommand::Zone(ZoneCommand::CreateTab {
            zone_id: self.zone_id,
            initial,
            overrides,
            reply: tx,
        })).await
            .map_err(|_| EngineError::ChannelClosed)?;

        match rx.await {
            Ok(res) => res,
            Err(e) => Err(EngineError::CreateTab(e.to_string()))
        }
    }

    pub async fn close_tab(&self, tab_id: TabId) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.cmd_tx.send(EngineCommand::Zone(ZoneCommand::CloseTab {
            zone_id: self.zone_id,
            tab_id,
            reply: tx,
        })).await?;
        rx.await?
    }

    pub async fn list_tabs(&self) -> Result<Vec<TabId>> {
        let (tx, rx) = oneshot::channel();
        self.cmd_tx.send(EngineCommand::Zone(ZoneCommand::ListTabs {
            zone_id: self.zone_id,
            reply: tx,
        })).await?;
        rx.await?
    }

    // pub async fn tab_title(&self, tab: TabId) -> Result<Option<String>> {
    //     let (tx, rx) = oneshot::channel();
    //     self.cmd_tx.send(EngineCommand::Zone(ZoneCommand::TabTitle {
    //         zone_id: self.zone_id,
    //         tab,
    //         reply: tx,
    //     })).await?;
    //     rx.await?
    // }
}