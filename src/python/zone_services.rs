use pyo3::prelude::*;
use std::sync::Arc;

use crate::zone::ZoneServices;
use crate::storage::{StorageService, InMemoryLocalStore, InMemorySessionStore};
use crate::storage::types::PartitionPolicy;
use crate::cookies::DefaultCookieJar;
use crate::engine::cookies::CookieJarHandle;

#[pyclass(name = "ZoneServices")]
pub struct PyZoneServices {
    pub inner: ZoneServices,
}

#[pymethods]
impl PyZoneServices {
    #[new]
    pub fn new() -> Self {
        PyZoneServices {
            inner: ZoneServices {
                storage: Arc::new(StorageService::new(
                    Arc::new(InMemoryLocalStore::new()),
                    Arc::new(InMemorySessionStore::new()),
                )),
                cookie_store: None,
                cookie_jar: None,
                partition_policy: PartitionPolicy::TopLevelOrigin,
            },
        }
    }

    #[staticmethod]
    pub fn in_memory() -> Self {
        let storage = Arc::new(StorageService::new(
            Arc::new(InMemoryLocalStore::new()),
            Arc::new(InMemorySessionStore::new()),
        ));

        let jar: CookieJarHandle = DefaultCookieJar::new().into();

        PyZoneServices {
            inner: ZoneServices {
                storage,
                cookie_store: None,
                cookie_jar: Some(jar),
                partition_policy: PartitionPolicy::TopLevelOrigin,
            },
        }
    }

    pub fn __repr__(&self) -> String {
        format!("PyZoneServices {{ storage: ... , cookie_jar: {} }}", self.inner.cookie_jar.is_some())
    }
}
