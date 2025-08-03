// A tab is basically a state machine with the following states
#[derive(Debug, Default)]
pub enum TabLifecycleStatus {
    /// Tab is idle and waits for input or external events (javascript, css animations, UX events)
    #[default]
    Idle,
    /// Tab is being loaded with an URL
    Loading,
    /// Tab finished loading
    Loaded,
    /// Tab is being rendered
    Rendering,
    /// Tab has finished rendering and is ready to paint to surface
    Rendered,
    /// Tab is in a failed state (time to show our gosub flappybird! :-))
    Failed(String),
}

// A tick result returns the current state of a tab after a tick() has been processed
#[derive(Default, Debug)]
pub struct TickResult {
    /// Current status of the tab
    pub status: TabLifecycleStatus,
    /// When should we trigger a next tick (not really used for now). Just tick at 16ms/60hz
    pub next_tick_in: Option<std::time::Duration>,
    /// Is the page ready for being redrawn (has rendering completed)
    pub needs_redraw: bool,
    /// Is the page loaded (but not yet rendered, we could extract title/favicon etc)
    pub page_loaded: bool,
}

// Dirty flags define what needs to be processed in the render pipeline
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