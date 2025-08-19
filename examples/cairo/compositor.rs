use std::collections::HashMap;
use gosub_engine::render::backend::{CompositorSink, ExternalHandle};
use gosub_engine::tab::TabId;

/// A compositor implementation for GTK that manages frames per tab
/// and requests redraws when new frames are submitted.
///
/// The compositor acts as the sink for rendered frames: the backend
/// rendering system calls [`CompositorSink::submit_frame`] to provide
/// a new frame for a specific tab. The compositor stores the frame
/// in its `frames` map and triggers a redraw callback so the host
/// UI (GTK) can repaint.
pub struct GtkCompositor {
    /// A map of tab IDs to their corresponding external handles.
    /// Each [`TabId`] maps to an [`ExternalHandle`] provided by the
    /// render backend.
    pub frames: HashMap<TabId, ExternalHandle>,

    /// A callback function invoked when a redraw is requested.
    /// Typically this is connected to a GTK widget’s `queue_draw()`
    /// or similar function.
    redraw_cb: Box<dyn Fn() + 'static>,
}

impl GtkCompositor {
    /// Creates a new `GtkCompositor` with the given redraw callback.
    ///
    /// # Arguments
    ///
    /// * `redraw_cb` - A closure that will be called whenever a new
    ///   frame is submitted and the UI should repaint.
    pub fn new<F: Fn() + 'static>(redraw_cb: F) -> Self {
        Self {
            frames: HashMap::new(),
            redraw_cb: Box::new(redraw_cb),
        }
    }

    /// Requests a redraw by invoking the stored callback.
    /// This is typically triggered when new frames arrive.
    fn request_redraw(&self) {
        println!("GtkCompositor: Requesting redraw");
        (self.redraw_cb)();
    }

    /// Retrieves an immutable reference to the [`ExternalHandle`]
    /// for the given [`TabId`], if it exists.
    ///
    /// # Arguments
    ///
    /// * `tab_id` - The ID of the tab whose frame should be retrieved.
    ///
    /// # Returns
    ///
    /// `Some(&ExternalHandle)` if a frame is stored for this tab,
    /// or `None` if no frame has been submitted yet.
    #[allow(unused)]
    pub fn frame_for(&self, tab_id: TabId) -> Option<&ExternalHandle> {
        self.frames.get(&tab_id)
    }

    /// Retrieves a mutable reference to the [`ExternalHandle`]
    /// for the given [`TabId`], if it exists.
    ///
    /// This can be used to update or replace the handle in place.
    pub fn frame_for_mut(&mut self, tab_id: TabId) -> Option<&mut ExternalHandle> {
        self.frames.get_mut(&tab_id)
    }
}

impl CompositorSink for GtkCompositor {
    /// Submits a new frame for the given [`TabId`].
    ///
    /// Called by the render backend when it has produced a new frame.
    /// The compositor stores the [`ExternalHandle`] in its frame map
    /// and requests a redraw via the callback.
    ///
    /// # Arguments
    ///
    /// * `tab_id` - The tab for which the frame is produced.
    /// * `handle` - The external handle containing the frame data.
    fn submit_frame(&mut self, tab_id: TabId, handle: ExternalHandle) {
        println!("GtkCompositor: Submitting frame for tab {:?}", tab_id);
        self.frames.insert(tab_id, handle);
        self.request_redraw();
    }
}
