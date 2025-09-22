use std::mem;
use pyo3::prelude::*;
use crate::zone::{ZoneConfig, ZoneConfigBuilder};

#[pyclass(name = "ZoneConfig")]
pub struct PyZoneConfig {
    pub inner: ZoneConfig,
}

#[pymethods]
impl PyZoneConfig {
    #[staticmethod]
    pub fn builder() -> PyZoneConfigBuilder {
        PyZoneConfigBuilder {
            inner: crate::zone::ZoneConfig::builder(),
        }
    }

    pub fn __repr__(&self) -> String {
        format!("{:?}", &self.inner)
    }
}

#[pyclass(name = "ZoneConfigBuilder")]
pub struct PyZoneConfigBuilder {
    inner: ZoneConfigBuilder,
}

#[pymethods]
impl PyZoneConfigBuilder {
    pub fn do_not_track(slf: Bound<'_, Self>, on: bool) -> Bound<'_, Self> {
        let mut  this = slf.borrow_mut();
        this.inner = mem::take(&mut this.inner).do_not_track(on);

        slf
    }

    pub fn accept_languages(slf: Bound<'_, Self>, langs: String) -> Bound<'_, Self> {
        let mut  this = slf.borrow_mut();
        this.inner = mem::take(&mut this.inner).accept_languages(langs);

        slf
    }

    pub fn max_tabs(slf: Bound<'_, Self>, n: usize) -> Bound<'_, Self> {
        let mut  this = slf.borrow_mut();
        this.inner = mem::take(&mut this.inner).max_tabs(n);

        slf
    }

    pub fn build(&mut self) -> PyResult<PyZoneConfig> {
        match mem::take(&mut self.inner).build() {
            Ok(cfg) => Ok(PyZoneConfig { inner: cfg }),
            Err(e) => Err(pyo3::exceptions::PyValueError::new_err(format!("{:?}", e))),
        }
    }
}
