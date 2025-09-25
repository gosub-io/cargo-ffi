use std::sync::{Arc, RwLock};
use crate::render::backend::CompositorSink;

/// A router that forwards compositor events to a dynamically set sink.
#[derive(Default, Clone)]
pub struct CompositorRouter {
    inner: Arc<RwLock<Option<Arc<dyn CompositorSink + Send + Sync>>>>,
}

impl CompositorRouter {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            inner: Arc::new(RwLock::new(None)),
        })
    }

    pub fn set_sink(&self, sink: Arc<dyn CompositorSink + Send + Sync>) {
        let mut inner = self.inner.write().unwrap();
        *inner = Some(sink);
    }

    pub fn clear_sink(&self) {
        let mut inner = self.inner.write().unwrap();
        *inner = None;
    }

    #[inline]
    fn with_sink<F>(&self, f: F)
    where
        F: FnOnce(&dyn CompositorSink),
    {
        if let Some(sink) = self.inner.read().unwrap().as_ref() {
            f(sink.as_ref());
        }
    }
}

impl CompositorSink for CompositorRouter {
    fn submit_frame(&self, tab_id: crate::tab::TabId, handle: crate::render::backend::ExternalHandle) {
        self.with_sink(|sink| sink.submit_frame(tab_id, handle));
    }
}