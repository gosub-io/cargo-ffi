use gosub_engine::render::backend::{CompositorSink, ExternalHandle};
use gosub_engine::tab::TabId;
use std::collections::HashMap;

pub struct VelloCompositor {
    pub frames: HashMap<TabId, ExternalHandle>,
}

impl VelloCompositor {
    pub fn new() -> Self {
        Self {
            frames: HashMap::new(),
        }
    }

    #[allow(unused)]
    pub fn frame_for(&self, tab_id: TabId) -> Option<&ExternalHandle> {
        self.frames.get(&tab_id)
    }

    pub fn frame_for_mut(&mut self, tab_id: TabId) -> Option<&mut ExternalHandle> {
        self.frames.get_mut(&tab_id)
    }
}

impl CompositorSink for VelloCompositor {
    fn submit_frame(&mut self, tab_id: TabId, handle: ExternalHandle) {
        self.frames.insert(tab_id, handle);
    }
}
