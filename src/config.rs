use crate::viewport::Viewport;

#[derive(Debug, Clone)]
pub struct ZoneConfig {
    pub max_tabs: usize,
}

#[derive(Debug, Clone)]
pub struct EngineConfig {
    pub viewport: Viewport,
    pub user_agent: String,
    pub max_zones: usize,
    pub zone_config: ZoneConfig,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            viewport: Viewport::new(800, 600),
            user_agent: "GosubEngine/1.0".to_string(),
            max_zones: 10,
            zone_config: ZoneConfig {
                max_tabs: 5,        // Default max tabs per group
            },
        }
    }
}