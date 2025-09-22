use pyo3::prelude::*;
use std::sync::{Arc, mpsc, Mutex as StdMutex, Mutex};
use tokio::sync::Mutex as AsyncMutex;
use tokio::runtime::Runtime;

use crate::{GosubEngine, EngineConfig, EngineError};
use crate::python::engine::PyEngineConfig;
use crate::render::backends::null::NullBackend;
use crate::zone::Zone;

#[pyclass(name = "EventReceiver")]
pub struct PyEventReceiver {
    inner: Arc<Mutex<mpsc::Receiver<String>>>,
}

#[pymethods]
impl PyEventReceiver {
    pub fn recv(&self) -> Option<String> {
        let rx = self.inner.lock().unwrap();
        rx.recv().ok()
    }
}

#[pyclass(name = "GosubEngine")]
pub struct PyGosubEngine {
    inner: Arc<StdMutex<GosubEngine>>,
    runtime: Arc<Runtime>,
}


trait BackendProvider {

}

#[pymethods]
impl PyGosubEngine {
    #[new]
    pub fn new(cfg: Option<&PyEngineConfig>) -> PyResult<Self> {
        let cfg_inner = cfg.map(|c| c.inner.clone());
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(format!("failed to build runtime: {}", e)))?;
        let backend = NullBackend::new().map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(format!("failed to create null backend: {:?}", e)))?;
        let engine = GosubEngine::new(cfg_inner, Box::new(backend));
        Ok(PyGosubEngine {
            inner: Arc::new(Mutex::new(engine)),
            runtime: Arc::new(runtime),
        })
    }

    pub fn start(&self) -> PyResult<()> {
        let rt = self.runtime.clone();
        let mut engine = self.inner.lock().unwrap();
        let res = rt.block_on(async {
            engine.start()
        });
        res.map(|_| ()).map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(format!("start failed: {:?}", e)))
    }

    pub fn shutdown(&self) -> PyResult<()> {
        let rt = self.runtime.clone();
        let mut engine = self.inner.lock().unwrap();
        rt.block_on(async {
            engine.shutdown().await
        }).map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(format!("shutdown failed: {:?}", e)))?;
        Ok(())
    }

    pub fn subscribe_events(&self) -> PyResult<PyEventReceiver> {
        let (tx, rx) = mpsc::channel::<String>();
        
        let mut eng = self.inner.lock().unwrap();
        let mut sub = eng.subscribe_events();
        let rt = self.runtime.clone();
        
        rt.spawn(async move {
            loop {
                match sub.recv().await {
                    Ok(ev) => {
                        let _ = tx.send(format!("{:?}", ev));
                    }
                    Err(_err) => {
                        break;
                    }
                }
            }
        });

        Ok(PyEventReceiver {
            inner: Arc::new(Mutex::new(rx)),
        })
    }

    pub fn create_zone(&self, py_zone_cfg: &crate::python::zone_config::PyZoneConfig, py_services: &crate::python::zone_services::PyZoneServices) -> PyResult<crate::python::zone::PyZone> {
        let rt = self.runtime.clone();
        let mut eng = self.inner.lock().unwrap();

        let zone = eng.create_zone(py_zone_cfg.inner.clone(), py_services.inner.clone(), None)
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(format!("create_zone failed: {:?}", e)))?;

        Ok(crate::python::zone::PyZone {
            inner: Arc::new(AsyncMutex::new(zone)),
            runtime: rt,
        })
    }
}
