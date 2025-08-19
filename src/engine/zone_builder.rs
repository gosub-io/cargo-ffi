use crate::cookies::{CookieJarHandle, CookieStoreHandle};
use crate::storage::StorageService;
use crate::zone::{ZoneConfig, ZoneId};
use crate::{EngineError, GosubEngine};
use std::sync::Arc;

/// Builder for creating a new [`Zone`](crate::engine::zone::Zone) in the [`GosubEngine`].
///
/// A `Zone` in Gosub represents an isolated browsing context.
/// Each zone can have its own storage (cookies, local/session storage, etc.),
/// configuration, and set of tabs. Zones are useful for separating
/// different browsing profiles, private/incognito sessions, or even
/// multi-tenant browsing in the same browsing context.
///
/// This builder allows you to configure optional parameters before creating
/// the zone. If you omit an option, the engine will use sensible defaults:
///
/// * [`ZoneId`] — If not provided, a new UUID will be generated.
/// * [`ZoneConfig`] — If not provided, the engine will use its default config.
/// * [`StorageService`] — If not provided, the zone will not have persistence.
///
/// # Fields
///
/// - `zone_id`: Assign a fixed ID to the zone (useful for restoring state).
/// - `config`: Per-zone configuration (e.g., user agent string, privacy settings).
/// - `storage`: Controls persistence of local/session storage for this zone.
///
/// # Examples
///
/// Creating a simple zone with default settings:
/// ```
/// use std::sync::Arc;
/// use gosub_engine::GosubEngine;
/// use gosub_engine::storage::{InMemorySessionStore, SqliteLocalStore, StorageService};
///
/// // Create the engine
/// let mut engine = GosubEngine::new(None);
///
/// // Create an in-memory storage service
/// let storage = Arc::new(StorageService::new(
///     Arc::new(SqliteLocalStore::new("local.db").unwrap()),
///     Arc::new(InMemorySessionStore::new()),
/// ));
///
/// // Build the zone
/// let zone_id = engine.zone_builder()
///     .storage(storage.clone())
///     .create()
///     .expect("zone creation failed");
///
/// println!("Zone created: {:?}", zone_id);
/// ```
///
/// Creating a zone without persistent storage:
/// ```
/// use gosub_engine::GosubEngine;
/// let mut engine = GosubEngine::new(None);
/// let zone_id = engine.zone_builder()
///     .create()
///     .unwrap();
/// ```
pub struct ZoneBuilder<'e> {
    /// Engine context to build the Zone.
    engine: &'e mut GosubEngine,
    /// Optional ID for the Zone being built.
    zone_id: Option<ZoneId>,
    /// Optional configuration for the Zone.
    config: Option<ZoneConfig>,
    /// Optional storage service for the Zone.
    storage: Option<Arc<StorageService>>,
    // partition_policy: Option<PartitionPolicy>,
    // quota_bytes: Option<u64>,
    /// Optional cookie store handle for the Zone.
    cookie_store: Option<CookieStoreHandle>,
    /// Optional cookie jar handle for the Zone. Will override the store
    cookie_jar: Option<CookieJarHandle>,
}

impl GosubEngine {
    /// Entry point to start building a Zone.
    pub fn zone_builder(&mut self) -> ZoneBuilder<'_> {
        ZoneBuilder {
            engine: self,
            zone_id: None,
            config: None,
            storage: None,
            cookie_store: None,
            cookie_jar: None,
            // partition_policy: None,
            // quota_bytes: None,
        }
    }
}

impl<'e> ZoneBuilder<'e> {
    pub fn id(mut self, id: ZoneId) -> Self {
        self.zone_id = Some(id);
        self
    }

    pub fn config(mut self, cfg: ZoneConfig) -> Self {
        self.config = Some(cfg);
        self
    }

    pub fn storage(mut self, svc: Arc<StorageService>) -> Self {
        self.storage = Some(svc);
        self
    }

    pub fn cookie_store(mut self, store: CookieStoreHandle) -> Self {
        self.cookie_store = Some(store);
        self
    }

    pub fn cookie_jar(mut self, jar: CookieJarHandle) -> Self {
        self.cookie_jar = Some(jar);
        self
    }

    pub fn create(&mut self) -> Result<ZoneId, EngineError> {
        // Either we have a cookie store from which we can take a jar, or we have provided a cookie jar, but not both.
        if self.cookie_store.is_some() && self.cookie_jar.is_some() {
            return Err(EngineError::InvalidConfiguration(
                "Cannot set both cookie store and cookie jar".to_string(),
            ));
        }

        // Generate a new ZoneId if not provided
        if self.zone_id.is_none() {
            self.zone_id = Some(ZoneId::new());
        }

        // If we have a cookie store but not a cookie jar, we let the store create the jar for the zone_id
        if self.cookie_jar.is_none() && self.cookie_jar.is_some() {
            let jar = self
                .cookie_store
                .clone()
                .unwrap()
                .jar_for(self.zone_id.unwrap());
            self.cookie_jar = jar
        }

        self.engine.create_zone(
            self.zone_id,
            self.config.take(),
            self.storage.take(),
            self.cookie_jar.take(),
        )
    }
}
