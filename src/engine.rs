//! Engine API surface.
//!
//! Most users should start with [`GosubEngine`].

mod context;
mod engine;
mod errors;
pub mod events;

pub mod cookies;
pub mod tab;
pub mod zone;
pub mod storage;

pub mod config;
mod handle;

pub use context::BrowsingContext;
pub use engine::GosubEngine;
pub use errors::EngineError;

/// Default capacity for MPSC channels
const DEFAULT_CHANNEL_CAPACITY: usize = 512;
