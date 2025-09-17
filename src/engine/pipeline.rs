//! Pipeline modules for processing different asset types.
//!
//! Each module defines a trait for parsing streams and byte slices of the respective asset type.

pub mod html;
pub mod css;
pub mod js;
pub mod font;
pub mod image;