use crate::engine::storage::{StorageArea, StorageHandles};
use crate::net::{fetch, Response};
use crate::render::{Color, DisplayItem, RenderList, Viewport};
use std::sync::Arc;
use tokio::runtime::Runtime;
use tokio::task::JoinHandle;
use url::Url;

/// BrowsingContext dedicated to a specific tab
///
/// A BrowsingContext is a single instance of the engine that deals with a specific tab. Each tab
/// has one BrowsingContext. These contexts though can use shared processes or threads, but not
/// from other contexts, only from the main engine.
pub struct BrowsingContext {
    // /// Is there anything that needs to be rendered or redrawn?
    // dirty: DirtyFlags,
    /// Current URL being processed
    current_url: Option<Url>,
    /// This should become the DOM document, but maybe we can leave the raw HTML here as well
    raw_html: String,
    /// True when the tab has failed loading (mostly net issues)
    failed: bool,

    /// Tokio runtime for async operations
    runtime: Arc<Runtime>,
    /// Handle for loading the task (async)
    loading_task: Option<JoinHandle<Result<Response, reqwest::Error>>>,

    /// Storage handles for local and session storage
    storage: Option<StorageHandles>,

    // Rendering commands to paint the tab onto a surface
    render_list: RenderList,
    /// Render dirty flag, used to determine if the tab needs to be rendered
    render_dirty: bool,
    /// Viewport for the tab, used to determine what part of the page to render
    viewport: Viewport,
    /// Epoch of the scene, used to determine if the scene has changed
    scene_epoch: u64,

    /// DOM dirty flag, used to determine if the DOM has changed
    dom_dirty: bool,
    /// Style dirty flag, used to determine if the styles have changed
    style_dirty: bool,
    /// Layout dirty flag, used to determine if the layout has changed
    layout_dirty: bool,
}

impl BrowsingContext {
    /// Creates a new runtime browsing context.
    pub(crate) fn new(runtime: Arc<Runtime>) -> BrowsingContext {
        Self {
            // dirty: DirtyFlags::default(),
            current_url: None,
            raw_html: String::new(),
            runtime,
            loading_task: None,
            failed: false,
            storage: None, // Default no storage unless binding manually by a tab
            render_list: RenderList::new(),
            render_dirty: false,
            viewport: Viewport::default(),
            scene_epoch: 0,
            dom_dirty: false,
            style_dirty: false,
            layout_dirty: false,
        }
    }

    /// Binds the storage handles to the browsing context (@TODO: Why not via the ::new()?).
    pub fn bind_storage(&mut self, local: Arc<dyn StorageArea>, session: Arc<dyn StorageArea>) {
        self.storage = Some(StorageHandles {
            local: local.clone(),
            session: session.clone(),
        });
        // At this point, we would probably want to hook our storage handles into the javascript/lua runtime
    }
    pub fn local_storage(&self) -> Option<Arc<dyn StorageArea>> {
        self.storage.as_ref().map(|s| s.local.clone())
    }
    pub fn session_storage(&self) -> Option<Arc<dyn StorageArea>> {
        self.storage.as_ref().map(|s| s.session.clone())
    }

    /// Starts a task that will load the actual url
    pub fn start_loading(&mut self, url: Url) {
        let url_clone = url.clone();
        let handle = self.runtime.spawn(async move { fetch(url_clone).await });

        self.loading_task = Some(handle);
        self.failed = false;
        self.current_url = Some(url);
    }

    /// Polls the loading to see if it is still running or not.
    pub fn poll_loading(&mut self) -> Option<Result<Response, String>> {
        use futures::FutureExt;

        if let Some(handle) = &mut self.loading_task {
            if let Some(join_result) = handle.now_or_never() {
                self.loading_task = None;
                return Some(match join_result {
                    Ok(Ok(resp)) => Ok(resp),
                    Ok(Err(e)) => Err(e.to_string()),
                    Err(e) => Err(format!("Join error: {}", e)),
                });
            }
        }

        None
    }

    /// Sets the rab HTML for the given tab
    pub fn set_raw_html(&mut self, html: &str) {
        self.raw_html = html.to_string();
        self.dom_dirty = true; // Mark the DOM as dirty, so it will be rendered
        self.style_dirty = true;
        self.layout_dirty = true;
        self.invalidate_render();
    }

    pub fn set_viewport(&mut self, vp: Viewport) {
        if self.viewport != vp {
            self.viewport = vp;
            self.layout_dirty = true;
            self.invalidate_render();
        }
    }

    #[inline]
    pub fn viewport(&self) -> &Viewport {
        &self.viewport
    }

    #[inline]
    pub fn scene_epoch(&self) -> u64 {
        self.scene_epoch
    }

    pub fn invalidate_render(&mut self) {
        self.render_dirty = true;
    }

    /// Build/refresh the device-agnostic scene if needed.
    /// For now, this renders raw_html as text lines; later, it consumes DOM/layout.
    pub fn rebuild_render_list_if_needed(&mut self) {
        if !self.render_dirty {
            return;
        }

        let mut rl = RenderList::default();

        // Example scene: clear + show raw HTML as text
        rl.items.push(DisplayItem::Clear {
            color: Color::new(0.75, 0.75, 0.75, 1.0),
        });

        // Text color: black
        let c = Color::new(0.0, 0.0, 0.0, 1.0);
        let mut y = 24.0;
        for line in self.raw_html.lines() {
            rl.items.push(DisplayItem::TextRun {
                x: 14.0,
                y,
                text: line.to_string(),
                size: 23.0,
                color: c,
                max_width: Some(self.viewport.width as f32),
            });
            y += 16.0;
        }

        self.render_list = rl;
        self.render_dirty = false;
        self.scene_epoch = self.scene_epoch.wrapping_add(1);

        self.dom_dirty = false;
        self.style_dirty = false;
        self.layout_dirty = false;
    }

    #[inline]
    pub fn render_list(&self) -> &RenderList {
        &self.render_list
    }

    /// Returns true when the loading failed
    pub fn has_failed(&self) -> bool {
        self.failed
    }

    /// Returns the raw HTML of the tab
    pub fn current_url(&self) -> Option<&Url> {
        self.current_url.as_ref()
    }
}
