//! Engine API surface.
//!
//! Most users should start with [`GosubEngine`].

mod context;
mod engine;
mod errors;
mod event;
mod zone_builder;

pub mod cookies;
pub mod tab;
pub mod tick;
pub mod zone;
pub mod storage;

pub mod config;

pub use context::BrowsingContext;
pub use engine::GosubEngine;
pub use errors::EngineError;
pub use event::{EngineCommand, EngineEvent, MouseButton};
