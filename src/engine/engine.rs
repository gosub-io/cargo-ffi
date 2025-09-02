use std::collections::HashMap;
use crate::render::backend::RenderBackend;
use crate::zone::{Zone, ZoneConfig, ZoneHandle, ZoneId, ZoneServices};
use crate::{EngineConfig, EngineError};
use std::sync::{Arc, RwLock};
use tokio::sync::{mpsc, broadcast};
use crate::engine::events::{EngineCommand, EngineEvent, ZoneCommand};
use anyhow::Result;
use tokio::task::JoinHandle;
use crate::engine::DEFAULT_CHANNEL_CAPACITY;
use crate::engine::handle::EngineHandle;
use crate::tab::{EffectiveTabServices, TabDefaults, TabHandle, TabOverrides};
use crate::tab::services::resolve_tab_services;

#[allow(unused)]
pub struct GosubEngine {
    /// Configuration for the whole engine.
    config: Arc<EngineConfig>,
    /// Active render backend for the engine.
    backend: Arc<RwLock<Box<dyn RenderBackend + Send + Sync>>>,
    /// Zones managed by this engine, indexed by [`ZoneId`].
    zones: RwLock<HashMap<ZoneId, Arc<Zone>>>,
    // /// Command sender (cloned into handles).
    cmd_tx: mpsc::Sender<EngineCommand>,
    /// Command receiver (owned by the engine run loop).
    cmd_rx: mpsc::Receiver<EngineCommand>,
    /// Event sender
    event_tx: broadcast::Sender<EngineEvent>,
    /// Is the engine running?
    running: bool,
    /// Join handle when the event loop is spawned
    join_handle: Option<tokio::task::JoinHandle<()>>,
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

        // Command channel on which to send and receive engine commands from the UA.
        let (cmd_tx, cmd_rx) = mpsc::channel::<EngineCommand>(DEFAULT_CHANNEL_CAPACITY);

        // Broadcast event bus. Subscribe to receive engine events (including zone and tab events)
        let (event_tx, _first_rx) = broadcast::channel::<EngineEvent>(DEFAULT_CHANNEL_CAPACITY);

