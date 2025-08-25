// #![deny(missing_docs)]
// #![deny(rustdoc::broken_intra_doc_links)]

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
//! let backend = gosub_engine::render::backends::null::NullBackend::new();
//! let mut engine = gosub_engine::GosubEngine::new(None, Box::new(backend));
//!
//! // Create a zone (with all default settings)
//! let zone_id = engine.zone_builder().create()?;
//!
//! // Open a tab in the zone
//! let tab_id = engine.open_tab_in_zone(zone_id)?;
//!
//! // Drive the engine and let it render stuff into the compositor
//! let compositor = &mut gosub_engine::render::DefaultCompositor::new(
//!    || { println!("Frame is ready and can be drawn")
//! });
//!
//! // Send events/commands
//! engine.handle_event(tab_id, gosub_engine::EngineEvent::MouseDown{ button: MouseButton::Left, x: 10.0, y: 10.0})?;
//! engine.execute_command(tab_id, gosub_engine::EngineCommand::Navigate(Url::from_str("https://example.com").expect("url")))?;
//!
//! while let Ok(results) = engine.tick(compositor) {
//!   // results contains all the events that happened since the last tick per tab.
//!   // based on its result, you can update the UI etc.
//! }
//!
//! ```
//!
//! ## Concepts
//! - [`GosubEngine`] — the main entry point
//! - [`Zone`](crate::zone::Zone) — user/session context (cookie jar, storage, tabs)
//! - [`Tab`](crate::tab::Tab) — a single tab with a dedicated browsing context
//! - [`Viewport`] — target surface size/information
//! - [`EngineEvent`], [`EngineCommand`] — how you drive tabs
//! - [`BrowsingContext`] — per-tab state (history, active URL, etc.)
//!
//! ## Modules
//! - [`zone`] — zones, ids, zone manager
//! - [`tab`] — tabs and tab ids
//!
//! ## Building docs
//! `cargo doc --open`

extern crate core;

mod engine;
mod net;
/// Rendering system: backends, surfaces, and display lists.
///
/// The [`render`] module contains abstractions for different rendering backends
/// (e.g. Cairo, Vello, Skia), surface handling, and display items used by the
/// engine to paint content into host-provided contexts.
pub mod render;

pub use engine::{EngineCommand, EngineConfig, EngineError, EngineEvent, GosubEngine, MouseButton};

#[doc(inline)]
pub use engine::tab;

#[doc(inline)]
pub use engine::zone;

#[doc(inline)]
pub use engine::cookies;

#[doc(inline)]
pub use engine::storage;

#[doc(inline)]
pub use engine::tick::TickResult;
