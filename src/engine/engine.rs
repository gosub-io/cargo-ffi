//! Engine core implementation.
//!
//! This module defines the [`GosubEngine`] struct, which is the main entry point for
//! creating and managing the engine, zones, and event bus. It also provides the
//! [`EngineContext`] struct for sharing resources and configuration across the engine.
//!
//! # Overview
//!
//! The engine is responsible for running zones and handling events. It provides a
//! command interface for starting, stopping, and configuring zones, as well as
//! subscribing to events from the engine and zones.
//!
//! # Main Types
//!
//! - [`GosubEngine`]: The main engine struct.
//! - [`EngineContext`]: Shared context for the engine, containing configuration and
//!   backend information.
//! - [`Zone`]: Represents a zone managed by the engine.
//! - [`EngineCommand`]: Commands that can be sent to the engine.
//! - [`EngineEvent`]: Events emitted by the engine, such as zone creation and
//!   destruction.

use crate::engine::events::{EngineCommand, EngineEvent};
use crate::engine::DEFAULT_CHANNEL_CAPACITY;
use crate::render::backend::RenderBackend;
use crate::zone::{Zone, ZoneConfig, ZoneId, ZoneServices, ZoneSink};
use crate::{EngineConfig, EngineError};
use anyhow::Result;
use std::borrow::BorrowMut;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use tokio::sync::{broadcast, mpsc};
use tokio::task::JoinHandle;

pub struct GosubEngine {
    /// Context is what can be shared downstream
    context: Arc<EngineContext>,
    /// Zones managed by this engine, indexed by [`ZoneId`].
    zones: HashMap<ZoneId, Arc<ZoneSink>>,
    /// Command sender used to send commands to the engine run loop.
    cmd_tx: mpsc::Sender<EngineCommand>,
    /// Command receiver (owned by the engine run loop).
    cmd_rx: Option<mpsc::Receiver<EngineCommand>>,
    /// Is the engine running?
    running: bool,
}

// Engine context that is shared downwards to zones.
#[derive(Clone)]
pub struct EngineContext {
    /// Active render backend for the engine.
    pub backend: Arc<RwLock<Box<dyn RenderBackend + Send + Sync>>>,
    /// Event sender
    pub event_tx: broadcast::Sender<EngineEvent>,
    /// Global engine configuration
    pub config: Arc<EngineConfig>,
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
            context: Arc::new(EngineContext {
                backend: Arc::new(RwLock::new(backend)),
                event_tx: event_tx.clone(),
                config: Arc::new(resolved_config),
            }),
            zones: HashMap::new(),
            cmd_tx,
            cmd_rx: Some(cmd_rx),
            running: false,
        }
    }

    /// Starts the engine and returns the join handle of the main run loop task.
    pub fn start(&mut self) -> Result<Option<JoinHandle<()>>, EngineError> {
        if self.running {
            return Err(EngineError::AlreadyRunning);
        }

        let join_handle = if let Some(task) = self.run() {
            Some(
                tokio::task::Builder::new()
                    .name("Engine runner")
                    .spawn(task)
                    .map_err(|e| EngineError::Internal(e.into()))?,
            )
        } else {
            None
        };

        Ok(join_handle)
    }

    /// Return a receiver for engine events.
    pub fn subscribe_events(&self) -> broadcast::Receiver<EngineEvent> {
        self.context.event_tx.subscribe()
    }

    /// Replace the active render backend.
    pub fn set_backend_renderer(&mut self, new_backend: Box<dyn RenderBackend + Send + Sync>) {
        {
            let binding = self.context.backend.read().unwrap();
            let old_name = binding.name();
            let _ = self.context.event_tx.send(EngineEvent::BackendChanged {
                old: old_name.to_string(),
                new: new_backend.name().to_string(),
            });
        }

        let binding = self.context.borrow_mut();
        let mut backend = binding.backend.write().unwrap();
        *backend = new_backend;
    }

    /// Get a clone of the engine’s command sender (mainly for testing or
    /// custom handles).
    #[cfg(test)]
    fn command_sender(&self) -> mpsc::Sender<EngineCommand> {
        self.cmd_tx.clone()
    }

    /// Run the engine’s inbound command loop in a dedicated thread/task.
    pub fn run<'a, 'b>(&'a mut self) -> Option<impl std::future::Future<Output = ()> + 'b> {
        self.running = true;

        println!("Sending engine started event");
        let _ = self.context.event_tx.send(EngineEvent::EngineStarted);

        let mut cmd_rx = self.cmd_rx.take()?;

        Some(async move {
            while let Some(cmd) = cmd_rx.recv().await {
                match cmd {
                    EngineCommand::Shutdown { reply } => {
                        println!("Engine received shutdown command. Shutting down main engine::run() loop");
                        let _ = reply.send(Ok(()));
                        break;
                    }
                    _ => {
                        unimplemented!("unhandled engine command: {:?}", cmd);
                    }
                }
            }
            println!("run() loop has exited")
        })
    }

    /// Shuts down the engine (will not take of zones and tabs at the moment)
    pub async fn shutdown(&mut self) -> Result<(), EngineError> {
        if !self.running {
            return Err(EngineError::NotRunning);
        }

        // Send shutdown command to the run loop
        let (tx, rx) = tokio::sync::oneshot::channel();
        let _ = self.cmd_tx.try_send(EngineCommand::Shutdown { reply: tx });

        // Wait for confirmation that the run loop has exited
        let _ = rx.await.map_err(|e| EngineError::Internal(e.into()))?;

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
        // // Flush any outstanding cookies, storage etc.
        // self.flush_persistence();

        // self.context.event_tx.send(EngineEvent::EngineShutdown { reason: reason.into() }).map_err(|_| EngineError::Internal)?;

        Ok(())
    }

    #[allow(unused)]
    fn flush_persistence(&mut self) {
        // if let Ok(zones) = self.zones.read() {
        //     for zone in zones.values() {
        //         if let Some(store) = zone.cookie_store_handle() {
        //             store.persist_all();
        //         }
        //     }
        // }
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
    ) -> Result<Zone, EngineError> {
        let zone = match zone_id {
            Some(zone_id) => Zone::new_with_id(zone_id, config, services, self.context.clone()),
            None => Zone::new(config, services, self.context.clone()),
        };

        let zone_id = zone.id;
        self.zones.insert(zone.id, zone.sink.clone());

        self.context
            .event_tx
            .send(EngineEvent::ZoneCreated { zone_id })
            .map_err(|e| EngineError::Internal(e.into()))?;

        Ok(zone)
    }
}
