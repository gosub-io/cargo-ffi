use std::mem;
use pyo3::prelude::*;
use crate::EngineConfig;

#[pyclass(name = "EngineConfig")]
pub struct PyEngineConfig {
    pub(crate) inner: EngineConfig,
}

#[pymethods]
impl PyEngineConfig {
    #[staticmethod]
    pub fn builder() -> PyEngineConfigBuilder {
        PyEngineConfigBuilder {
            inner: EngineConfig::builder(),
        }
    }

    pub fn __repr__(&self) -> String {
        format!("{:?}", &self.inner)
    }
}

#[pyclass(name = "EngineConfigBuilder")]
pub struct PyEngineConfigBuilder {
    inner: crate::engine::config::EngineConfigBuilder,
}

#[pymethods]
impl PyEngineConfigBuilder {
    pub fn max_zones(slf: Bound<'_, Self>, n: usize) -> Bound<'_, Self> {
        let mut this = slf.borrow_mut();
        this.inner = mem::take(&mut this.inner).max_zones(n);

        slf
    }

    pub fn user_agent(slf: Bound<'_, Self>, ua: String) -> Bound<'_, Self> {
        let mut this = slf.borrow_mut();
        this.inner = mem::take(&mut this.inner).user_agent(ua);

        slf
    }

    pub fn build(&mut self) -> PyResult<PyEngineConfig> {
        match mem::take(&mut self.inner).build() {
            Ok(cfg) => Ok(PyEngineConfig { inner: cfg }),
            Err(e) => Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(format!("{:?}", e))),
        }
    }
}
