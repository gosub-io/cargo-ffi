//! Zone system: [`Zone`], and [`ZoneId`].
//!
//! A **zone** in Gosub is an isolated browsing context that groups together:
//!
//! - A set of [`Tab`](crate::engine::tab::Tab) instances
//! - Shared session/local storage
//! - A cookie jar
//! - Zone-scoped configuration and metadata
//! - Optional password store, bookmarks, autocomplete entries, etc.
//!
//! Zones are the Gosub equivalent of browser profiles. They can be:
//!
//! - **Private** — only the tabs within that zone can access its data.
//! - **Shared** — marked with flags in [`SharedFlags`](crate::engine::zone::Zone) so
//!   other zones can read or write certain datasets (cookies, passwords, etc.).
//!
//! # Key Types
//!
//! - [`Zone`] — The struct representing one zone instance.
//! - [`ZoneId`] — Opaque, globally unique identifier for a zone.
//! - [`ZoneConfig`] — Per-zone configuration settings.
//!
//! # Example
//!
//! Creating a zone with defaults:
//!
//! ```no_run
//! use gosub_engine::GosubEngine;
//!
//! let mut engine = GosubEngine::new(None);
//! let zone_id = engine.zone_builder().create().unwrap();
//! println!("Created zone: {:?}", zone_id);
//! ```
//!
//! Creating a zone with a fixed ID and custom config:
//!
//! ```no_run
//! use gosub_engine::GosubEngine;
//! use gosub_engine::zone::{ZoneConfig, ZoneId};
//!
//! let mut engine = GosubEngine::new(None);
//!
//! let zone_id = engine.zone_builder()
//!     .id(ZoneId::new())
//!     .create()
//!     .unwrap();
//! ```
//!
//! Attaching a persistent cookie jar to a zone:
//!
//! ```no_run
//! use std::sync::{Arc, RwLock};
//! use gosub_engine::cookies::{SqliteCookieStore, PersistentCookieJar, DefaultCookieJar};
//! use gosub_engine::GosubEngine;
//! use gosub_engine::zone::ZoneId;
//!
//! let jar = DefaultCookieJar::new();
//!
//! let mut engine = GosubEngine::new(None);
//! let zone_id = engine.zone_builder()
//!     .cookie_jar(Arc::new(RwLock::new(jar)))
//!     .create()
//!     .unwrap();
//!
//! ```
//!
//! See [`Zone`] docs for field-level details.

mod config;
mod manager;
mod password_store;
mod zone;

pub use config::ZoneConfig;
pub(crate) use manager::ZoneManager;
pub use zone::Zone;
pub use zone::ZoneId;
