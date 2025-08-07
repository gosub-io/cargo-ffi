use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use crate::{EngineConfig, EngineError, ZoneConfig};
use crate::zone::zone::{Zone, ZoneId};

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
    pub fn create_zone(&self, config: Option<ZoneConfig>) -> Result<ZoneId, EngineError> {
        let mut zones = self.zones.lock().unwrap();

        if zones.len() >= self.config.max_zones {
            return Err(EngineError::ZoneLimitExceeded)
        }

        let resolved_config = config.unwrap_or_else(|| self.config.default_zone_config.clone());
        let zone = Zone::new(resolved_config);
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