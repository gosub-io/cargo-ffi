//! Engine core: zones, commands, and run loop.
//!
//! The **host application / UA** creates a [`GosubEngine`] by providing a
//! render backend and an engine-wide [`EngineConfig`]. The engine owns a
//! command channel (`cmd_tx`, `cmd_rx`) used by handles (e.g. [`ZoneHandle`])
//! to send control messages into the engine.
//!
//! The engine’s [`run`](GosubEngine::run) loop awaits inbound
//! [`EngineCommand`]s. When it receives a zone command, it dispatches to
//! [`handle_zone_command`](GosubEngine::handle_zone_command) which finds the
//! target [`Zone`] and performs the requested action.
//!
//! New zones are created via [`GosubEngine::create_zone`]. You pass a
//! [`ZoneConfig`], a pre-assembled [`ZoneServices`] bundle (cookies, storage,
//! etc.), an optional [`ZoneId`] (or let the engine generate one), and an
//! `event_tx` channel where the zone (and its tabs) can emit [`EngineEvent`]s
//! back to the host. The function returns a [`ZoneHandle`] (just the id +
//! a clone of the engine’s `cmd_tx`) that userland code can use to control the
//! zone asynchronously.

use std::collections::HashMap;
use crate::render::backend::RenderBackend;
use crate::zone::{Zone, ZoneConfig, ZoneHandle, ZoneId, ZoneServices};
use crate::{EngineConfig, EngineError};
use std::sync::{Arc, RwLock};
use tokio::sync::mpsc::{Receiver, Sender};
use crate::engine::events::{EngineCommand, EngineEvent, ZoneCommand};
use anyhow::Result;
use crate::engine::DEFAULT_CHANNEL_CAPACITY;
use crate::render::Viewport;
use crate::tab::{OpenTabParams, TabHandle};

/// Entry point to the Gosub engine.
///
/// Typical usage:
///
/// ```
/// # use gosub_engine as ge;
/// let backend = ge::render::backends::null::NullBackend::new()
///     .expect("null renderer cannot be created");
/// let config = ge::EngineConfig::default();
/// let engine = ge::GosubEngine::new(Some(config), Box::new(backend));
/// ```
pub struct GosubEngine {
    /// Configuration for the whole engine.
    config: Arc<EngineConfig>,
    /// Active render backend for the engine.
    backend: Arc<RwLock<Box<dyn RenderBackend + Send + Sync>>>,
    /// Zones managed by this engine, indexed by [`ZoneId`].
    zones: RwLock<HashMap<ZoneId, Arc<Zone>>>,
    /// Command sender (cloned into handles).
    cmd_tx: Sender<EngineCommand>,
    /// Command receiver (owned by the engine run loop).
    cmd_rx: Receiver<EngineCommand>,
}

impl GosubEngine {
    /// Create a new engine.
    ///
    /// If `config` is `None`, [`EngineConfig::default`] is used.
    ///
    /// ```
    /// # use gosub_engine as ge;
    /// let backend = ge::render::backends::null::NullBackend::new().unwrap();
    /// let engine = ge::GosubEngine::new(None, Box::new(backend));
    /// ```
    pub fn new(config: Option<EngineConfig>, backend: Box<dyn RenderBackend + Send + Sync>) -> Self {
        let resolved_config = config.unwrap_or_else(EngineConfig::default);
        let (cmd_tx, cmd_rx) = tokio::sync::mpsc::channel(DEFAULT_CHANNEL_CAPACITY);

        Self {
            config: Arc::new(resolved_config),
            backend: Arc::new(RwLock::new(backend)),
            zones: RwLock::new(HashMap::new()),
            cmd_tx,
            cmd_rx,
        }
    }

    /// Create a new event channel for engine/zone → host messages.
    ///
    /// Returns `(Sender<EngineEvent>, Receiver<EngineEvent>)`.
    pub fn create_event_channel(&self, cap: usize) -> (Sender<EngineEvent>, Receiver<EngineEvent>) {
        tokio::sync::mpsc::channel(cap)
    }

