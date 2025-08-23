//! Engine configuration.
//!
//! The [`EngineConfig`] struct defines global configuration for the Gosub
//! engine, including defaults for zones, user agent strings, and global
//! resource limits. It is provided to core components such as the
//! [`ZoneManager`](crate::engine::zone::manager::ZoneManager) to enforce
//! constraints (e.g. maximum zones) and to supply default settings when
//! creating new zones.
//!
//! # Example
//!
//! ```rust
//! use gosub_engine::engine::config::EngineConfig;
//!
//! // Use the default engine configuration
//! let config = EngineConfig::default();
//!
//! assert_eq!(config.max_zones, 10);
//! assert_eq!(config.default_zone_config.max_tabs, 5);
//! ```

use crate::engine::zone::ZoneConfig;

/// Default user agent string used by the engine for HTTP requests.
const DEFAULT_USER_AGENT: &str = "Gosub/1.0 (X11; Linux x86_64) Gecko/20250802 GosubBrowser/1.0";

/// Main engine configuration.
///
/// `EngineConfig` contains global defaults and limits for the entire
/// engine, as well as the default configuration applied to new zones
/// if no specific configuration is provided at creation time.
#[derive(Debug, Clone)]
pub struct EngineConfig {
    /// User agent string used for outgoing HTTP requests.
    pub user_agent: String,

    /// Maximum number of zones that can be created within this engine.
    /// Attempts to create more than this number will result in an error.
    pub max_zones: usize,

    /// Default zone configuration applied when creating a new zone
    /// without an explicit [`ZoneConfig`].
    pub default_zone_config: ZoneConfig,
}

impl Default for EngineConfig {
    /// Provides a sensible default engine configuration:
    ///
    /// - User agent string set to a Gosub identifier.
    /// - `max_zones` = 10
    /// - Each zone defaults to `max_tabs` = 5
    fn default() -> Self {
        Self {
            user_agent: DEFAULT_USER_AGENT.to_string(),
            max_zones: 10,
            default_zone_config: ZoneConfig { max_tabs: 5 },
        }
    }
}
