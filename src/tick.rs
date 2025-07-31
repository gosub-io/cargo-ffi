#[derive(Debug, Default)]
pub enum TabLifecycleStatus {
    #[default]
    Idle,
    Loading,
    Loaded,
    Rendering,
    Rendered,
    Failed(String),
}

#[derive(Default, Debug)]
pub struct TickResult {
    pub status: TabLifecycleStatus,
    pub next_tick_in: Option<std::time::Duration>,
    pub needs_redraw: bool,
    pub page_loaded: bool,
}

#[derive(Default, Debug)]
pub struct DirtyFlags {
    pub render_tree: bool,      // Render tree needs to be rebuilt
    pub layout: bool,           // Layout needs to be recalculated
    pub paint: bool,            // Paint commands need to rebuild
    pub tiles: bool,            // Tiles need to be redrawn
    pub layers: bool,           // Layers need to be redrawn
    pub scroll: bool,           // Scroll position has changed
    pub viewport: bool,         // Viewport has changed
}

impl DirtyFlags {
    pub fn any(&self) -> bool {
        self.render_tree || self.layout || self.paint || self.tiles || self.layers || self.scroll || self.viewport
    }
}