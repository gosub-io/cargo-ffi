//! Render list and display items.
//!
//! This module defines a lightweight, immediate-style render list
//! consisting of [`DisplayItem`] commands. It acts as a temporary
//! system for testing and prototyping before the full render pipeline
//! is integrated.
//!
//! The core type is [`RenderList`], which collects a sequence of
//! display items such as rectangles, text runs, or clears, and can
//! later be consumed by a compositor or renderer.
//!
//! # Example
//!
//! ```rust
//! use gosub_engine::render::render_list::{RenderList, DisplayItem, Color};
//!
//! let mut list = RenderList::new();
//!
//! // Clear background
//! list.add_command(DisplayItem::Clear { color: Color::from_u8(0, 0, 0, 255) });
//!
//! // Draw a white rectangle
//! list.add_command(DisplayItem::Rect {
//!     x: 10.0,
//!     y: 20.0,
//!     w: 100.0,
//!     h: 50.0,
//!     color: Color::from_u8(255, 255, 255, 255),
//! });
//! ```

/// RGBA color used for drawing commands.
///
/// Channels are represented as `f32` in the range `0.0 ..= 1.0`.
#[derive(Debug, Clone, Copy)]
pub struct Color {
    /// Red channel
    pub r: f32,
    /// Green channel
    pub g: f32,
    /// Blue channel
    pub b: f32,
    /// Alpha channel (opacity)
    pub a: f32,
}

#[allow(unused)]
impl Color {
    /// Creates a new color from `f32` channel values in the range `0.0 ..= 1.0`.
    pub fn new(r: f32, g: f32, b: f32, a: f32) -> Color {
        Color {
            r: r as f32,
            g: g as f32,
            b: b as f32,
            a: a as f32,
        }
    }

    /// Creates a new color from `u8` channel values in the range `0 ..= 255`.
    pub fn from_u8(r: u8, g: u8, b: u8, a: u8) -> Color {
        Color {
            r: r as f32 / 255.0,
            g: g as f32 / 255.0,
            b: b as f32 / 255.0,
            a: a as f32 / 255.0,
        }
    }

    /// Returns the red channel as an `u8` (0–255).
    fn r_u8(&self) -> u8 {
        (self.r * 255.0) as u8
    }
    /// Returns the green channel as an `u8` (0–255).
    fn g_u8(&self) -> u8 {
        (self.g * 255.0) as u8
    }
    /// Returns the blue channel as an `u8` (0–255).
    fn b_u8(&self) -> u8 {
        (self.b * 255.0) as u8
    }
    /// Returns the alpha channel as an `u8` (0–255).
    fn a_u8(&self) -> u8 {
        (self.a * 255.0) as u8
    }
}

/// A single display item representing a drawing command.
///
/// These commands are appended to a [`RenderList`] and later processed
/// by the render backend.
///
/// Variants:
/// - [`Clear`] — clear the entire surface to a color.
/// - [`Rect`] — draw a solid rectangle.
/// - [`TextRun`] — draw a run of text at a position.
#[derive(Clone, Debug)]
pub enum DisplayItem {
    /// Clear the entire surface with the given color.
    Clear {
        /// The color to clear the surface with.
        color: Color,
    },

    /// Draw a filled rectangle at `(x, y)` with width `w` and height `h`.
    Rect {
        /// The x-coordinate of the rectangle's top-left corner.
        x: f32,
        /// The y-coordinate of the rectangle's top-left corner.
        y: f32,
        /// The width of the rectangle.
        w: f32,
        /// The height of the rectangle.
        h: f32,
        /// The color to fill the rectangle with.
        color: Color,
    },

    /// Draw a text run at `(x, y)` with font size `size`.
    TextRun {
        /// The x-coordinate where the text starts.
        x: f32,
        /// The y-coordinate where the text starts.
        y: f32,
        /// The text to render.
        text: String,
        /// The font size to use for the text.
        size: f32,
        /// The color to render the text with.
        color: Color,
    },
}

/// A list of display items to be rendered.
///
/// Collects commands during layout/painting that will be consumed
/// by the rendering backend or compositor.
#[derive(Clone, Debug, Default)]
pub struct RenderList {
    /// Sequence of drawing commands to execute.
    pub items: Vec<DisplayItem>,
}

impl RenderList {
    /// Creates a new, empty render list.
    pub fn new() -> Self {
        RenderList { items: Vec::new() }
    }

    /// Adds a new display item (drawing command) to the list.
    pub fn add_command(&mut self, command: DisplayItem) {
        self.items.push(command);
    }

    /// Clears all display items from the list.
    pub fn clear(&mut self) {
        self.items.clear();
    }
}
