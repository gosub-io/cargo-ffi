// src/engine/zone.rs
//! Zone system: [`ZoneManager`], [`Zone`], and [`ZoneId`].
//!
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use crate::{EngineConfig, EngineError, ZoneConfig};
use crate::engine::storage::local::in_memory::InMemoryLocalStore;
use crate::engine::storage::StorageService;
use crate::engine::zone::{Zone, ZoneId};
use crate::storage::InMemorySessionStore;

pub struct ZoneManager {
    config: EngineConfig,
    zones: Arc<Mutex<HashMap<ZoneId, Arc<Mutex<Zone>>>>>,
}

impl ZoneManager {
    pub fn new(config: EngineConfig) -> Self {
        Self {
            config,
            zones: Arc::new(Mutex::new(HashMap::new())),
        }
    }


    /// Create a new zone with the given config
    pub fn create_zone(&self, zone_id: Option<ZoneId>, config: Option<ZoneConfig>, storage_service: Option<Arc<StorageService>>) -> Result<ZoneId, EngineError> {
        let mut zones = self.zones.lock().unwrap();

        if zones.len() >= self.config.max_zones {
            return Err(EngineError::ZoneLimitExceeded)
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
                Zone::new_with_id(id, resolved_config, storage)
            },
            None => {
                Zone::new(resolved_config, storage)
            }
        };
        let zone_id = zone.id;

        zones.insert(zone_id, Arc::new(Mutex::new(zone)));
        Ok(zone_id)
    }

    pub fn get_zone(&self, id: ZoneId) -> Option<Arc<Mutex<Zone>>> {
        let zones = self.zones.lock().ok()?;
        zones.get(&id).cloned()
    }


    /// Get a mutable reference to a zone
    pub fn get_zone_mut(&self, id: &ZoneId) -> Option<Arc<Mutex<Zone>>> {
        let zones = self.zones.lock().ok()?;
        zones.get(id).cloned()
    }

    /// Remove a zone
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

    pub fn iter(&self) -> Vec<ZoneId> {
        self.zones
            .lock()
            .map(|z| z.keys().copied().collect())
            .unwrap_or_default()
    }
}