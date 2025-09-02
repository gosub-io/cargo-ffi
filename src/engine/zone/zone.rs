use crate::engine::cookies::CookieJarHandle;
use crate::engine::storage::{StorageService, Subscription};
use crate::engine::tab::TabId;
use crate::EngineError;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::{Debug, Display};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{broadcast, oneshot, mpsc};
use tokio::task::JoinHandle;
use tokio::time::timeout;
use uuid::Uuid;
use crate::cookies::CookieStoreHandle;
use crate::engine::DEFAULT_CHANNEL_CAPACITY;
use crate::engine::engine::SharedEngineState;
use crate::engine::events::EngineEvent;
use crate::events::TabCommand;
use crate::storage::types::PartitionPolicy;
use crate::tab::{spawn_tab_task, EffectiveTabServices, TabDefaults, TabHandle, TabSpawnArgs};
use crate::zone::ZoneConfig;

const TAB_CREATION_TIMEOUT: Duration = Duration::from_secs(3);

/// A unique identifier for a [`Zone`] within a [`GosubEngine`](crate::GosubEngine).
///
/// Internally, a `ZoneId` wraps a [`Uuid`] to guarantee global uniqueness for
/// each zone created in the engine.
///
/// **Note:** The use of [`Uuid`] is an implementation detail and may change in
/// the future without notice. Always treat `ZoneId` as an opaque handle rather
/// than relying on its internal representation.
///
/// # Purpose
///
/// A `ZoneId` allows the engine and user code to unambiguously reference and
/// operate on a specific [`Zone`], even if multiple zones are created, closed,
/// or restored across sessions.
///
/// # Examples
///
/// Creating a new `ZoneId` manually:
/// ```
/// use gosub_engine::zone::ZoneId;
///
/// let id = ZoneId::new();
/// println!("New zone ID: {:?}", id);
///
/// let fixed_id = ZoneId::from("123e4567-e89b-12d3-a456-426614174000");
/// println!("Fixed zone ID: {}", fixed_id);
/// ```
///
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ZoneId(Uuid);