    // pub fn engine_handle(&self) -> EngineHandle {
    //     EngineHandle::new(
    //         self.cmd_tx.clone(),
    //         self.backend.clone()
    //     )
    // }

    /// Replace the active render backend.
    pub fn set_backend_renderer(&mut self, new_backend: Box<dyn RenderBackend + Send + Sync>) {
        self.backend = Arc::new(RwLock::new(new_backend));
    }

    /// Get a clone of the engine’s command sender (mainly for testing or
    /// custom handles).
    #[cfg(test)]
    fn command_sender(&self) -> Sender<EngineCommand> {
        self.cmd_tx.clone()
    }

    /// Run the engine’s inbound command loop.
    ///
    /// This awaits messages from handles (e.g., [`ZoneHandle`]) and dispatches
    /// zone-related commands through [`handle_zone_command`](Self::handle_zone_command).
    /// The loop ends when all senders are dropped and the channel closes.
    pub async fn run(mut self) -> Result<()> {
        while let Some(cmd) = self.cmd_rx.recv().await {
            match cmd {
                EngineCommand::Shutdown => break, // graceful stop
                EngineCommand::Zone(zc) => self.handle_zone_command(zc).await?,
                _ => { /* engine-level commands can be handled here in the future */ }
            }
        }
        Ok(())
    }

    /// Dispatch a [`ZoneCommand`] to the targeted zone, replying on the provided
    /// responder channel contained in the command.
    ///
    /// Locks are held only for short, non-`await`ing critical sections to avoid
    /// holding a mutex across `.await`.
    async fn handle_zone_command(&mut self, zc: ZoneCommand) -> Result<()> {
        match zc {
            ZoneCommand::SetTitle { zone, title, reply } => {
                let res = (|| -> Result<()> {
                    let mut z = self.zone_by_id(zone)?;
                    z.set_title(&title);
                    Ok(())
                })();
                let _ = reply.send(res);
            }
            ZoneCommand::SetDescription { zone, description, reply } => {
                let res = (|| -> Result<()> {
                    let mut z = self.zone_by_id(zone)?;
                    z.set_description(&description);
                    Ok(())
                })();
                let _ = reply.send(res);
            }
            ZoneCommand::SetIcon { zone, icon, reply } => {
                let res = (|| -> Result<()> {
                    let mut z = self.zone_by_id(zone)?;
                    z.set_icon(icon);
                    Ok(())
                })();
                let _ = reply.send(res);
            }
            ZoneCommand::SetColor { zone, color, reply } => {
                let res = (|| -> Result<()> {
                    let mut z = self.zone_by_id(zone)?;
                    z.set_color(color);
                    Ok(())
                })();
                let _ = reply.send(res);
            }
            ZoneCommand::CreateTab { zone, title, viewport, url, reply } => {
                let res = self.create_tab_in_zone(zone, title, viewport, url).await;
                let _ = reply.send(res);
            }
            ZoneCommand::CloseTab { zone, tab, reply } => {
                let res = (|| -> Result<()> {
                    let z = self.zone_by_id(zone)?;
                    if z.close_tab(tab) { Ok(()) } else { anyhow::bail!("no such tab") }
                })();
                let _ = reply.send(res);
            }
            ZoneCommand::ListTabs { zone, reply } => {
                let res = (|| -> Result<_> {
                    let z = self.zone_by_id(zone)?;
                    Ok(z.list_tabs())
                })();
                let _ = reply.send(res);
            }
            // ZoneCommand::TabTitle { zone, tab, reply } => {
            //     let res = (|| -> Result<_> {
            //         let z = self.zone_by_id(zone)?;
            //         Ok(z.tab_title(tab))
            //     })();
            //     let _ = reply.send(res);
            // }
        }
        Ok(())
    }

