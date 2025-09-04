// #![deny(missing_docs)]
// #![deny(rustdoc::broken_intra_doc_links)]

//! # Gosub Engine
//!
//! Gosub is a work-in-progress, embeddable browser engine for building your own User Agent (UA).
//! It uses **async channels** and **handles**:
//! - `EngineEvent` flows from the engine → UA over an event channel.
//! - You control things via `EngineCommand` (engine/zone scoped) and `TabCommand` (tab scoped).
//!
//! ## Quick start (async, handles & channels)
//!
//! ```rust,no_run
//! use std::sync::Arc;
//! use url::Url;
//!
//! use gosub_engine::{EngineConfig, GosubEngine};
//! use gosub_engine::render::Viewport;
//! use gosub_engine::render::backends::null::NullBackend;
//! use gosub_engine::events::{EngineEvent, TabCommand};
//! use gosub_engine::storage::{StorageService, InMemoryLocalStore, InMemorySessionStore, PartitionPolicy};
//! use gosub_engine::cookies::DefaultCookieJar;
//! use gosub_engine::zone::{ZoneConfig, ZoneServices};
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     // 1) Engine + backend
//!     let backend = NullBackend::new().expect("null renderer cannot be created (!?)");
//!     let mut engine = GosubEngine::new(Some(EngineConfig::default()), Box::new(backend));
//!
//!     // 2) Event channel: UA keeps `event_rx` to receive EngineEvent;
//!     //    engine/zones/tabs clone `event_tx` to send events.
//!     let (event_tx, mut event_rx) = engine.create_event_channel(1024);
//!
//!     // 3) Zone services (ephemeral cookies here; use a CookieStore for persistence)
//!     let services = ZoneServices {
//!         storage: Arc::new(StorageService::new(
//!             Arc::new(InMemoryLocalStore::new()),
//!             Arc::new(InMemorySessionStore::new()),
//!         )),
//!         cookie_store: None,
//!         cookie_jar: Some(DefaultCookieJar::new().into()),
//!         partition_policy: PartitionPolicy::None,
//!     };
//!
//!     // 4) Create a zone (ZoneHandle)
//!     let zone = engine
//!         .create_zone(ZoneConfig::default(), services, None, event_tx)?;
//!
//!     // 5) Create a tab (TabHandle)
//!     let tab = zone.create_tab(Default::default(), None).await?;
//!
//!     // 6) Drive the tab
//!     tab.send(TabCommand::Navigate{ url: "https://example.com".to_string() }).await?;
//!     tab.send(TabCommand::SetViewport{ x: 0, y: 0, width: 1280, height: 800 }).await?;
//!
//!     // 7) Handle engine events in your UA
//!     while let Some(ev) = event_rx.recv().await {
//!         match ev {
//!             EngineEvent::LoadStarted { tab_id, url } => {
//!                 println!("[{tab_id:?}] Starting loading: {url}");
//!             }
//!             EngineEvent::Redraw { tab_id, .. } => {
//!                 // Composite `handle` into your UI
//!                 println!("[{tab_id:?}] Redraw requested");
//!             }
//!             _ => {}
//!         }
//!     }
//!
//!     Ok(())
//! }
//! ```
//!
//! ## Concepts
//! - [`GosubEngine`] — engine entry point; creates zones, owns backend and event bus.
//! - **Event channel** — `(event_tx, event_rx) = engine.create_event_channel()`; UA keeps `event_rx`.
//! - [`Zone`](crate::engine::zone::Zone) / **ZoneHandle** — per-profile/session state (cookies, storage, tabs).
//! - **Tab task** / **TabHandle** — a single browsing context controlled via [`TabCommand`](crate::events::TabCommand).
//! - [`Viewport`](crate::render::Viewport) — target surface description for rendering.
//! - [`RenderBackend`](crate::render::backend::RenderBackend) — pluggable renderer (e.g., Null, Cairo, Vello).
//!
//! ## Persistence
//! To persist cookies, pass a [`CookieStore`](crate::cookies::CookieStore) in
//! `ZoneServices::cookie_store` and omit `cookie_jar`; the engine will attach a per-zone
//! [`PersistentCookieJar`](crate::cookies::PersistentCookieJar).
//!
//! ## Modules
//! - [`engine::zone`](crate::engine::zone)
//! - [`engine::tab`](crate::engine::tab)
//! - [`engine::cookies`](crate::engine::cookies)
//! - [`engine::storage`](crate::engine::storage)
//! - [`render`](crate::render)
//! - [`net`](crate::net)
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

pub mod events {
    pub use crate::engine::events::{EngineCommand, EngineEvent, MouseButton, TabCommand};
}

// Public `config` namespace with the enums/structs:
/// Configuration options for the Gosub engine.
pub mod config {
    pub use crate::engine::config::{
        CookiePartitioning, GpuOptions, LogLevel, ProxyConfig, RedirectPolicy, SandboxMode,
        TlsConfig,
    };
}
