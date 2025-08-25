//! Viewport definition for rendering.
//!
//! A [`Viewport`] describes the rectangular region of a page that should be
//! rendered. It is defined by its top-left corner `(x, y)` and its pixel
//! `width` and `height`. The engine and backends use this to determine what
//! part of a [`Tab`](crate::tab::Tab) to paint and at what size.
//!
//! The coordinate system is engine-defined. Typically `(0, 0)` refers to the
//! top-left corner of the root surface or window.
//!
//! # Examples
//!
//! Creating a viewport and passing it to a new tab:
//! ```
//! use gosub_engine::render::Viewport;
//!
//! // 800x600 at the origin
//! let viewport = Viewport::new(0, 0, 800, 600);
//! ```
//!
//! Resizing and moving a viewport:
//! ```
//! use gosub_engine::render::Viewport;
//!
//! let mut vp = Viewport::new(0, 0, 800, 600);
//! vp.resize(1024, 768);
//! vp.translate(10, 20);
//! assert_eq!(vp.width, 1024);
//! assert_eq!(vp.x, 10);
//! assert_eq!(vp.y, 20);
//! ```
//!
//! Computing aspect ratio:
//! ```
//! use gosub_engine::render::Viewport;
//!
//! let vp = Viewport::new(0, 0, 1920, 1080);
//! assert_eq!(vp.aspect_ratio(), 1920.0 / 1080.0);
//! ```
//!
//! Converting to a [`SurfaceSize`] for backend use:
//! ```
//! use gosub_engine::render::{Viewport, backend::SurfaceSize};
//!
//! let vp = Viewport::new(0, 0, 1280, 720);
//! let size: SurfaceSize = vp.as_size();
//! assert_eq!(size.width, 1280);
//! ```

use crate::render::backend::SurfaceSize;

/// Represents the viewport for rendering.
#[derive(Clone, Eq, PartialEq, Copy)]
pub struct Viewport {
    /// Horizontal offset in pixels from the origin.
    pub x: i32,

    /// Vertical offset in pixels from the origin.
    pub y: i32,

    /// Width in pixels.
    pub width: u32,

    /// Height in pixels.
    pub height: u32,
}

impl Default for Viewport {
    fn default() -> Self {
        Self {
            x: 0,
            y: 0,
            width: 0,
            height: 0,
        }
    }
}

impl std::fmt::Debug for Viewport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Viewport {{ x: {}, y: {}, width: {}, height: {} }}",
            self.x, self.y, self.width, self.height
        )
    }
}

impl Viewport {
    /// Creates a new [`Viewport`] with the given position and size.
    pub fn new(x: i32, y: i32, width: u32, height: u32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Resizes the viewport to the given width and height.
    pub fn resize(&mut self, width: u32, height: u32) {
        self.width = width;
        self.height = height;
    }

    /// Moves the viewport’s origin to `(x, y)` in pixels.
    pub fn translate(&mut self, x: i32, y: i32) {
        self.x = x;
        self.y = y;
    }

    /// Returns the aspect ratio (`width / height`) as `f32`.
    ///
    /// Returns `0.0` if `height` is `0` to avoid division by zero.
    pub fn aspect_ratio(&self) -> f32 {
        if self.height == 0 {
            0.0
        } else {
            self.width as f32 / self.height as f32
        }
    }

    /// Converts this viewport to a [`SurfaceSize`].
    pub fn as_size(&self) -> SurfaceSize {
        SurfaceSize {
            width: self.width,
            height: self.height,
        }
    }
}
