use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use gtk4::cairo;
use tokio::runtime::Runtime;
use EngineError::GroupNotFound;
use crate::config::GosubEngineConfig;
use crate::errors::EngineError;
use crate::event::EngineEvent;
use crate::tabgroup::{GroupId, TabGroup};
use crate::tab::TabId;
use crate::tick::TickResult;

pub struct GosubEngine {
    config: GosubEngineConfig,              // Configuration for the whole engine
    groups: HashMap<GroupId, TabGroup>,     // List of tabgroups
    pub runtime: Arc<Runtime>,              // Tokio runtime for async operations
}

impl GosubEngine {
    // Initializes a new GosubEngine with the provided configuration.
    pub fn new(config: GosubEngineConfig) -> Self {
        let runtime = Arc::new(
            tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .expect("Failed to create Tokio runtime")
        );

        Self {
            config,
            groups: HashMap::new(),
            runtime,
        }
    }

    // Creates a new tab group, returning its ID or error
    pub fn create_group(&mut self) -> Result<GroupId, EngineError> {
        if self.groups.len() >= self.config.max_groups {
            return Err(EngineError::GroupLimitExceeded);
        }

        let group = TabGroup::new(self.config.tab_group_config.clone());
        let id = group.id;

        self.groups.insert(group.id, group);

        Ok(id)
    }

    // Retrieves a reference to a tab group by its ID
    pub fn get_group_mut(&mut self, group_id: GroupId) -> &mut TabGroup {
        self.groups.get_mut(&group_id).expect("Group not found")
    }

    // Opens a new tab in the specified group, returning its ID
    pub fn open_tab(&mut self, group_id: GroupId) -> Result<TabId, EngineError> {
        let group = self.groups.get_mut(&group_id).ok_or(GroupNotFound)?;
        group.open_tab(self.runtime.clone())
    }

    // Do an engine tick, processing all groups and tabs
    pub fn tick(&mut self) -> BTreeMap<TabId, TickResult> {
        let mut results = BTreeMap::new();

        for group in self.groups.values_mut() {
            for (tab_id, result) in group.tick_tabs() {
                results.insert(tab_id, result);
            }
        }

        results
    }

    // Handle an event for a specific tab
    pub fn handle_event(&mut self, tab_id: TabId, event: EngineEvent) {
        for group in self.groups.values_mut() {
            if let Some(tab) = group.get_tab_mut(tab_id) {
                tab.handle_event(event);
                return;
            }
        }
    }

    pub fn get_surface(&self, tab_id: TabId) -> Option<&cairo::ImageSurface> {
        for group in self.groups.values() {
            if let Some(tab) = group.get_tab(tab_id) {
                return tab.get_surface();
            }
        }
        None
    }
}