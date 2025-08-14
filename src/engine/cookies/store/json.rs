//! JSON-backed cookie store.
//!
//! `JsonCookieStore` persists **all zones'** cookie jars in a single JSON file on disk.
//! It implements the [`CookieStore`] trait and returns per-zone jars wrapped in
//! [`PersistentCookieJar`], so that **every mutation** to a jar triggers a snapshot
//! write back to this store.
//!
//! ### Design
//! - One file for all zones (`CookieStoreFile { zones: HashMap<ZoneId, DefaultCookieJar> }`).
//! - In-memory cache: `jars: RwLock<HashMap<ZoneId, CookieJarHandle>>` for quick reuse.
//! - The store keeps a self handle (`store_self`) so the persistent jars can call
//!   back into `persist_zone_from_snapshot`.
//!
//! ### Concurrency
//! - This type is internally synchronized via `RwLock`s and is `Send + Sync` behind
//!   a `CookieStoreHandle = Arc<dyn CookieStore + Send + Sync>`.
//! - Returned jars are `Arc<RwLock<_>>` and safe to share across threads.
//!
//! ### I/O characteristics & caveats
//! - `persist_zone_from_snapshot` and `remove_zone` **read then rewrite** the entire
//!   JSON file. For large datasets, consider an SQLite-backed store.
//! - File writes are not atomic.
//! - Several helpers use `expect(...)` and will **panic** on I/O/serialization errors.
//!
//! ### Example
//! ```ignore
//! let store = JsonCookieStore::new("cookies.json".into());
//!
//! // New zones will receive a PersistentCookieJar minted by this store.
//! let zone_id = engine.zone().cookie_store(store).create()?;
//! ```
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use serde::{Deserialize, Serialize};
use crate::engine::cookies::{CookieJarHandle, CookieStoreHandle};
use crate::engine::cookies::cookie_jar::DefaultCookieJar;
use crate::engine::cookies::store::CookieStore;
use crate::engine::cookies::persistent_cookie_jar::PersistentCookieJar;
use crate::engine::zone::ZoneId;


/// On-disk representation of all zones' cookie jars.
///
/// This is the JSON payload stored at `JsonCookieStore::path`.
#[derive(Debug, Serialize, Deserialize)]
struct CookieStoreFile {
    zones: HashMap<ZoneId, DefaultCookieJar>,
}

/// A JSON-based cookie store that persists cookies across sessions.
///
/// The store caches per-zone jars in memory and loads/saves them to a single JSON file.
/// Jars returned by this store are wrapped in [`PersistentCookieJar`], so that writes
/// automatically trigger persistence to disk.
pub struct JsonCookieStore {
    /// Path to the JSON file where cookies are stored.
    path: PathBuf,

    /// Actual list of cookie jars per zone
    jars: RwLock<HashMap<ZoneId, CookieJarHandle>>,

    /// Self handle, so `PersistentCookieJar` can call back into this store.
    ///
    /// This is initialized in [`new`](Self::new) and then read-only thereafter.
    store_self: RwLock<Option<CookieStoreHandle>>,
}

impl JsonCookieStore {
    /// Creates (or opens) a JSON cookie store at `path`.
    ///
    /// If the file does not exist, an empty structure is written to disk.
    ///
    /// # Panics
    /// Panics if the initial write of an empty file fails.
    pub fn new(path: PathBuf) -> Arc<Self> {
        // Try to create empty file if it doesn't exist
        if !path.exists() {
            let _ = fs::write(&path, serde_json::to_vec(&CookieStoreFile { zones: HashMap::new() }).unwrap());
        }

        let store = Arc::new(Self {
            path,
            jars: RwLock::new(HashMap::new()),
            store_self: RwLock::new(None),
        });

        {
            let mut self_ref = store.store_self.write().unwrap();
            *self_ref = Some(store.clone() as CookieStoreHandle);
        }

        store
    }


    /// Loads and deserializes the full cookie store file.
    ///
    /// Returns an empty structure if deserialization fails.
    ///
    /// # Panics
    /// Panics if the file cannot be opened or read.
    fn load_file(&self) -> CookieStoreFile {
        let mut file = File::open(&self.path).expect("Failed to open cookie store file");
        let mut contents = String::new();
        file.read_to_string(&mut contents).expect("Failed to read cookie store file");

        serde_json::from_str(&contents).unwrap_or_else(|_| CookieStoreFile { zones: HashMap::new() })
    }