impl ZoneId {
    /// Creates a new `ZoneId` with a random UUID.
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl From<Uuid> for ZoneId {
    fn from(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

impl From<&str> for ZoneId {
    fn from(s: &str) -> Self {
        Self(Uuid::parse_str(s).unwrap_or_else(|_| Uuid::new_v4()))
    }
}

impl Display for ZoneId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Internal record for a tab within a zone
#[derive(Debug)]
#[allow(unused)]
struct TabRecord {
    /// Tab ID
    id: TabId,
    /// Tab title
    title: String,
    /// Command channel to the tab task
    cmd_tx: mpsc::Sender<TabCommand>,
    /// Join handle
    join: Option<JoinHandle<()>>,
}

impl TabRecord {
    // Returns true when the tab worker has finished
    #[allow(unused)]
    #[inline]
    fn is_finished(&self) -> bool {
        self.join.as_ref().map(|j| j.is_finished()).unwrap_or(true)
    }
}


/// Services provided to tabs within a zone
#[derive(Clone, Debug)]
pub struct ZoneServices {
    // pub zone_id: ZoneId,
    pub storage: Arc<StorageService>,
    pub cookie_store: Option<CookieStoreHandle>,
    pub cookie_jar: Option<CookieJarHandle>,
    pub partition_policy: PartitionPolicy,
    // pub runtime: Handle,
    // pub backend: Arc<dyn RenderBackend + Send + Sync>,
}

// pub struct ZoneState {
//     /// Configuration for the zone (like max tabs allowed)
//     config: ZoneConfig,
//     /// Title of the zone (ie: Home, Work)
//     pub title: String,
//     /// Icon of the zone (could be a base64 encoded image)
//     pub icon: Vec<u8>,
//     /// Description of the zone
//     pub description: String,
//     /// Tab color (RGBA)
//     pub color: [u8; 4],
// }


// pub struct SharedZoneState {
//     pub shared_flags: SharedFlags,
// }

// Things that the tab shares with the zone (or anyone else)
pub struct SharedTabState {
    cmd_tx: mpsc::Sender<TabCommand>,
}

// Things that the Zone shares with the Tab
pub struct ZoneSharedTabState {
    /// Zone services (storage, cookies, etc)
    services: ZoneServices,
    /// Subscription for session storage changes
    storage_rx: Subscription,
    /// Flags controlling which data is shared with other zones.
    pub shared_flags: SharedFlags,
    /// Event channel to send events back to the UI
    event_tx: broadcast::Sender<EngineEvent>,
}

// Things that the zone shares with the engine (or anyone else)
pub struct ZoneSharedEngineState {
    metrics: bool,
}

/// This is the zone structure, which contains tabs and shared services. It is only known to the engine
/// and can be controlled by the user via the engine API.
pub struct Zone {
    pub shared_engine: SharedEngineState,
    pub zone_shared_engine: Arc<ZoneSharedEngineState>,
    pub shared_tabs: Arc<ZoneSharedTabState>,
    /// ID of the zone
    pub id: ZoneId,
    // List of tabs
    tabs: HashMap<TabId, Arc<SharedTabState>>,

    /// Configuration for the zone (like max tabs allowed)
    config: ZoneConfig,
    /// Title of the zone (ie: Home, Work)
    pub title: String,
    /// Icon of the zone (could be a base64 encoded image)
    pub icon: Vec<u8>,
    /// Description of the zone
    pub description: String,
    /// Tab color (RGBA)
    pub color: [u8; 4],
}

impl Debug for Zone {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Zone")
            .field("id", &self.id)
            .field("title", &self.title)
            .field("description", &self.description)
            .field("color", &self.color)
            .field("config", &self.config)
            .field("shared_flags", &self.shared_tabs.shared_flags)
            .finish()
    }
}

#[allow(unused)]
#[derive(Clone, Copy, Debug, Default)]
pub struct SharedFlags {
    /// Other zones are allowed to read this autocomplete elements
    pub share_autocomplete: bool,
    /// Other zones are allowed to read bookmarks
    pub share_bookmarks: bool,
    /// Other zones are allowed to read password entries
    pub share_passwords: bool,
    /// Other zones are allowed to read cookies
    pub share_cookiejar: bool,
}

impl Zone {
    /// Creates a new zone with a specific zone ID
    pub fn new_with_id(
        // Unique ID for the zone
        zone_id: ZoneId,
        // Configuration for the zone
        config: ZoneConfig,
        // Services to provide to tabs within this zone
        services: ZoneServices,
        // Event channel to send events back to the UI
        event_tx: broadcast::Sender<EngineEvent>,
        shared_engine: SharedEngineState,
    ) -> Self {
        // We generate the color by using the zone id as a seed
        let mut rng = StdRng::seed_from_u64(zone_id.0.as_u64_pair().0);
        let random_color = [
            rng.random::<u8>(),
            rng.random::<u8>(),
            rng.random::<u8>(),
            0xff, // Fully opaque
        ];

        let storage_rx = services.storage.subscribe();

        let zone = Self {
            shared_engine,
            zone_shared_engine: Arc::new(ZoneSharedEngineState {
                metrics: false,
            }),
            shared_tabs: Arc::new(ZoneSharedTabState {
                services,
                storage_rx,
                shared_flags: SharedFlags {
                    share_autocomplete: false,
                    share_bookmarks: false,
                    share_passwords: false,
                    share_cookiejar: false,
                },
                event_tx,
            }),
            id: zone_id,
            tabs: HashMap::new(),
            title: "Untitled Zone".to_string(),
            icon: vec![],
            description: "".to_string(),
            color: random_color,
            config,
        };

        zone.spawn_storage_forward();
        zone
    }

    /// Creates a new zone with a random ID and the provided configuration
    pub fn new(
        config: ZoneConfig,
        services: ZoneServices,
        event_tx: broadcast::Sender<EngineEvent>,
        shared_engine: SharedEngineState,
    ) -> Self {
        Self::new_with_id(ZoneId::new(), config, services, event_tx, shared_engine)
    }

    /// Sets the title of the zone
    pub fn set_title(&mut self, title: impl Into<String>) {
        self.title = title.into();
    }

    /// Sets the icon of the zone
    pub fn set_icon(&mut self, icon: Vec<u8>) {
        self.icon = icon;
    }

    /// Sets the description of the zone
    pub fn set_description(&mut self, description: impl Into<String>) {
        self.description = description.into();
    }

    /// Sets the color of the zone (RGBA)
    pub fn set_color(&mut self, color: [u8; 4]) {
        self.color = color;
    }

    // /// Returns the services available to tabs within this zone
    // pub fn services(&self) -> ZoneServices { self.services.clone() }

    /// This function does the actual creation of the tab
    pub(crate) async fn create_tab(&mut self, tab_services: EffectiveTabServices, initial: TabDefaults) -> Result<TabHandle, EngineError> {
        if self.tabs.len() >= self.config.max_tabs {
            return Err(EngineError::TabLimitExceeded);
        }

        // Channel to send and receive commands to and from the UI
        let (tab_cmd_tx, tab_cmd_rx) = mpsc::channel::<TabCommand>(DEFAULT_CHANNEL_CAPACITY);
        let tab_id = TabId::new();

        let (ack_tx, ack_rx) = oneshot::channel::<anyhow::Result<()>>();

        let tab_args = TabSpawnArgs {
            tab_id,
            cmd_rx: tab_cmd_rx,
            event_tx: self.shared_tabs.event_tx.clone(),
            services: tab_services,
        };

        // We need to create tab here...
        //                 TabRecord {
        //                         id: tab_id,
        //                         title,
        //                         cmd_tx: tab_cmd_tx.clone(),
        //                         join: Some(join),
        //                     },
        // @TODO: fix this later
        // let tab = Tab::new(tab_cmd_tx)
        // tab.shared.tab_rx

        let tab: Tab = todo!();

        let join = spawn_tab_task(tab_args, ack_tx);

        match timeout(TAB_CREATION_TIMEOUT, ack_rx).await {
            Ok(Ok(Ok(()))) => {
                let title = initial.clone().title.unwrap_or_else(|| "New Tab".to_string());
                self.tabs.insert(
                    tab_id,
                    tab.shared_state.clone(),
                );

                self.shared_tabs.event_tx.send(EngineEvent::TabCreated { tab_id, zone_id: self.id }).unwrap();
                Ok(TabHandle::new(tab_id, tab_cmd_tx.clone()))
            }
            Ok(Ok(Err(e))) => {
                join.abort();
                Err(EngineError::TaskInitFailed(e.into()))
            }
            Ok(Err(e)) => {
                join.abort();
                Err(EngineError::TaskInitFailed(e.into()))
            }
            Err(e) => {
                join.abort();
                Err(EngineError::TaskInitFailed(e.into()))
            }
        }
    }

    // /// Get the shared localStorage area for this (zone × partition × origin).
    // #[allow(unused)]
    // pub fn local_area(
    //     &self,
    //     pk: &PartitionKey,
    //     origin: &url::Origin,
    // ) -> anyhow::Result<Arc<dyn StorageArea>> {
    //     self.services.storage.local_for(self.id, pk, origin)
    // }

    // /// Get the per-tab sessionStorage area for (zone × tab × partition × origin).
    // #[allow(unused)]
    // pub fn session_area(
    //     &self,
    //     tab: TabId,
    //     pk: &PartitionKey,
    //     origin: &url::Origin,
    // ) -> anyhow::Result<Arc<dyn StorageArea>> {
    //     self.services.storage.session_for(self.id, tab, pk, origin)
    // }


    /// Forwards storage events from the storage service to the engine event channel.
    fn spawn_storage_forward(&self) {
        let mut rx = self.shared_tabs.storage_rx.resubscribe();
        let tx = self.shared_tabs.event_tx.clone();
        let zone_id = self.id;

        tokio::spawn(async move {
            while let Ok(ev) = rx.recv().await {
                let _ = tx.send(EngineEvent::StorageChanged {
                    tab_id: ev.source_tab,
                    zone: Some(zone_id),
                    key: ev.key.unwrap_or("".into()),
                    value: ev.new_value,
                    scope: ev.scope,
                    origin: ev.origin.clone(),
                });
            }
        });
    }

    /// Closes a tab.
    pub fn close_tab(&mut self, tab_id: TabId) -> bool {
        if let Some(_) = self.tabs.remove(&tab_id) {
            // Drop the command channel to signal the tab to close
            // drop(shared_state.cmd_tx);

            // Also disconnect the session storage for this tab
            self.shared_tabs.services.storage.drop_tab(self.id, tab_id);
            true
        } else {
            false
        }
    }

    pub fn list_tabs(&self) -> Vec<TabId> {
        self.tabs.keys().cloned().collect()
    }

    // pub fn tab_title(&self, tab_id: TabId) -> Option<String> {
    //     self.tabs.read().unwrap().get(&tab_id).map(|rec| rec.title.clone())
    // }
}
