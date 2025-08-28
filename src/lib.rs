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
//! use std::thread::sleep;
//! use url::Url;
//! use gosub_engine::{EngineConfig, EngineError, MouseButton};
//! use gosub_engine::render::Viewport;
//!
//! let backend = gosub_engine::render::backends::null::NullBackend::new().expect("null renderer cannot be created (!?)");
//!
//! let mut engine = gosub_engine::GosubEngine::new(None, Box::new(backend));
//!
//! // Create a zone (with all default settings)
//! let zone_id = engine.zone_builder().create()?;
//!
//! // Open a tab in the zone
//! let viewport = Viewport::new(0, 0, 800, 600);
//!
//! let tab_id = engine.open_tab_in_zone(zone_id, viewport)?;
//!
//! // Drive the engine and let it render stuff into the compositor
//! let compositor = &mut gosub_engine::render::DefaultCompositor::new(
//!     || { println!("Frame is ready and can be drawn") }
//! );
//!
//! // Send events/commands
//! engine.handle_event(tab_id, gosub_engine::EngineEvent::MouseDown{ button: MouseButton::Left, x: 10.0, y: 10.0})?;
//! engine.execute_command(tab_id, gosub_engine::EngineCommand::Navigate(Url::from_str("https://example.com").expect("url")))?;
//!
//! loop {
//!    let results = engine.tick(compositor);
//!    for (_tab_id, tick_result) in &results {
//!        if tick_result.page_loaded {
//!            println!("Page has been loaded: {}", tick_result.commited_url.clone().unwrap().to_string());
//!        }
//!        if tick_result.needs_redraw {
//!            println!("Page is rendered and can be drawn on the screen");
//!        }
//!    }
//!    sleep(std::time::Duration::from_millis(100));
//!  }
//! # Ok(())
//! # }
//!
//! ```
//!
//! ## Concepts
//! - [`GosubEngine`] — the main entry point
//! - [`Zone`](crate::zone::Zone) — user/session context (cookie jar, storage, tabs)
//! - [`Tab`](crate::tab::Tab) — a single tab with a dedicated browsing context
//! - [`Viewport`](crate::render::Viewport) — target surface size/information
//! - [`EngineEvent`], [`EngineCommand`] — how you drive tabs
//! - `BrowsingContext` — per-tab state (history, active URL, etc.)
//!
//! ## Modules
//! - [`zone`] — zones, ids, zone manager
//! - [`tab`] — tabs and tab ids
//!
//! ## Building docs
//! `cargo doc --open`

extern crate core;

mod engine;

pub mod net;

pub mod render;

pub use engine::{EngineError, GosubEngine};

#[doc(inline)]
pub use engine::tab;

#[doc(inline)]
pub use engine::zone;

#[doc(inline)]
pub use engine::cookies;

#[doc(inline)]
pub use engine::storage;

// EngineConfig at crate root:
#[doc(inline)]
pub use crate::engine::config::EngineConfig;

// Public `config` namespace with the enums/structs:

/// Configuration options for the Gosub engine.
pub mod config {
    pub use crate::engine::config::{
        CookiePartitioning,
        RedirectPolicy,
        ProxyConfig,
        TlsConfig,
        GpuOptions,
        LogLevel,
        SandboxMode,
    };
}

