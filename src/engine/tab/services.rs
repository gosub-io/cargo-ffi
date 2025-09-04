use std::sync::Arc;
use crate::zone::{ZoneId, ZoneServices};
use crate::cookies::{CookieJarHandle, DefaultCookieJar};
use crate::storage::{StorageService, InMemoryLocalStore, InMemorySessionStore, PartitionKey, PartitionPolicy};
use crate::tab::options::{TabCookieJar, TabOverrides, TabStorageScope};
use crate::tab::structs::EffectiveTabServices;

/// Resolve the effective services for a tab based on the zone services and tab overrides.
pub fn resolve_tab_services(zone_id: ZoneId, services: &ZoneServices, ov: &TabOverrides) -> EffectiveTabServices {
    let partition_key = ov.partition_key.clone().unwrap_or_else(|| {
        PartitionKey::from_zone(zone_id)
    });

    let partition_policy = if ov.partition_key.is_some() {
        // Custom partition key means isolated storage/cookies
        PartitionPolicy::None
    } else {
        // Inherit zone policy
        services.partition_policy.clone()
    };

    let storage = match &ov.storage_scope {
        TabStorageScope::Inherit => services.storage.clone(),
        TabStorageScope::Custom(s) => s.clone(),
        TabStorageScope::Ephemeral => Arc::new(StorageService::new(
            Arc::new(InMemoryLocalStore::new()),
            Arc::new(InMemorySessionStore::new()),
        )),
    };

    let cookie_jar = match &ov.cookie_jar {
        TabCookieJar::Inherit => services.cookie_jar.clone().unwrap_or_else(|| DefaultCookieJar::new().into()),
        TabCookieJar::Custom(handle) => handle.clone(),
        TabCookieJar::Ephemeral => {
            let jar: CookieJarHandle = DefaultCookieJar::new().into();
            jar
        }
    };

    EffectiveTabServices {
        partition_key,
        partition_policy,
        storage,
        cookie_jar,
    }
}