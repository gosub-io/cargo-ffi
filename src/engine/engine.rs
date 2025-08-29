use std::collections::HashMap;
use crate::render::backend::{RenderBackend};
use crate::zone::{Zone, ZoneConfig, ZoneHandle, ZoneId, ZoneServices};
use crate::EngineConfig;
use std::sync::{Arc, Mutex};
use tokio::runtime::Runtime;
use tokio::sync::mpsc::{Receiver, Sender};
use crate::engine::events::{EngineCommand, EngineEvent, ZoneCommand};
use anyhow::Result;

/// Entry point to the Gosub engine.
///
/// Create an engine, then create zones and open tabs.
///
/// See [`Viewport`], [`ZoneId`], [`TabId`], [`EngineEvent`], [`EngineCommand`].
pub struct GosubEngine {
    /// Configuration for the whole engine
    config: Arc<EngineConfig>,
    /// Tokio runtime for async operations
    pub runtime: Arc<Runtime>,
    // (Current) render backend for the engine
    backend: Box<dyn RenderBackend + Send + Sync>,

    zones: HashMap<ZoneId, Arc<Mutex<Zone>>>,
    cmd_tx: Sender<EngineCommand>,
    cmd_rx: Receiver<EngineCommand>,
}

impl GosubEngine {
    /// Create a new engine.
    ///
    /// If `config` is `None`, defaults are used.
    ///
    /// ```
    /// let backend = gosub_engine::render::backends::null::NullBackend::new().expect("null renderer cannot be created (!?)");
    /// let config = gosub_engine::EngineConfig::default();
    /// let engine = gosub_engine::GosubEngine::new(Some(config), Box::new(backend));
    /// ```
    pub fn new(config: Option<EngineConfig>, backend: Box<dyn RenderBackend + Send + Sync>) -> Self {
        let runtime = Arc::new(
            tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .expect("Failed to create Tokio runtime"),
        );

        let resolved_config = config.unwrap_or_else(EngineConfig::default);

        let (cmd_tx, cmd_rx)
            = tokio::sync::mpsc::channel(512);

        Self {
            config: Arc::new(resolved_config),
            runtime,
            backend,
            zones: HashMap::new(),
            cmd_tx,
            cmd_rx,
        }
    }

    pub fn create_event_channel(&self, cap: usize) -> (Sender<EngineEvent>, Receiver<EngineEvent>) {
        tokio::sync::mpsc::channel(cap)
    }

    pub fn set_backend_renderer(&mut self, new_backend: Box<dyn RenderBackend + Send + Sync>) {
        self.backend = new_backend;
    }

    pub fn command_sender(&self) -> Sender<EngineCommand> {
        self.cmd_tx.clone()
    }

    /// Pump the engine's inbound command loop.
    pub async fn run(mut self) -> Result<()> {
        while let Some(cmd) = self.cmd_rx.recv().await {
            match cmd {
                EngineCommand::Zone(zc) => self.handle_zone_command(zc).await?,
                _ => {}
            }
        }
        Ok(())
    }

    async fn handle_zone_command(&mut self, zc: ZoneCommand) -> Result<()> {
        match zc {
            ZoneCommand::SetTitle { zone, title, reply } => {
                let res = (|| -> Result<()> {
                    let z = self.zones.get(&zone).ok_or_else(|| anyhow::anyhow!("unknown zone"))?;
                    let mut z = z.lock().map_err(|_| anyhow::anyhow!("zone lock poisoned"))?;
                    z.set_title(&title);
                    Ok(())
                })();
                let _ = reply.send(res);
            }
            ZoneCommand::SetDescription { zone, description, reply } => {
                let res = (|| -> Result<()> {
                    let z = self.zones.get(&zone).ok_or_else(|| anyhow::anyhow!("unknown zone"))?;
                    let mut z = z.lock().map_err(|_| anyhow::anyhow!("zone lock poisoned"))?;
                    z.set_description(&description);
                    Ok(())
                })();
                let _ = reply.send(res);
            }
            ZoneCommand::SetIcon { zone, icon, reply } => {
                let res = (|| -> Result<()> {
                    let z = self.zones.get(&zone).ok_or_else(|| anyhow::anyhow!("unknown zone"))?;
                    let mut z = z.lock().map_err(|_| anyhow::anyhow!("zone lock poisoned"))?;
                    z.set_icon(icon);
                    Ok(())
                })();
                let _ = reply.send(res);
            }
            ZoneCommand::SetColor { zone, color, reply } => {
                let res = (|| -> Result<()> {
                    let z = self.zones.get(&zone).ok_or_else(|| anyhow::anyhow!("unknown zone"))?;
                    let mut z = z.lock().map_err(|_| anyhow::anyhow!("zone lock poisoned"))?;
                    z.set_color(color);
                    Ok(())
                })();
                let _ = reply.send(res);
            }
            ZoneCommand::OpenTab { zone, title, viewport, reply } => {
                let res = (|| -> Result<_> {
                    let z = self.zones.get(&zone).ok_or_else(|| anyhow::anyhow!("unknown zone"))?;
                    let z = z.lock().map_err(|_| anyhow::anyhow!("zone lock poisoned"))?;
                    z.create_tab(title, viewport).map_err(Into::into)
                })();
                let _ = reply.send(res);
            }
            ZoneCommand::CloseTab { zone, tab, reply } => {
                let res = (|| -> Result<()> {
                    let z = self.zones.get(&zone).ok_or_else(|| anyhow::anyhow!("unknown zone"))?;
                    let z = z.lock().map_err(|_| anyhow::anyhow!("zone lock poisoned"))?;
                    if z.close_tab(tab) { Ok(()) } else { anyhow::bail!("no such tab") }
                })();
                let _ = reply.send(res);
            }
            ZoneCommand::ListTabs { zone, reply } => {
                let res = (|| -> Result<_> {
                    let z = self.zones.get(&zone).ok_or_else(|| anyhow::anyhow!("unknown zone"))?;
                    let z = z.lock().map_err(|_| anyhow::anyhow!("zone lock poisoned"))?;
                    Ok(z.list_tabs())
                })();
                let _ = reply.send(res);
            }
            ZoneCommand::TabTitle { zone, tab, reply } => {
                let res = (|| -> Result<_> {
                    let z = self.zones.get(&zone).ok_or_else(|| anyhow::anyhow!("unknown zone"))?;
                    let z = z.lock().map_err(|_| anyhow::anyhow!("zone lock poisoned"))?;
                    Ok(z.tab_title(tab))
                })();
                let _ = reply.send(res);
            }
        }
        Ok(())
    }

