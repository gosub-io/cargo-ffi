// src/engine/zone.rs
//! Zone system: [`Zone`], and [`ZoneId`].
//!
//! Zone manager.
//!
//! The [`ZoneManager`] is responsible for creating, tracking, and
//! managing all [`Zone`] instances in the
//! engine. It enforces global limits defined by the [`EngineConfig`]
//! (such as maximum number of zones) and provides accessors to retrieve,
//! iterate, and remove zones.
//!
//! Zones are stored in a thread-safe [`Arc<Mutex<_>>`] container and
//! can be accessed concurrently from multiple parts of the engine.
//!
//! # Responsibilities
//!
//! - Enforce engine-wide constraints (e.g., `max_zones`).
//! - Create zones with either caller-supplied or default configuration.
//! - Provide default in-memory storage if no storage service is supplied.
//! - Manage the lifecycle of zones (insert, get, remove, iterate).
//!
//! # Example
//!
//! ```rust
//! use gosub_engine::zone::{ZoneManager, ZoneConfig};
//! use gosub_engine::{EngineConfig, EngineError};
//!
//! let engine_config = EngineConfig::default();
//! let manager = ZoneManager::new(engine_config);
//!
//! // Create a new zone with defaults
//! let zone_id = manager.create_zone(None, None, None, None).unwrap();
//!
//! // Access the zone later
//! let zone = manager.get_zone(zone_id).unwrap();
//! ```

use crate::cookies::CookieJarHandle;
use crate::engine::storage::local::in_memory::InMemoryLocalStore;
use crate::engine::storage::StorageService;
use crate::engine::zone::{Zone, ZoneConfig, ZoneId};
use crate::storage::InMemorySessionStore;
use crate::{EngineConfig, EngineError};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Manages all zones within the engine.
///
/// The `ZoneManager` enforces global engine limits (such as maximum
/// number of zones) and owns a thread-safe registry of all active
/// zones. Each zone is identified by a unique [`ZoneId`].
pub struct ZoneManager {
    /// Global engine configuration, including zone limits and defaults.
    config: EngineConfig,
    /// Thread-safe map of all active zones, keyed by their IDs.
    zones: Arc<Mutex<HashMap<ZoneId, Arc<Mutex<Zone>>>>>,
}

impl ZoneManager {
    /// Creates a new [`ZoneManager`] with the given engine configuration.
    pub fn new(config: EngineConfig) -> Self {
        Self {
            config,
            zones: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Creates a new zone with the given configuration and optional services.
    ///
    /// # Arguments
    /// - `zone_id`: Optional ID. If not provided, a new one is generated.
    /// - `config`: Optional [`ZoneConfig`]. Falls back to engine default if not supplied.
    /// - `storage_service`: Optional custom storage service. Defaults to in-memory storage.
    /// - `cookie_jar`: Optional cookie jar handle for the zone.
    ///
    /// # Errors
    /// - Returns [`EngineError::ZoneLimitExceeded`] if the maximum number of zones is reached.
    /// - Returns [`EngineError::ZoneAlreadyExists`] if a zone with the given ID already exists.
    pub fn create_zone(
        &self,
        zone_id: Option<ZoneId>,
        config: Option<ZoneConfig>,
        storage_service: Option<Arc<StorageService>>,
        cookie_jar: Option<CookieJarHandle>,
    ) -> Result<ZoneId, EngineError> {
        let mut zones = self.zones.lock().unwrap();

        if zones.len() >= self.config.max_zones {
            return Err(EngineError::ZoneLimitExceeded);
        }

        // Check if we defined storage service, if not we use the default one (in-memory)
        let storage = storage_service.unwrap_or_else(|| {
            // If no storage service is provided, we use the default in-memory storage
            Arc::new(StorageService::new(
                Arc::new(InMemoryLocalStore::new()),
                Arc::new(InMemorySessionStore::new()),
            ))
        });

        let resolved_config = config.unwrap_or_else(|| self.config.default_zone_config.clone());
        let zone = match zone_id {
            Some(id) => {
                if zones.contains_key(&id) {
                    return Err(EngineError::ZoneAlreadyExists);
                }
                Zone::new_with_id(id, resolved_config, storage, cookie_jar)
            }
            None => Zone::new(resolved_config, storage, cookie_jar),
        };
        let zone_id = zone.id;

        zones.insert(zone_id, Arc::new(Mutex::new(zone)));
        Ok(zone_id)
    }

    /// Retrieves a zone by its [`ZoneId`], if it exists.
    pub fn get_zone(&self, id: ZoneId) -> Option<Arc<Mutex<Zone>>> {
        let zones = self.zones.lock().ok()?;
        zones.get(&id).cloned()
    }

    /// Retrieves a zone by its [`ZoneId`] for mutation, if it exists.
    pub fn get_zone_mut(&self, id: &ZoneId) -> Option<Arc<Mutex<Zone>>> {
        let zones = self.zones.lock().ok()?;
        zones.get(id).cloned()
    }

    /// Removes a zone by its [`ZoneId`].
    ///
    /// # Errors
    /// - Returns [`EngineError::ZoneNotFound`] if the zone does not exist
    ///   or the lock could not be acquired.
    #[allow(unused)]
    pub fn remove_zone(&self, zone_id: ZoneId) -> Result<(), EngineError> {
        if !self.zones.lock().is_ok() {
            return Err(EngineError::ZoneNotFound);
        }

        let mut zones = self.zones.lock().unwrap();
        if zones.remove(&zone_id).is_none() {
            return Err(EngineError::ZoneNotFound);
        }

        Ok(())
    }

    /// Returns a list of all active [`ZoneId`]s.
    pub fn iter(&self) -> Vec<ZoneId> {
        self.zones
            .lock()
            .map(|z| z.keys().copied().collect())
            .unwrap_or_default()
    }
}
