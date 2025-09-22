mod engine;
mod gosub_engine;
mod null_backend;
mod zone_config;
mod zone;
mod tab_defaults;
mod zone_services;
mod tab;

use std::mem;
use crate::python::engine::PyEngineConfig;
use crate::python::tab::PyTab;
use pyo3::prelude::*;
use pyo3::Bound;
use pyo3::types::PyModule;
use crate::python::gosub_engine::{PyEventReceiver, PyGosubEngine};
use crate::python::null_backend::PyNullBackend;
use crate::python::tab_defaults::PyTabDefaults;
use crate::python::zone::PyZone;
use crate::python::zone_config::PyZoneConfig;
use crate::python::zone_services::PyZoneServices;

#[pymodule(name = "gosub_engine")]
pub fn gosub_engine_module(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyEngineConfig>()?;
    m.add_class::<PyGosubEngine>()?;
    m.add_class::<PyZoneConfig>()?;
    m.add_class::<PyZone>()?;
    m.add_class::<PyNullBackend>()?;
    m.add_class::<PyTabDefaults>()?;
    m.add_class::<PyZoneServices>()?;
    m.add_class::<PyEventReceiver>()?;
    m.add_class::<PyTab>()?;

    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(format!("failed to create tokio runtime: {:?}", e)))?;

    mem::forget(rt.enter());

    Ok(())
}