    /// Create and register a new zone, returning a [`ZoneHandle`] for userland code.
    ///
    /// - `config`: zone configuration (features, limits, identity)
    /// - `services`: storage, cookie store/jar, partition policy, etc.
    /// - `zone_id`: optional id; if `None`, a fresh one is generated
    /// - `event_tx`: channel where the zone (and its tabs) will emit [`EngineEvent`]s
    ///
    /// The returned handle contains the [`ZoneId`] and a clone of the engine’s
    /// command sender, allowing the caller to send zone commands without holding
    /// a reference to the engine.
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
        self.zones.write().unwrap().insert(zone.id, Arc::new(zone));

        Ok(ZoneHandle::new(zone_id, self.cmd_tx.clone()))
    }

    #[inline]
    fn zone_by_id(&self, id: ZoneId) -> Result<Arc<Zone>, EngineError> {
        let guard = self.zones.read().map_err(|_| EngineError::Poisoned)?;
        guard.get(&id).cloned().ok_or(EngineError::ZoneNotFound)
    }

    async fn create_tab_in_zone(
        &self,
        zone_id: ZoneId,
        title: Option<String>,
        viewport: Option<Viewport>,
        url: Option<String>,
    ) -> Result<TabHandle, EngineError> {
        let zone = self
            .zones
            .read().unwrap()
            .get(&zone_id)
            .cloned()
            .ok_or(EngineError::ZoneNotFound)?;

        // Build your open params; adjust names to match your struct
        let params = OpenTabParams {
            title,
            viewport,
            url,
            ..Default::default()
        };

        // This calls Zone::create_tab(..) which does the spawn + ack oneshot internally
        zone.create_tab(params).await
    }
}

#[cfg(test)]
mod tests {
    use crate::cookies::DefaultCookieJar;
    use crate::render::backends::null::NullBackend;
    use crate::storage::{InMemoryLocalStore, InMemorySessionStore, StorageService};
    use crate::storage::types::PartitionPolicy;
    use super::*;

    /// Ensure `create_zone` returns a handle and registers the zone internally.
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
            storage: storage.clone(),
            cookie_store: None,     // No cookie store needed; we do not persist cookies here
            cookie_jar: Some(cookie_jar.into()),
            partition_policy: PartitionPolicy::TopLevelOrigin,
        };

        let cfg = ZoneConfig::default();
        let handle = engine.create_zone(cfg, services, Some(zone_id), ev_tx).unwrap();
        assert_eq!(handle.id(), zone_id);
    }

    /// Demonstrate a handle call round-tripping through the engine’s command loop.
    #[tokio::test]
    async fn zone_handle_set_title_round_trips_through_engine() {
        let backend = Box::new(NullBackend::new().unwrap());
        let mut engine = GosubEngine::new(None, backend);

        let (ev_tx, _ev_rx) = engine.create_event_channel(16);

        let cookie_jar = DefaultCookieJar::new();
        let storage = Arc::new(StorageService::new(
            Arc::new(InMemoryLocalStore::new()),
            Arc::new(InMemorySessionStore::new()),
        ));

        // stub services
        let services = ZoneServices {
            storage: storage.clone(),
            cookie_store: None,
            cookie_jar: Some(cookie_jar.into()),
            partition_policy: PartitionPolicy::TopLevelOrigin,
        };

        let cfg = ZoneConfig::default();
        let zone_handle = engine.create_zone(cfg, services, None, ev_tx).unwrap();

        let engine_tx = engine.command_sender().clone();

        // spawn engine loop
        let engine_task = tokio::spawn(engine.run());

        // call into zone_handle
        zone_handle.set_title("Work".to_string()).await.unwrap();

        // graceful shutdown
        engine_tx.send(EngineCommand::Shutdown).await.unwrap();

        // wait for loop to end
        engine_task.await.unwrap().unwrap();
    }
}
