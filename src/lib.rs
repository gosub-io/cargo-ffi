// #![forbid(unsafe_code)]
// Optional but nice once docs are filled in:
#![deny(missing_docs)]
#![deny(rustdoc::broken_intra_doc_links)]

//! # Gosub Engine
//!
//! Gosub is a (work-in-progress) browser engine you can embed in your own custom user agent.
//!
//! ## Quick start
//!
//! ```rust,no_run
//!
//! # fn main() -> Result<(), gosub_engine::EngineError> {
//! use std::str::FromStr;
//! use url::Url;
//! use gosub_engine::{EngineError, MouseButton};
//!
//! let mut engine = gosub_engine::GosubEngine::new(None);
//! let zone_id = engine.zone_builder().create()?;
//!
//! // Set up your viewport however your app does it
//! let tab_id = engine.open_tab_in_zone(zone_id)?;
//!
//! // Drive the engine
//! let _results = engine.tick();
//!
//! // Send events/commands
//! engine.handle_event(tab_id, gosub_engine::EngineEvent::MouseDown{ button: MouseButton::Left, x: 10.0, y: 10.0})?;
//! engine.execute_command(tab_id, gosub_engine::EngineCommand::Navigate(Url::from_str("https://example.com").expect("url")))?;
//!
//! // Read back the rendered surface
//! let _surface = engine.get_surface(tab_id);
//! # Ok(()) }
//! ```
//!
//! ## Concepts
//! - [`GosubEngine`] — the main entry point
//! - [`Zone`](crate::zone::Zone) — user/session context (cookie jar, storage, tabs)
//! - [`Tab`](crate::tab::Tab) — a single tab with a dedicated browsing context
//! - [`Viewport`] — target surface size/information
//! - [`EngineEvent`], [`EngineCommand`] — how you drive tabs
//!
//! ## Modules
//! - [`zone`] — zones, ids, zone manager
//! - [`tab`] — tabs and tab ids
//!
//! ## Building docs
//! `cargo doc --open`


mod engine;
mod net;
/// Rendering system: backends, surfaces, and display lists.
///
/// The [`render`] module contains abstractions for different rendering backends
/// (e.g. Cairo, Vello, Skia), surface handling, and display items used by the
/// engine to paint content into host-provided contexts.
pub mod render;

pub use engine::{
    EngineConfig, EngineCommand, EngineEvent, MouseButton, EngineError, GosubEngine,
};

#[doc(inline)]
pub use engine::tab as tab;

#[doc(inline)]
pub use engine::zone as zone;

#[doc(inline)]
pub use engine::cookies as cookies;

#[doc(inline)]
pub use engine::storage as storage;

#[doc(inline)]
pub use engine::tick::TickResult as TickResult;
