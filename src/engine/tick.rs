// src/engine/tick.rs
//! Tab system: [`Tab`], ['Tick'], and [`TabId`].
//!
use crate::engine::tab::TabState;

// A tick result returns the current state of a tab after a tick() has been processed
#[derive(Default, Debug)]
pub struct TickResult {
    /// Current status of the tab
    pub status: TabState,
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