//! Ticking and render pipeline state.
//!
//! This module defines the data structures returned or updated when the
//! engine advances a [`Tab`](crate::tab::Tab) by one `tick()`.
//!
//! A “tick” is a single iteration of the engine’s scheduling loop for a tab.
//! Each tick processes the tab’s current [`TabState`](crate::tab::TabState),
//! performs any pending network/layout/rendering work, and produces a [`TickResult`].
//!
//! The render pipeline can also use [`DirtyFlags`] to track which stages need
//! rebuilding or repainting.
//!
//! # Typical flow
//!
//! ```no_run
//! use gosub_engine::{GosubEngine, Viewport};
//!
//! let mut engine = GosubEngine::new(None);
//! let zone_id = engine.zone_builder().create().unwrap();
//! let tab_id = engine.open_tab_in_zone(zone_id, &Viewport::new(0, 0, 800, 600)).unwrap();
//!
//! // Drive the engine
//! let results = engine.tick();
//! if let Some(res) = results.get(&tab_id) {
//!     if res.page_loaded {
//!         println!("Page committed: {:?}", res.commited_url);
//!     }
//!     if res.needs_redraw {
//!         // Schedule repaint
//!     }
//! }
//! ```
use crate::engine::tab::TabState;

/// Result of processing a single [`Tab`](crate::engine::tab::Tab) tick.
///
/// Returned from [`Tab::tick`](crate::engine::tab::Tab::tick) and collected by
/// [`GosubEngine::tick`](crate::GosubEngine::tick).
#[derive(Default, Debug)]
pub struct TickResult {
    /// Current [`TabState`] after this tick.
    pub status: TabState,

    /// Suggested time until the next tick.
    ///
    /// Not currently used; engine defaults to ~16 ms (≈60 Hz).
    pub next_tick_in: Option<std::time::Duration>,

    /// Whether the page has a fresh surface ready to paint.
    pub needs_redraw: bool,

    /// Whether the main document has committed (loaded), even if not yet painted.
    ///
    /// Use this to trigger title/favicon extraction or similar.
    pub page_loaded: bool,

    /// URL that was just committed by this tick, if any.
    pub commited_url: Option<url::Url>,
}


/// “Dirty” flags for the render pipeline.
///
/// Each flag corresponds to a stage in the pipeline that needs to be rebuilt,
/// recalculated, or repainted.
#[derive(Default, Debug)]
#[allow(unused)]
pub struct DirtyFlags {
    /// Render tree needs to be rebuilt.
    pub render_tree: bool,

    /// Layout needs to be recalculated.
    pub layout: bool,

    /// Paint commands need to be regenerated.
    pub paint: bool,

    /// Tiles need to be redrawn.
    pub tiles: bool,

    /// Layers need to be re-rasterized.
    pub layers: bool,

    /// Scroll position has changed.
    pub scroll: bool,

    /// Viewport size or position has changed.
    pub viewport: bool,
}