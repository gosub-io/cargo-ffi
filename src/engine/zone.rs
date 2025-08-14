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
//! let zone_id = engine.zone().create().unwrap();
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
//! let zone_id = engine.zone()
//!     .id(ZoneId::new())
//!     .create()
//!     .unwrap();
//! ```
//!
//! Attaching a persistent cookie jar to a zone:
//!
//! ```no_run
//! use std::sync::Arc;
//! use gosub_engine::cookies::{SqliteCookieStore, PersistentCookieJar};
//! use gosub_engine::GosubEngine;
//! use gosub_engine::zone::ZoneId;
//!
//! let mut engine = GosubEngine::new(None);
//! let zone_id = engine.zone().id(ZoneId::new()).create().unwrap();
//!
//! let jar = PersistentCookieJar::new(Arc::new(SqliteCookieStore::new("cookies.db".into())));
//! let zone_arc = engine.get_zone_mut(zone_id).unwrap();
//! zone_arc.lock().unwrap().set_cookie_jar(jar);
//! ```
//!
//! See [`Zone`] docs for field-level details.

mod manager;
mod password_store;
mod zone;
mod config;

pub(crate) use manager::ZoneManager;
pub use zone::ZoneId;
pub use zone::Zone;
pub use config::ZoneConfig;