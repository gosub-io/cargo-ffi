use pyo3::prelude::*;
use crate::render::backends::null::NullBackend;

#[pyclass(name = "NullBackend")]
pub struct PyNullBackend {
    inner: NullBackend,
}

#[pymethods]
impl PyNullBackend {
    #[new]
    pub fn new() -> PyResult<Self> {
        let inner = NullBackend::new().map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(format!("NullBackend::new failed: {:?}", e)))?;
        Ok(PyNullBackend {
            inner,
        })
    }

    pub fn __repr__(&self) -> String {
        "PyNullBackend()".into()
    }
}
