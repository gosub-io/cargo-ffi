use pyo3::prelude::*;
use crate::tab::TabDefaults;
use crate::render::Viewport;

#[pyclass(name = "TabDefaults")]
pub struct PyTabDefaults {
    pub inner: TabDefaults,
}

#[pymethods]
impl PyTabDefaults {
    #[new]
    #[allow(clippy::too_many_arguments)]
    pub fn new(url: Option<&str>, title: Option<&str>, viewport: Option<(i32, i32, u32, u32)>) -> Self {
        let vp = viewport.map(|(x, y, w, h)| Viewport::new(x, y, w, h));
        PyTabDefaults {
            inner: TabDefaults {
                url: url.map(|s| s.to_string()),
                title: title.map(|s| s.to_string()),
                viewport: vp,
            },
        }
    }

    pub fn __repr__(&self) -> String {
        format!("{:?}", &self.inner)
    }
}
