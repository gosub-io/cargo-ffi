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
//!     let mut engine_handle = GosubEngine::new(Some(EngineConfig::default()), Box::new(backend));
//!
//!     // 2) Zone services (ephemeral cookies here; use a CookieStore for persistence)
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
//!     // 3) Create a zone (ZoneHandle)
//!     let mut zone = engine_handle.create_zone(ZoneConfig::default(), services, None)?;
//!
//!     // 4) Create a tab (TabHandle)
//!     let tab_handle = zone.create_tab(Default::default(), None).await?;
//!
//!     // 5) Drive the tab
//!     tab_handle.send(TabCommand::Navigate{ url: "https://example.com".to_string() }).await?;
//!     tab_handle.send(TabCommand::SetViewport{ x: 0, y: 0, width: 1280, height: 800 }).await?;
//!
//!     // 6) Handle engine events in your UA
//!     let mut event_rx = engine_handle.subscribe_events();
//!     while let Ok(ev) = event_rx.recv().await {
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
//!     engine_handle.shutdown().await;
//!
//!     Ok(())
//! }
//! ```
//!
//! ## Concepts
//! - [`GosubEngine`] — engine entry point; creates zones, owns backend and event bus.
//! - [`Zone`](crate::zone::Zone) / **ZoneHandle** — per-profile/session state (cookies, storage, tabs).
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
//! - [`zone`](crate::zone)
//! - [`tab`](crate::tab)
//! - [`cookies`](crate::cookies)
//! - [`storage`](crate::storage)
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
/// Tab management and browsing context API.
pub use engine::tab;

#[doc(inline)]
/// Per-profile/session state (cookies, storage, tabs).
pub use engine::zone;

#[doc(inline)]
/// Cookie handling and storage.
pub use engine::cookies;

#[doc(inline)]
/// Storage APIs for local/session data.
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

// Advice: When using the Gosub engine, always ensure that your async runtime (e.g., Tokio) is properly initialized before interacting with engine handles or channels. Carefully manage the lifetimes of handles and event receivers to avoid resource leaks. For persistent cookies or storage, provide the appropriate services in [`ZoneServices`](crate::zone::ZoneServices).
