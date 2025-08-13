const DEFAULT_USER_AGENT: &str = "Gosub/1.0 (X11; Linux x86_64) Gecko/20250802 GosubBrowser/1.0";

/// Zone configuration that defines the properties of a zone
#[derive(Debug, Clone)]
pub struct ZoneConfig {
    /// How many tabs might be opened in the zone
    pub max_tabs: usize,
}

/// Main engine configuration. Also contains default configuration for other components like zones.
#[derive(Debug, Clone)]
pub struct EngineConfig {
    /// User agent string for HTTP requests
    pub user_agent: String,
    /// Maximum number of zones that can be created
    pub max_zones: usize,
    /// Default zone config if none is supplied
    pub default_zone_config: ZoneConfig,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            user_agent: DEFAULT_USER_AGENT.to_string(),
            max_zones: 10,
            default_zone_config: ZoneConfig {
                max_tabs: 5,        // Default max tabs per group
            },
        }
    }
}