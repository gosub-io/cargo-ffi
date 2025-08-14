//! Engine API surface.
//!
//! Most users should start with [`GosubEngine`](crate::engine::GosubEngine).

mod config;
mod instance;
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
pub use event::{EngineCommand, EngineEvent, MouseButton};
pub use instance::EngineInstance;
pub use errors::EngineError;
pub use engine::GosubEngine;
