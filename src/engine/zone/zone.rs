use crate::engine::cookies::CookieJarHandle;
use crate::engine::cookies::DefaultCookieJar;
use crate::engine::storage::{
    PartitionKey, StorageArea, StorageService, Subscription,
};
use crate::engine::tab::TabId;
use crate::engine::zone::password_store::PasswordStore;
use crate::render::Viewport;
use crate::zone::ZoneConfig;
use crate::EngineError;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::Display;
use std::sync::{Arc, RwLock};
use tokio::sync::mpsc::{Sender, channel};
use uuid::Uuid;
use crate::engine::events::{EngineCommand, EngineEvent};
use crate::tab::{spawn_tab_task, TabHandle};



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
/// ```
///
/// Creating a zone with a fixed ID:
/// ```no_run
/// use gosub_engine::GosubEngine;
/// use gosub_engine::zone::ZoneId;
///
/// let backend = gosub_engine::render::backends::null::NullBackend::new().expect("null renderer cannot be created (!?)");
/// let mut engine = GosubEngine::new(None, Box::new(backend));
///
/// let fixed_id = ZoneId::from("123e4567-e89b-12d3-a456-426614174000");
/// let zone_id = engine.zone_builder()
///     .id(fixed_id)
///     .create()
///     .unwrap();
/// assert_eq!(zone_id, fixed_id);
/// ```
///
/// Using `ZoneId` as a map key:
/// ```
/// use gosub_engine::zone::ZoneId;
/// use std::collections::HashMap;
///
/// let mut zones: HashMap<ZoneId, String> = HashMap::new();
/// let z1 = ZoneId::new();
/// zones.insert(z1, "Profile 1".to_string());
/// ```
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
#[derive(Clone, Debug)]
struct TabRecord {
    /// Tab ID
    id: TabId,
    /// Tab title
    title: String,
    /// Command channel to the tab task
    cmd_tx: Sender<EngineCommand>,
}

/// Services provided to tabs within a zone
pub struct ZoneServices {
    pub zone_id: ZoneId,
    pub storage: Arc<StorageService>,
    pub cookie_jar: CookieJarHandle,
}

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

    /// Session storage for the zone (shared between all tabs in the zone)
    pub storage: Arc<StorageService>,
    /// Subscription for session storage changes
    storage_rx: Subscription,

    /// Where to load/store cookies within this zone
    pub cookie_jar: CookieJarHandle,
    /// Per-zone password storage
    pub password_store: PasswordStore,
    /// Flags controlling which data is shared with other zones.
    pub shared_flags: SharedFlags,

    event_tx: Sender<EngineEvent>,
}

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
        zone_id: ZoneId,
        config: ZoneConfig,
        storage: Arc<StorageService>,
        cookie_jar: Option<CookieJarHandle>,
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

        let storage_rx = storage.subscribe();

        let cookie_jar =
            cookie_jar.unwrap_or_else(|| Arc::new(RwLock::new(DefaultCookieJar::new())));

        let zone = Self {
            id: zone_id,
            title: "Untitled Zone".to_string(),
            icon: vec![],
            description: "".to_string(),
            color: random_color,
            tabs:  RwLock::new(HashMap::new()),
            config,

            storage,
            storage_rx,

            cookie_jar,
            password_store: PasswordStore::new(),
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
        storage: Arc<StorageService>,
        cookie_jar: Option<CookieJarHandle>,
        event_tx: Sender<EngineEvent>,
    ) -> Self {
        let zone_id = ZoneId::new();
        Self::new_with_id(zone_id, config, storage, cookie_jar, event_tx)
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

    /// Sets the cookie jar for the zone
    pub fn set_cookie_jar(&mut self, cookie_jar: CookieJarHandle) {
        self.cookie_jar = cookie_jar;
    }

    /// Returns the services available to tabs within this zone
    pub fn services(&self) -> ZoneServices {
        ZoneServices {
            zone_id: self.id,
            storage: self.storage.clone(),
            cookie_jar: self.cookie_jar.clone(),
        }
    }

    pub fn create_tab(
        &self,
        title: impl Into<String>,
        viewport: Viewport,
    ) -> Result<TabHandle, EngineError> {
        if self.tabs.read().unwrap().len() >= self.config.max_tabs {
            return Err(EngineError::TabLimitExceeded);
        }

        let (cmd_tx, cmd_rx) = channel::<EngineCommand>(256);
        let tab_id = TabId::new();

        spawn_tab_task(
            tab_id,
            cmd_rx,
            self.event_tx.clone(),
            self.services(),
            viewport,
        );

        {
            let mut tabs = self.tabs.write().unwrap();
            tabs.insert(
                tab_id,
                TabRecord { id: tab_id, title: title.into(), cmd_tx: cmd_tx.clone() }
            );
        }

        Ok(TabHandle { id: tab_id, cmd_tx })
    }

    /// Get the shared localStorage area for this (zone × partition × origin).
    pub fn local_area(
        &self,
        pk: &PartitionKey,
        origin: &url::Origin,
    ) -> anyhow::Result<Arc<dyn StorageArea>> {
        self.storage.local_for(self.id, pk, origin)
    }

    /// Get the per-tab sessionStorage area for (zone × tab × partition × origin).
    pub fn session_area(
        &self,
        tab: TabId,
        pk: &PartitionKey,
        origin: &url::Origin,
    ) -> Arc<dyn StorageArea> {
        self.storage.session_for(self.id, tab, pk, origin)
    }


    /// Forwards storage events from the storage service to the engine event channel.
    fn spawn_storage_forward(&self) {
        let mut rx = self.storage_rx.clone();
        let tx = self.event_tx.clone();

        let zone_id = self.id;

        tokio::spawn(async move {
            while let Some(ev) = rx.recv().await {
                let _ = tx.send(EngineEvent::StorageChanged {
                    tab: ev.source_tab,
                    zone: Some(zone_id.clone()),
                    key: ev.key,
                    value: ev.value,
                    scope: ev.scope,
                    origin: ev.origin.clone(),
                }).await;
            }
        });
    }

    pub fn close_tab(&self, tab_id: TabId) -> bool {
        if let Some(rec) = self.tabs.write().unwrap().remove(&tab_id) {
            drop(rec.cmd_tx);
            self.storage.drop_tab(self.id, tab_id);
            true
        } else {
            false
        }
    }

    pub fn list_tabs(&self) -> Vec<TabId> {
        self.tabs.read().unwrap().keys().cloned().collect()
    }

    pub fn tab_title(&self, tab_id: TabId) -> Option<String> {
        self.tabs.read().unwrap().get(&tab_id).map(|rec| rec.title.clone())
    }
}
