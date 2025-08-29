//! Engine API surface.
//!
//! Most users should start with [`GosubEngine`].

mod context;
mod engine;
mod errors;
mod events;

pub mod cookies;
pub mod tab;
pub mod zone;
pub mod storage;

pub mod config;
mod handle;

pub use context::BrowsingContext;
pub use engine::GosubEngine;
pub use errors::EngineError;
