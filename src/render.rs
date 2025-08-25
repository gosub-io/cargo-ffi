pub mod backend;

/// Rendering backends for the Gosub engine.
pub mod backends {
    pub mod null;
    /// Cairo rendering backend
    #[cfg(feature = "backend_cairo")]
    pub mod cairo;
    /// Vello rendering backend
    #[cfg(feature = "backend_vello")]
    pub mod vello;
}

mod render_list;
pub use render_list::*;

mod viewport;
pub use viewport::Viewport;

mod compositor;
pub use compositor::{DefaultCompositor};
