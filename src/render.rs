
pub mod backend;

/// Rendering backends for the Gosub engine.
pub mod backends {
    /// Cairo rendering backend
    #[cfg(feature = "backend_cairo")] pub mod cairo;
    #[cfg(feature = "backend_vello")] pub mod vello;
    #[cfg(feature = "backend_skia")] pub mod skia;
}

mod render_list;
pub use render_list::*;

mod viewport;

pub use viewport::Viewport as Viewport;
