//! Engine API surface.
//!
//! Most users should start with [`GosubEngine`].

mod context;
mod engine;
mod errors;
pub mod events;

pub mod cookies;
pub mod storage;
pub mod tab;
pub mod zone;

pub mod config;
pub mod types;
mod downloader;
mod policy;

pub use context::BrowsingContext;
pub use engine::GosubEngine;
pub use errors::EngineError;

pub use policy::UaPolicy;

/// Default capacity for MPSC channels
const DEFAULT_CHANNEL_CAPACITY: usize = 512;
