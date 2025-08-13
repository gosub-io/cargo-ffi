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

/// Serializable structure for all zones' cookie jars
#[derive(Debug, Serialize, Deserialize)]
struct CookieStoreFile {
    zones: HashMap<ZoneId, DefaultCookieJar>,
}

/// A JSON-based cookie store that persists cookies across sessions.
pub struct JsonCookieStore {
    /// Path to the JSON file where cookies are stored.
    path: PathBuf,
    /// Actual list of cookie jars per zone
    jars: RwLock<HashMap<ZoneId, CookieJarHandle>>,
    /// Link to the actual cookie store to send to the persistent cookie jars
    store_self: RwLock<Option<CookieStoreHandle>>,
}

impl JsonCookieStore {
    #[allow(unused)]
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

    fn load_file(&self) -> CookieStoreFile {
        let mut file = File::open(&self.path).expect("Failed to open cookie store file");
        let mut contents = String::new();
        file.read_to_string(&mut contents).expect("Failed to read cookie store file");

        serde_json::from_str(&contents).unwrap_or_else(|_| CookieStoreFile { zones: HashMap::new() })
    }

    fn save_file(&self, store_file: &CookieStoreFile) {
        let contents = serde_json::to_string_pretty(store_file).expect("Failed to serialize cookies");
        let mut file = File::create(&self.path).expect("Failed to open cookie store file for writing");
        file.write_all(contents.as_bytes()).expect("Failed to write cookie store file");
    }
}

impl CookieStore for JsonCookieStore {
    fn get_jar(&self, zone_id: ZoneId) -> Option<CookieJarHandle> {
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

    fn persist_zone_from_snapshot(&self, zone_id: ZoneId, snapshot: &DefaultCookieJar) {
        let mut store_file = self.load_file();
        store_file.zones.insert(zone_id, snapshot.clone());
        self.save_file(&store_file);
    }

    fn remove_zone(&self, zone_id: ZoneId) {
        self.jars.write().unwrap().remove(&zone_id);

        let mut file = self.load_file();
        file.zones.remove(&zone_id);
        self.save_file(&file);
    }

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
