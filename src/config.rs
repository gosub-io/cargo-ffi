use crate::viewport::Viewport;

#[derive(Debug, Clone)]
pub struct TabGroupConfig {
    pub max_tabs: usize,
}

#[derive(Debug, Clone)]
pub struct GosubEngineConfig {
    pub viewport: Viewport,
    pub user_agent: String,
    pub max_groups: usize,
    pub tab_group_config: TabGroupConfig,
}

impl Default for GosubEngineConfig {
    fn default() -> Self {
        Self {
            viewport: Viewport::new(800, 600),
            user_agent: "GosubEngine/1.0".to_string(),
            max_groups: 10,
            tab_group_config: TabGroupConfig {
                max_tabs: 5,        // Default max tabs per group
            },
        }
    }
}