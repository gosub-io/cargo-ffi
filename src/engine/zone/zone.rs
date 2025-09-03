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
use std::sync::atomic::AtomicUsize;
use tokio::sync::broadcast;
use uuid::Uuid;
use crate::cookies::CookieStoreHandle;
use crate::engine::engine::EngineContext;
use crate::engine::events::EngineEvent;
use crate::storage::types::PartitionPolicy;
use crate::tab::{TabDefaults, TabOverrides, TabSink, TabWorker, TabHandle};
use crate::tab::services::resolve_tab_services;
use crate::zone::ZoneConfig;

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

impl Display for ZoneId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
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
}

/// Zone context we can share downwards to tabs
pub struct ZoneContext {
    /// Zone services (storage, cookies, etc)
    pub(crate) services: ZoneServices,
    /// Subscription for session storage changes
    pub(crate) storage_rx: Subscription,
    /// Flags controlling which data is shared with other zones.
    pub(crate) shared_flags: SharedFlags,
    /// Event channel to send events back to the UI
    pub(crate) event_tx: broadcast::Sender<EngineEvent>,
}

// Things that are shared upwards to the engine
pub struct ZoneSink {
    /// How many tabs has this zone created over its lifetime
    tabs_created: AtomicUsize,
}

/// This is the zone structure, which contains tabs and shared services. It is only known to the engine
/// and can be controlled by the user via the engine API.
pub struct Zone {
    // Shared context from the engine
    pub engine_context: Arc<EngineContext>,
    // Shared context that is passed down to tabs
    pub context: Arc<ZoneContext>,
    // Shared state that can be read by anyone with a ZoneSink
    pub sink: Arc<ZoneSink>,
    // List of tabs
    tabs: HashMap<TabId, Arc<TabSink>>,

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
}

impl Debug for Zone {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Zone")
            .field("id", &self.id)
            .field("title", &self.title)
            .field("description", &self.description)
            .field("color", &self.color)
            .field("config", &self.config)
            .field("shared_flags", &self.context.shared_flags)
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
        engine_context: Arc<EngineContext>,
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

        let event_tx = engine_context.event_tx.clone();

        let zone = Self {
            engine_context,
            sink: Arc::new(ZoneSink {
                tabs_created: AtomicUsize::new(0),
            }),
            context: Arc::new(ZoneContext {
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

        zone.spawn_storage_events_to_engine();
        zone
    }

    /// Creates a new zone with a random ID and the provided configuration
    pub fn new(
        config: ZoneConfig,
        services: ZoneServices,
        engine_context: Arc<EngineContext>,
    ) -> Self {
        Self::new_with_id(ZoneId::new(), config, services, engine_context)
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
    pub async fn create_tab(&mut self, initial: TabDefaults, overrides: Option<TabOverrides>) -> Result<TabHandle, EngineError> {
        if self.tabs.len() >= self.config.max_tabs {
            return Err(EngineError::TabLimitExceeded);
        }

        let tab_services = resolve_tab_services(self.id, &self.context.services, &overrides.unwrap_or_default());

        let handle = TabWorker::new_on_thread(tab_services, self.context.clone())
            .map_err(|e| EngineError::CreateTab(e.into()))?;
        self.tabs.insert(handle.tab_id, handle.sink.clone());

        // Increase metrics
        self.sink.tabs_created.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        // Set tab defaults
        handle.set_title(initial.title.as_deref().unwrap_or("New Tab")).await?;
        handle.set_viewport(initial.viewport.unwrap_or_default()).await?;

        // Load URL in tab if provided
        if let Some(url) = initial.url.as_ref() {
            handle.navigate(url).await?;
        }

        Ok(handle)


        // let join = spawn_tab_task(tab_args, ack_tx);
        //
        // match timeout(TAB_CREATION_TIMEOUT, ack_rx).await {
        //     Ok(Ok(Ok(()))) => {
        //         let title = initial.clone().title.unwrap_or_else(|| "New Tab".to_string());
        //         self.tabs.insert(
        //             tab_id,
        //             tab.shared_state.clone(),
        //         );
        //
        //         self.shared_tabs.event_tx.send(EngineEvent::TabCreated { tab_id, zone_id: self.id }).unwrap();
        //         Ok(TabHandle::new(tab_id, tab_cmd_tx.clone()))
        //     }
        //     Ok(Ok(Err(e))) => {
        //         join.abort();
        //         Err(EngineError::TaskInitFailed(e.into()))
        //     }
        //     Ok(Err(e)) => {
        //         join.abort();
        //         Err(EngineError::TaskInitFailed(e.into()))
        //     }
        //     Err(e) => {
        //         join.abort();
        //         Err(EngineError::TaskInitFailed(e.into()))
        //     }
        // }
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
    fn spawn_storage_events_to_engine(&self) {
        let mut rx = self.context.storage_rx.resubscribe();
        let tx = self.context.event_tx.clone();
        let zone_id = self.id;

        tokio::spawn(async move {
            while let Ok(ev) = rx.recv().await {
                let _ = tx.send(EngineEvent::StorageChanged {
                    tab_id: ev.source_tab,
                    zone: Some(zone_id),
                    key: ev.key.unwrap_or_default(),
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

            // Disconnect the session storage for this tab
            self.context.services.storage.drop_tab(self.id, tab_id);
            return true;
        }

        false
    }

    /// Lists all tab IDs in this zone.
    pub fn list_tabs(&self) -> Vec<TabId> {
        self.tabs.keys().cloned().collect()
    }
}
