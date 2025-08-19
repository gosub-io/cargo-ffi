use std::collections::HashMap;
use gosub_engine::render::backend::{CompositorSink, ExternalHandle};
use gosub_engine::tab::TabId;

pub struct GtkCompositor {
    /// A map of tab IDs to their corresponding external handles.
    pub frames: HashMap<TabId, ExternalHandle>,
    /// A callback function to be called when a redraw is requested.
    redraw_cb: Box<dyn Fn() + 'static>,
}

impl GtkCompositor {
    pub fn new<F: Fn() + 'static>(redraw_cb: F) -> Self {
        Self {
            frames: HashMap::new(),
            redraw_cb: Box::new(redraw_cb),
        }
    }

    fn request_redraw(&self) {
        (self.redraw_cb)();
    }

    #[allow(unused)]
    pub fn frame_for(&self, tab_id: TabId) -> Option<&ExternalHandle> {
        self.frames.get(&tab_id)
    }

    pub fn frame_for_mut(&mut self, tab_id: TabId) -> Option<&mut ExternalHandle> {
        self.frames.get_mut(&tab_id)
    }
}

impl CompositorSink for GtkCompositor {
    /// Submits a frame for the given tab ID, storing the external handle. This is done by the actual
    /// backend render system.
    fn submit_frame(&mut self, tab_id: TabId, handle: ExternalHandle) {
        self.frames.insert(tab_id, handle);
        self.request_redraw();
    }
}