    pub fn create_zone(
        &mut self,
        config: ZoneConfig,
        services: ZoneServices,
        zone_id: Option<ZoneId>,
        event_tx: Sender<EngineEvent>
    ) -> Result<ZoneHandle> {
        let zone = match zone_id {
            Some(zone_id) => Zone::new_with_id(zone_id, config, services, event_tx.clone()),
            None => Zone::new(config, services, event_tx.clone()),
        };

        let zone_id = zone.id;
        self.zones.insert(zone.id, Arc::new(Mutex::new(zone)));

        Ok(ZoneHandle::new(zone_id, self.cmd_tx.clone()))
    }
}


#[cfg(test)]
mod tests {
    use std::time::Duration;
    use tokio::time::timeout;
    use crate::cookies::{DefaultCookieJar, InMemoryCookieStore};
    use crate::render::backends::null::NullBackend;
    use crate::storage::{InMemoryLocalStore, InMemorySessionStore, StorageService};
    use crate::storage::types::PartitionPolicy;
    use super::*;

    #[tokio::test]
    async fn create_zone_returns_handle_and_registers_zone() {
        let backend = Box::new(NullBackend::new().unwrap());
        let mut engine = GosubEngine::new(None, backend);

        // events out (unused in this test)
        let (ev_tx, _ev_rx) = engine.create_event_channel(16);

        let cookie_jar = DefaultCookieJar::new();
        let storage = Arc::new(StorageService::new(
            Arc::new(InMemoryLocalStore::new()),
            Arc::new(InMemorySessionStore::new()),
        ));

        let zone_id = ZoneId::new();

        // stub services
        let services = ZoneServices {
            zone_id,
            storage: storage.clone(),
            cookie_store: None,
            cookie_jar: Some(cookie_jar),
            partition_policy: PartitionPolicy::TopLevelOrigin,
        };

        let cfg = ZoneConfig::default();
        let handle = engine.create_zone(cfg, services, Some(zone_id), ev_tx).unwrap();
        assert_eq!(handle.id(), zone_id);
    }

    #[tokio::test]
    async fn zonehandle_set_title_round_trips_through_engine() {
        let backend = Box::new(NullBackend::new().unwrap());
        let mut engine = GosubEngine::new(None, backend);

        let (ev_tx, _ev_rx) = engine.create_event_channel(16);

        let cookie_store = InMemoryCookieStore::new();
        let storage = Arc::new(StorageService::new(
            Arc::new(InMemoryLocalStore::new()),
            Arc::new(InMemorySessionStore::new()),
        ));

        // stub services
        let services = ZoneServices {
            zone_id: ZoneId::new(),
            storage: storage.clone(),
            cookie_store: Some(cookie_store),
            cookie_jar: None,
            partition_policy: PartitionPolicy::TopLevelOrigin,
        };

        let cfg = ZoneConfig::default();
        let handle = engine.create_zone(cfg, services, None, ev_tx).unwrap();

        // spawn engine loop
        let engine_task = tokio::spawn(engine.run());

        // call into handle
        handle.set_title("Work".to_string()).await.unwrap();

        // To assert the title changed, access engine internals would require
        // exposing a read API or listening to an event (recommended). For now, we just stop.
        // You can add an EngineEvent::ZoneTitleChanged and assert via ev_rx.

        // shut down engine loop by dropping sender or implement a shutdown command
        drop(handle);
        // cancel the engine task after a short delay
        let _ = timeout(Duration::from_millis(50), engine_task).await;
    }
}