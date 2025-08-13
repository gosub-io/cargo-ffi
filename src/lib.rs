#![forbid(unsafe_code)]
// Optional but nice once docs are filled in:
// #![deny(missing_docs)]
// #![deny(rustdoc::broken_intra_doc_links)]

//! # Gosub Engine
//!
//! Gosub is a (work-in-progress) browser engine you can embed in a GTK application.
//!
//! ## Quick start
//!
//! ```rust,no_run
//! use gosub_engine::prelude::*;
//!
//! # fn main() -> Result<(), EngineError> {
//! use std::str::FromStr;
//! use url::Url;
//! use gosub_engine::{MouseButton, Viewport};
//! let mut engine = GosubEngine::new(None);
//! let zone_id = engine.create_zone(None, None, None)?;
//!
//! // Set up your viewport however your app does it
//! let viewport = Viewport::new(0, 0, 800, 600);
//! let tab_id = engine.open_tab(zone_id, &viewport)?;
//!
//! // Drive the engine
//! let _results = engine.tick();
//!
//! // Send events/commands
//! engine.handle_event(tab_id, EngineEvent::MouseDown{ button: MouseButton::Left, x: 10.0, y: 10.0})?;
//! engine.execute_command(tab_id, EngineCommand::Navigate(Url::from_str("https://example.com").expect("url")))?;
//!
//! // Read back the rendered surface
//! let _surface = engine.get_surface(tab_id);
//! # Ok(()) }
//! ```
//!
//! ## Concepts
//! - [`GosubEngine`] — the main entry point
//! - [`Zone`](zone::zone::Zone) — user/session context (cookie jar, storage, tabs)
//! - [`Tab`](tab::Tab) — a single browsing context with an engine instance
//! - [`Viewport`] — target surface size/information
//! - [`EngineEvent`], [`EngineCommand`] — how you drive tabs
//!
//! ## Modules
//! - [`engine`] — public API surface (re-exports types you need)
//! - [`zone`] — zones, ids, zone manager
//! - [`tab`] — tabs and tab ids
//! - [`tick`] — ticking the engine and results
//! - [`viewport`] — viewport data
//! - [`net`] — networking (internal; subject to change)
//!
//! ## Building docs
//! `cargo doc --open`


mod engine;
mod viewport;
mod net;

pub use engine::{
    EngineConfig, ZoneConfig, EngineCommand, EngineEvent, MouseButton,
    EngineInstance, EngineError, GosubEngine,
};
pub use viewport::Viewport;

pub use engine::tab::TabId;
pub use engine::tab::TabMode;
pub use engine::zone::ZoneId;

pub mod prelude {
    pub use crate::{
        GosubEngine, EngineConfig, EngineCommand, EngineEvent, EngineError
    };
}

pub mod tab {
    pub use crate::{
        TabId, TabMode
    };
}

pub mod zone {
    pub use crate::{
        ZoneConfig, ZoneId,
    };
}

pub mod cookies {
    pub use crate::engine::cookies::{
        Cookie, CookieJar, DefaultCookieJar,
        CookieStore, JsonCookieStore, SqliteCookieStore,
        PersistentCookieJar,
    };
}

pub mod storage {
    pub use crate::engine::storage::{
        StorageService, SqliteLocalStore, InMemorySessionStore,
    };
}