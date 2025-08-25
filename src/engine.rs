//! Engine API surface.
//!
//! Most users should start with [`GosubEngine`](crate::engine::GosubEngine).

mod config;
mod context;
pub mod cookies;
mod engine;
mod errors;
mod event;
pub mod storage;
pub mod tab;
pub mod tick;
pub mod zone;
mod zone_builder;

pub use config::EngineConfig;
pub use context::BrowsingContext;
pub use engine::GosubEngine;
pub use errors::EngineError;
pub use event::{EngineCommand, EngineEvent, MouseButton};