    /// Serializes and writes the full cookie store file (pretty-printed).
    ///
    /// # Panics
    /// Panics if serialization or writing fails.
    fn save_file(&self, store_file: &CookieStoreFile) {
        let contents = serde_json::to_string_pretty(store_file).expect("Failed to serialize cookies");
        let mut file = File::create(&self.path).expect("Failed to open cookie store file for writing");
        file.write_all(contents.as_bytes()).expect("Failed to write cookie store file");
    }
}

impl CookieStore for JsonCookieStore {
    /// Returns the cookie jar handle for `zone_id`, creating it if needed.
    ///
    /// Behavior:
    /// - If a jar for `zone_id` exists in the in-memory cache, it is returned.
    /// - Otherwise, a serialized jar is loaded from disk (if present) or an empty
    ///   [`DefaultCookieJar`] is created.
    /// - That jar is wrapped in a [`PersistentCookieJar`] bound to this store
    ///   (via `store_self`) so that subsequent mutations persist automatically.
    ///
    /// Always returns `Some(_)` for valid inputs; `None` is reserved for stores
    /// that may intentionally refuse provisioning.
    fn jar_for(&self, zone_id: ZoneId) -> Option<CookieJarHandle> {
        {
            // Fast path: already in memory
            let jars = self.jars.read().unwrap();
            if let Some(jar) = jars.get(&zone_id) {
                return Some(jar.clone());
            }
        }

        // Load from disk
        let mut file = self.load_file();
        let jar = file.zones.remove(&zone_id).unwrap_or_else(DefaultCookieJar::new);
        let arc_jar: CookieJarHandle = Arc::new(RwLock::new(jar));

        let store_ref = self.store_self.read().unwrap();
        let store = store_ref.as_ref().expect("store_self not initialized").clone();

        // Wrap in PersistentCookieJar
        let persistent = Arc::new(RwLock::new(PersistentCookieJar::new(
            zone_id,
            arc_jar.clone(),
            store,
        )));

        self.jars.write().unwrap().insert(zone_id, persistent.clone());

        Some(persistent)
    }


    /// Persists a snapshot of `zone_id`'s jar to disk.
    ///
    /// Called by [`PersistentCookieJar`] after each mutation. This method reads
    /// the current file, updates/replaces the zone entry, and writes the file back.
    ///
    /// # Panics
    /// Panics on I/O/serialization errors.
    fn persist_zone_from_snapshot(&self, zone_id: ZoneId, snapshot: &DefaultCookieJar) {
        let mut store_file = self.load_file();
        store_file.zones.insert(zone_id, snapshot.clone());
        self.save_file(&store_file);
    }

    /// Removes `zone_id` from both the in-memory cache and the on-disk file.
    ///
    /// # Panics
    /// Panics on I/O/serialization errors while updating the file.
    fn remove_zone(&self, zone_id: ZoneId) {
        self.jars.write().unwrap().remove(&zone_id);

        let mut file = self.load_file();
        file.zones.remove(&zone_id);
        self.save_file(&file);
    }

    /// Persists **all** in-memory jars to disk by snapshotting them.
    ///
    /// Only jars of type [`PersistentCookieJar`] that wrap a [`DefaultCookieJar`]
    /// are snapshotted here. This avoids double-wrapping and keeps the format stable.
    ///
    /// # Panics
    /// Panics on I/O/serialization errors while writing the file.
    fn persist_all(&self) {
        let jars = self.jars.read().unwrap();

        let mut file = self.load_file();
        for (zone_id, jar) in jars.iter() {
            if let Ok(jar) = jar.read() {
                if let Some(persist) = jar.as_any().downcast_ref::<PersistentCookieJar>() {
                    if let Ok(inner) = persist.inner.read() {
                        if let Some(default) = inner.as_any().downcast_ref::<DefaultCookieJar>() {
                            file.zones.insert(*zone_id, default.clone());
                        }
                    }
                }
            }
        }

        self.save_file(&file);
    }
}
