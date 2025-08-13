use std::sync::Arc;
use crate::{EngineError, GosubEngine, ZoneConfig, ZoneId};
use crate::storage::StorageService;

pub struct ZoneBuilder<'e> {
    engine: &'e mut GosubEngine,
    zone_id: Option<ZoneId>,
    config: Option<ZoneConfig>,
    storage: Option<Arc<StorageService>>,
    // partition_policy: Option<PartitionPolicy>,
    // quota_bytes: Option<u64>,
}

impl GosubEngine {
    /// Entry point to start building a Zone.
    pub fn zone(&mut self) -> ZoneBuilder<'_> {
        ZoneBuilder {
            engine: self,
            zone_id: None,
            config: None,
            storage: None,
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

    // Example of ergonomic Into<Arc<...>> if you want:
    // pub fn storage<S: Into<Arc<StorageService>>>(mut self, svc: S) -> Self {
    //     self.storage = Some(svc.into());
    //     self
    // }

    pub fn create(&mut self) -> Result<ZoneId, EngineError> {
        self.engine.create_zone(
            self.zone_id,
            self.config.take(),
            self.storage.take(),
        )
    }
}