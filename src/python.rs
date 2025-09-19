mod engine_config;
mod gosub_engine;
mod null_backend;
mod zone_config;
mod zone;
mod tab_defaults;
mod zone_services;
mod tab;

use pyo3::prelude::*;
use pyo3::Bound;
use pyo3::types::PyModule;

#[pymodule]
fn gosub_engine(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyEngineConfig>()?;
    m.add_class::<PyGosubEngine>()?;
    m.add_class::<PyZoneConfig>()?;
    m.add_class::<PyZone>()?;
    m.add_class::<PyNullBackend>()?;
    m.add_class::<PyTabDefaults>()?;
    m.add_class::<PyZoneServices>()?;
    m.add_class::<PyEventReceiver>()?;
    m.add_class::<PyTab>()?;

    Ok(())
}
