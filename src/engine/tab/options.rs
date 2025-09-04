use crate::cookies::CookieJarHandle;
use crate::render::Viewport;
use crate::storage::{PartitionKey, StorageService};
use std::sync::Arc;

#[derive(Clone, Debug, Default)]
pub struct TabDefaults {
    pub url: Option<String>,
    pub title: Option<String>,
    pub viewport: Option<Viewport>,
}

#[derive(Clone, Debug, Default)]
pub struct TabOverrides {
    // Services & partitioning
    pub partition_key: Option<PartitionKey>, // None => inherit zone policy
    pub cookie_jar: TabCookieJar,            // Default::Inherit
    pub storage_scope: TabStorageScope,      // Default::Inherit
    pub cache_mode: TabCacheMode,            // Default::Inherit

    // Identity
    pub user_agent: Option<String>,
    pub accept_language: Option<Vec<String>>,

    // Content
    pub js_enabled: Option<bool>,
    pub images_enabled: Option<bool>,

    // UI/Render
    pub zoom: Option<f32>,

    // Persistence
    pub persist_history: Option<bool>,
    pub persist_downloads: Option<bool>,
}

#[derive(Clone, Debug)]
pub enum TabCookieJar {
    Inherit,   // use zone jar
    Ephemeral, // fresh jar, dropped on close
    Custom(CookieJarHandle),
}
impl Default for TabCookieJar {
    fn default() -> Self {
        Self::Inherit
    }
}

#[derive(Clone, Debug)]
pub enum TabStorageScope {
    Inherit,   // zone StorageService
    Ephemeral, // in-memory Local/Session for this tab only
    Custom(Arc<StorageService>),
}
impl Default for TabStorageScope {
    fn default() -> Self {
        Self::Inherit
    }
}

#[derive(Clone, Debug)]
pub enum TabCacheMode {
    Inherit,
    Default,
    Bypass,
    Ephemeral,
}
impl Default for TabCacheMode {
    fn default() -> Self {
        Self::Inherit
    }
}
