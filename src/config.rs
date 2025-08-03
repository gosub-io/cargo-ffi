const DEFAULT_USER_AGENT: &str = "Gosub/1.0 (X11; Linux x86_64) Gecko/20250802 GosubBrowser/1.0";

/// Zone configuration
#[derive(Debug, Clone)]
pub struct ZoneConfig {
    pub max_tabs: usize,            // How many tabs might be opened in the zone
}

/// Main engine configuration. Also contains default configuration for other components like zones.
#[derive(Debug, Clone)]
pub struct EngineConfig {
    pub user_agent: String,                 // User agent string for HTTP requests
    pub max_zones: usize,                   // Maximum number of zones that can be created
    pub default_zone_config: ZoneConfig,    // Default zone config if none is supplied
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