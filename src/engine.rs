//! Engine API surface.
//!
//! Most users should start with [`GosubEngine`](crate::engine::GosubEngine).

mod config;
mod context;
mod event;
mod errors;
pub mod tick;
pub mod zone;
pub mod tab;
mod engine;
pub mod cookies;
#[allow(unused)]
pub mod storage;
mod zone_builder;

pub use config::EngineConfig;
pub use context::BrowsingContext;
pub use engine::GosubEngine;
pub use errors::EngineError;
pub use event::{EngineCommand, EngineEvent, MouseButton};
