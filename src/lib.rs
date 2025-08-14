#![forbid(unsafe_code)]
// Optional but nice once docs are filled in:
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
//! use gosub_engine::{EngineError, MouseButton, Viewport};
//!
//! let mut engine = gosub_engine::GosubEngine::new(None);
//! let zone_id = engine.zone().create()?;
//!
//! // Set up your viewport however your app does it
//! let viewport = Viewport::new(0, 0, 800, 600);
//! let tab_id = engine.open_tab(zone_id, &viewport)?;
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
//! - [`Tab`](crate::tab::Tab) — a single browsing context with an engine instance
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
mod viewport;
mod net;

pub use engine::{
    EngineConfig, EngineCommand, EngineEvent, MouseButton,
    EngineInstance, EngineError, GosubEngine,
};

pub use viewport::Viewport;

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