        Self {
            config: Arc::new(resolved_config),
            backend: Arc::new(RwLock::new(backend)),
            zones: RwLock::new(HashMap::new()),
            cmd_tx,
            cmd_rx,
            event_tx,
            running: false,
            join_handle: None,
        }
    }

    // /// Create a new event channel for engine/zone → host messages.
    // ///
    // /// Returns `(Sender<EngineEvent>, Receiver<EngineEvent>)`.
    // pub fn create_event_channel(&self, cap: usize) -> (Sender<EngineEvent>, Receiver<EngineEvent>) {
    //     tokio::sync::mpsc::channel(cap)
    // }

    // pub fn start(&mut self) {
    //     self.running = true;
    //     self.join_handle = Some(tokio::spawn(self.run()));
    // }

    /// Starts the engine and returns the engine and join handle
    pub fn start(self) -> Result<(EngineHandle, JoinHandle<()>), EngineError> {
        if self.running {
            return Err(EngineError::AlreadyRunning);
        }

        let engine_handle = EngineHandle::new(self.cmd_tx.clone(), self.event_tx.clone(), self.backend.clone());
        let join_handle = tokio::spawn(self.run());

        Ok((engine_handle, join_handle))
    }


    /// Replace the active render backend.
    pub fn set_backend_renderer(&mut self, new_backend: Box<dyn RenderBackend + Send + Sync>) {
        {
            let binding = self.backend.read().unwrap();
            let old_name = binding.name();
            let _ = self.event_tx.send(EngineEvent::BackendChanged { old: old_name.to_string(), new: new_backend.name().to_string() });
        }

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
    pub async fn run(mut self) {
        self.running = true;

        let _ = self.event_tx.send(EngineEvent::EngineStarted);

        println!("Run() is started!");
        while let Some(cmd) = self.cmd_rx.recv().await {
            match cmd {
                EngineCommand::Shutdown { reply } => {
                    let res = self.shutdown_impl().await;
                    let _ = reply.send(res);
                    break;
                }
                EngineCommand::Zone(zc) => self.handle_zone_command(zc).await.unwrap(),
                _ => { /* engine-level commands can be handled here in the future */ }
            }
        }
        println!("Run() is ending!");
    }

    pub async fn shutdown_impl(&mut self) -> Result<(), EngineError> {
        // // Tabs should be closed first, then zones
        // let tab_cmds: Vec<_> = {
        //     // snapshot without holding locks across awaits
        //     let zones_guard = self.zones.read().map_err(|_| EngineError::Poisoned)?;
        //     zones_guard
        //         .values()
        //         .flat_map(|zone| zone.tabs_snapshot_handles()) // -> Vec<(TabId, mpsc::Sender<TabCommand>, Option<JoinHandle<()>>)>
        //         .collect()
        // };
        //
        // for (_tab_id, tx, _jh) in &tab_cmds {
        //     let _ = tx.send(crate::events::TabCommand::CloseTab).await;
        // }
        //
        // let mut joins = JoinSet::new();
        // for (_id, _tx, maybe_jh) in tab_cmds {
        //     if let Some(jh) = maybe_jh {
        //         joins.spawn(async move {
        //             let _ = jh.await;
        //         });
        //     }
        // }
        //
        // // Wait for a few seconds for tabs to close
        // let _ = timeout(Duration::from_secs(2), async {
        //     while let Some(_res) = joins.join_next().await {}
        // }).await;
        //
        // // Flush any outstanding cookies, storage etc
        // self.flush_persistence();

        Ok(())
    }

    fn flush_persistence(&mut self) {
        // if let Ok(zones) = self.zones.read() {
        //     for zone in zones.values() {
        //         if let Some(store) = zone.cookie_store_handle() {
        //             store.persist_all();
        //         }
        //     }
        // }
    }


    /// Dispatch a [`ZoneCommand`] to the targeted zone, replying on the provided
    /// responder channel contained in the command.
    ///
    /// Locks are held only for short, non-`await`ing critical sections to avoid
    /// holding a mutex across `.await`.
    async fn handle_zone_command(&mut self, zc: ZoneCommand) -> Result<()> {
        match zc {
            ZoneCommand::SetTitle { zone_id, title, reply } => {
                let res = (|| -> Result<()> {
                    let z = self.zone_by_id(zone_id)?;
                    z.set_title(&title);
                    Ok(())
                })();
                let _ = reply.send(res);
            }
            ZoneCommand::SetDescription { zone_id, description, reply } => {
                let res = (|| -> Result<()> {
                    let z = self.zone_by_id(zone_id)?;
                    z.set_description(&description);
                    Ok(())
                })();
                let _ = reply.send(res);
            }
            ZoneCommand::SetIcon { zone_id, icon, reply } => {
                let res = (|| -> Result<()> {
                    let z = self.zone_by_id(zone_id)?;
                    z.set_icon(icon);
                    Ok(())
                })();
                let _ = reply.send(res);
            }
            ZoneCommand::SetColor { zone_id, color, reply } => {
                let res = (|| -> Result<()> {
                    let z = self.zone_by_id(zone_id)?;
                    z.set_color(color);
                    Ok(())
                })();
                let _ = reply.send(res);
            }
            ZoneCommand::CreateTab { zone_id, initial, overrides, reply } => {
                println!("Engine: calling self.create_tab_in_zone");
                let res = match self.create_tab_in_zone(zone_id, initial, overrides).await {
                    Ok(res) => Ok(res),
                    Err(e) => Err(EngineError::CreateTab(e.into()))
                };

                println!("Engine: sending back reply");
                let _ = reply.send(res);
            }
            ZoneCommand::CloseTab { zone_id, tab_id, reply } => {
                let res = (|| -> Result<()> {
                    let z = self.zone_by_id(zone_id)?;
                    if z.close_tab(tab_id) { Ok(()) } else { anyhow::bail!("no such tab") }
                })();
                let _ = reply.send(res);
            }
            ZoneCommand::ListTabs { zone_id, reply } => {
                let res = (|| -> Result<_> {
                    let z = self.zone_by_id(zone_id)?;
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
        event_tx: broadcast::Sender<EngineEvent>
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
        initial: TabDefaults,
        overrides: Option<TabOverrides>,
    ) -> Result<TabHandle, EngineError> {
        let zone = self.zone_by_id(zone_id)?;
        let eff: EffectiveTabServices = resolve_tab_services(zone_id, &zone.services(), &overrides.unwrap_or_default());
        zone.create_tab(eff, initial).await
    }
}

#[cfg(test)]
mod tests {
    use crate::cookies::DefaultCookieJar;
    use crate::render::backends::null::NullBackend;
    use crate::storage::{InMemoryLocalStore, InMemorySessionStore, StorageService, PartitionPolicy};
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
        assert_eq!(handle.zone_id(), zone_id);
    }

    /// Demonstrate a handle call round-tripping through the engine’s command loop.
    #[tokio::test]
    async fn zone_handle_set_title_round_trips_through_engine() {
        let backend = Box::new(NullBackend::new().unwrap());
        let mut engine = GosubEngine::new(None, backend);

        let (ev_tx, _ev_rx) = engine.create_event_channel(16);

        // spawn engine loop. Do this before you send anything to the engine
        let engine_task = tokio::spawn(engine.run());


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

        // call into zone_handle
        zone_handle.set_title("Work".to_string()).await.unwrap();

        // graceful shutdown
        engine_tx.send(EngineCommand::Shutdown).await.unwrap();

        // wait for loop to end
        engine_task.await.unwrap().unwrap();
    }
}
