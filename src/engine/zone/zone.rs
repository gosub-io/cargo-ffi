use crate::engine::cookies::CookieJarHandle;
use crate::engine::storage::{
    PartitionKey, StorageArea, StorageService, Subscription,
};
use crate::engine::tab::TabId;
use crate::EngineError;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::{Debug, Display};
use std::sync::{Arc, RwLock};
use std::time::Duration;
use tokio::sync::mpsc::{Sender, channel};
use tokio::sync::oneshot;
use tokio::task::JoinHandle;
use tokio::time::timeout;
use uuid::Uuid;
use crate::cookies::CookieStoreHandle;
use crate::engine::DEFAULT_CHANNEL_CAPACITY;
use crate::engine::events::{EngineCommand, EngineEvent};
use crate::storage::types::PartitionPolicy;
use crate::tab::{spawn_tab_task, OpenTabParams, TabHandle, TabSpawnArgs};
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
struct TabRecord {
    /// Tab ID
    id: TabId,
    /// Tab title
    title: String,
    /// Command channel to the tab task
    cmd_tx: Sender<EngineCommand>,
    /// Join handle
    join: Option<JoinHandle<()>>,
}

impl TabRecord {
    // Returns true when the tab worker has finished
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

/// This is the zone structure, which contains tabs and shared services. It is only known to the engine
/// and can be controlled by the user via the engine API.
pub struct Zone {
    /// ID of the zone
    pub id: ZoneId,
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
    /// Tabs in the zone
    tabs: RwLock<HashMap<TabId, TabRecord>>,
    /// Zone services (storage, cookies, etc)
    services: ZoneServices,
    /// Subscription for session storage changes
    storage_rx: Subscription,
    /// Flags controlling which data is shared with other zones.
    pub shared_flags: SharedFlags,
    /// Event channel to send events back to the UI
    event_tx: Sender<EngineEvent>,
    // Handle to the engine
    // engine_handle: EngineHandle,
}

impl Debug for Zone {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Zone")
            .field("id", &self.id)
            .field("title", &self.title)
            .field("description", &self.description)
            .field("color", &self.color)
            .field("tabs", &self.tabs.read().unwrap().keys().collect::<Vec<_>>())
            .field("config", &self.config)
            .field("shared_flags", &self.shared_flags)
            .finish()
    }
}

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
        event_tx: Sender<EngineEvent>,
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
            id: zone_id,
            title: "Untitled Zone".to_string(),
            icon: vec![],
            description: "".to_string(),
            color: random_color,
            tabs:  RwLock::new(HashMap::new()),
            config,

            services: services.clone(),
            storage_rx,

            shared_flags: SharedFlags {
                share_autocomplete: false,
                share_bookmarks: false,
                share_passwords: false,
                share_cookiejar: false,
            },
            event_tx,
        };

        zone.spawn_storage_forward();
        zone
    }

    /// Creates a new zone with a random ID and the provided configuration
    pub fn new(
        config: ZoneConfig,
        services: ZoneServices,
        event_tx: Sender<EngineEvent>,
    ) -> Self {
        Self::new_with_id(ZoneId::new(), config, services, event_tx)
    }

    /// Sets the title of the zone
    pub fn set_title(&mut self, title: &str) {
        self.title = title.to_string();
    }

    /// Sets the icon of the zone
    pub fn set_icon(&mut self, icon: Vec<u8>) {
        self.icon = icon;
    }

    /// Sets the description of the zone
    pub fn set_description(&mut self, description: &str) {
        self.description = description.to_string();
    }

    /// Sets the color of the zone (RGBA)
    pub fn set_color(&mut self, color: [u8; 4]) {
        self.color = color;
    }

    // /// Sets the cookie jar for the zone
    // pub fn set_cookie_jar(&mut self, cookie_jar: CookieJarHandle) {
    //     self.cookie_jar = cookie_jar;
    // }

    /// Returns the services available to tabs within this zone
    pub fn services(&self) -> ZoneServices { self.services.clone() }

    pub async fn create_tab(&self, params: OpenTabParams) -> Result<TabHandle, EngineError> {
        if self.tabs.read().unwrap().len() >= self.config.max_tabs {
            return Err(EngineError::TabLimitExceeded);
        }

        // Channel to send and receive commands to and from the UI
        let (tab_cmd_tx, tab_cmd_rx) = channel::<EngineCommand>(DEFAULT_CHANNEL_CAPACITY);

        let tab_id = TabId::new();

        let (ack_tx, ack_rx) = oneshot::channel::<anyhow::Result<()>>();

        let join = spawn_tab_task(TabSpawnArgs {
            tab_id,
            cmd_rx: tab_cmd_rx,
            event_tx: self.event_tx.clone(),
            services: self.services(),
            // engine: self.engine_handle.clone(),
            initial: params.clone(),
        }, ack_tx);

        match timeout(TAB_CREATION_TIMEOUT, ack_rx).await {
            Ok(Ok(Ok(()))) => {
                let title = params.title.unwrap_or_else(|| "New Tab".to_string());
                let mut tabs = self.tabs.write().unwrap();
                tabs.insert(
                    tab_id,
                    TabRecord {
                        id: tab_id,
                        title,
                        cmd_tx: tab_cmd_tx.clone(),
                        join: Some(join),
                    },
                );
                Ok(TabHandle::new(tab_id, self.engine_tx.clone()))
            }
            Ok(Ok(Err(e))) => {
                join.abort();
                Err(EngineError::TaskInitFailed(e.to_string()))
            }
            Ok(Err(_cancelled)) => {
                join.abort();
                Err(EngineError::TaskInitFailed("Cancelled".to_string()))
            }
            Err(_elapsed) => {
                join.abort();
                Err(EngineError::TaskInitFailed("timeout".to_string()))
            }
        }
    }

    /// Get the shared localStorage area for this (zone × partition × origin).
    pub fn local_area(
        &self,
        pk: &PartitionKey,
        origin: &url::Origin,
    ) -> anyhow::Result<Arc<dyn StorageArea>> {
        self.services.storage.local_for(self.id, pk, origin)
    }

    /// Get the per-tab sessionStorage area for (zone × tab × partition × origin).
    pub fn session_area(
        &self,
        tab: TabId,
        pk: &PartitionKey,
        origin: &url::Origin,
    ) -> anyhow::Result<Arc<dyn StorageArea>> {
        self.services.storage.session_for(self.id, tab, pk, origin)
    }


    /// Forwards storage events from the storage service to the engine event channel.
    fn spawn_storage_forward(&self) {
        let rx = &self.storage_rx;
        let tx = self.event_tx.clone();

        let zone_id = self.id;

        tokio::spawn(async move {
            while let Ok(ev) = rx.recv().await {
                let _ = tx.send(EngineEvent::StorageChanged {
                    tab: ev.source_tab,
                    zone: Some(zone_id),
                    key: ev.key.unwrap_or("".into()),
                    value: ev.new_value,
                    scope: ev.scope,
                    origin: ev.origin.clone(),
                }).await;
            }
        });
    }

    /// Closes a tab.
    pub fn close_tab(&self, tab_id: TabId) -> bool {
        if let Some(rec) = self.tabs.write().unwrap().remove(&tab_id) {
            // Drop the command channel to signal the tab to close
            drop(rec.cmd_tx);
            // Also disconnect the session storage for this tab
            self.services.storage.drop_tab(self.id, tab_id);
            true
        } else {
            false
        }
    }

    pub fn list_tabs(&self) -> Vec<TabId> {
        self.tabs.read().unwrap().keys().cloned().collect()
    }

    // pub fn tab_title(&self, tab_id: TabId) -> Option<String> {
    //     self.tabs.read().unwrap().get(&tab_id).map(|rec| rec.title.clone())
    // }
}
