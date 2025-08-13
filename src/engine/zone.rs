// src/engine/zone.rs
//! Zone system: [`ZoneManager`], [`Zone`], and [`ZoneId`].
//!
mod manager;
mod password_store;
mod zone;

pub(crate) use manager::ZoneManager;
pub use zone::ZoneId;
pub use zone::Zone;

