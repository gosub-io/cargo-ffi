use pyo3::prelude::*;
use std::sync::Arc;
use tokio::runtime::Runtime;
use tokio::sync::Mutex as AsyncMutex;

use crate::zone::Zone;

#[pyclass(name = "Zone")]
pub struct PyZone {
    pub inner: Arc<AsyncMutex<Zone>>,
    pub runtime: Arc<Runtime>,
}

#[pymethods]
impl PyZone {
    pub fn set_title(&self, title: &str) {
        let rt = self.runtime.clone();
        let inner = self.inner.clone();
        rt.block_on(async move {
            let mut z = inner.lock().await;
            z.set_title(title.to_string());
        });
    }

    pub fn set_description(&self, desc: &str) {
        let rt = self.runtime.clone();
        let inner = self.inner.clone();
        rt.block_on(async move {
            let mut z = inner.lock().await;
            z.set_description(desc.to_string());
        });
    }

    pub fn set_color(&self, r: u8, g: u8, b: u8, a: u8) {
        let rt = self.runtime.clone();
        let inner = self.inner.clone();
        rt.block_on(async move {
            let mut z = inner.lock().await;
            z.set_color([r, g, b, a]);
        });
    }

    pub fn create_tab(&self, py_tab_defaults: &crate::python::tab_defaults::PyTabDefaults) -> PyResult<crate::python::tab::PyTab> {
        let rt = self.runtime.clone();
        let inner = self.inner.clone();
        
        let res = rt.block_on(async move {
            let mut z = inner.lock().await;
            z.create_tab(py_tab_defaults.inner.clone(), None).await
        });

        match res {
            Ok(handle) => Ok(crate::python::tab::PyTab {
                inner: Arc::new(tokio::sync::Mutex::new(handle)),
                runtime: rt,
            }),
            Err(e) => Err(pyo3::exceptions::PyRuntimeError::new_err(format!("create_tab failed: {:?}", e))),
        }
    }
}
