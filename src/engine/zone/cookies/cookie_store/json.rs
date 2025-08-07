use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use serde::{Deserialize, Serialize};
use crate::zone::cookies::CookieJar;
use crate::zone::cookies::cookie_jar::DefaultCookieJar;
use crate::zone::cookies::cookie_store::CookieStore;
use crate::zone::cookies::persistent_cookie_jar::PersistentCookieJar;
use crate::zone::zone::ZoneId;

/// Serializable structure for all zones' cookie jars
#[derive(Debug, Serialize, Deserialize)]
struct CookieStoreFile {
    zones: HashMap<ZoneId, DefaultCookieJar>,
}

pub struct JsonCookieStore {
    path: PathBuf,
    jars: RwLock<HashMap<ZoneId, Arc<RwLock<dyn CookieJar + Send + Sync>>>>,
    store_self: RwLock<Option<Arc<dyn CookieStore + Send + Sync>>>,
}

impl JsonCookieStore {
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
            *self_ref = Some(store.clone() as Arc<dyn CookieStore + Send + Sync>);
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
    fn get_jar(&self, zone_id: ZoneId) -> Option<Arc<RwLock<dyn CookieJar + Send + Sync>>> {
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
        let arc_jar: Arc<RwLock<dyn CookieJar + Send + Sync>> = Arc::new(RwLock::new(jar));

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

    fn persist_zone(&self, zone_id: ZoneId) {
        let jars = self.jars.read().unwrap();
        let Some(jar) = jars.get(&zone_id) else { return };

        let jar = jar.read().unwrap();

        let Some(inner) = jar.as_any().downcast_ref::<PersistentCookieJar>() else { return };
        let jar_data = inner.inner.read().unwrap();

        let Some(default) = jar_data.as_any().downcast_ref::<DefaultCookieJar>() else { return };

        let mut store_file = self.load_file();
        store_file.zones.insert(zone_id, default.clone());
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
