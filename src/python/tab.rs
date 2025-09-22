use pyo3::prelude::*;
use std::sync::Arc;
use tokio::runtime::Runtime;
use tokio::sync::Mutex as AsyncMutex;

use crate::tab::TabHandle;
use crate::render::Viewport;
use crate::events::MouseButton;

#[pyclass(name = "Tab")]
pub struct PyTab {
    pub inner: Arc<AsyncMutex<TabHandle>>,
    pub runtime: Arc<Runtime>,
}

#[pymethods]
impl PyTab {
    pub fn set_viewport(&self, x: i32, y: i32, width: u32, height: u32) -> PyResult<()> {
        let rt = self.runtime.clone();
        let inner = self.inner.clone();
        rt.block_on(async move {
            let handle = inner.lock().await;
            handle.set_viewport(Viewport::new(x, y, width, height)).await
        })
        .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(format!("{:?}", e)))
    }

    pub fn navigate(&self, url: &str) -> PyResult<()> {
        let rt = self.runtime.clone();
        let inner = self.inner.clone();
        rt.block_on(async move {
            let handle = inner.lock().await;
            handle.navigate(url).await
        })
        .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(format!("{:?}", e)))
    }

    pub fn mouse_move(&self, x: f32, y: f32) -> PyResult<()> {
        let rt = self.runtime.clone();
        let inner = self.inner.clone();
        rt.block_on(async move {
            let h = inner.lock().await;
            h.send(crate::events::TabCommand::MouseMove { x, y }).await
        })
        .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(format!("{:?}", e)))
    }

    pub fn mouse_down(&self, x: f32, y: f32, button: &str) -> PyResult<()> {
        let btn = parse_button(button).ok_or_else(|| pyo3::exceptions::PyValueError::new_err("unknown mouse button"))?;
        let rt = self.runtime.clone();
        let inner = self.inner.clone();
        rt.block_on(async move {
            let h = inner.lock().await;
            h.send(crate::events::TabCommand::MouseDown { x, y, button: btn }).await
        })
        .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(format!("{:?}", e)))
    }

    pub fn mouse_up(&self, x: f32, y: f32, button: &str) -> PyResult<()> {
        let btn = parse_button(button).ok_or_else(|| pyo3::exceptions::PyValueError::new_err("unknown mouse button"))?;
        let rt = self.runtime.clone();
        let inner = self.inner.clone();
        rt.block_on(async move {
            let h = inner.lock().await;
            h.send(crate::events::TabCommand::MouseUp { x, y, button: btn }).await
        })
        .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(format!("{:?}", e)))
    }

    pub fn __repr__(&self) -> String {
        format!("PyTab {{ ... }}")
    }
}


fn parse_button(btn: &str) -> Option<MouseButton> {
    match btn.to_ascii_lowercase().as_str() {
        "left" => Some(MouseButton::Left),
        "middle" | "middlebutton" => Some(MouseButton::Middle),
        "right" => Some(MouseButton::Right),
        _ => None,
    }
}

