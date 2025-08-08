
use std::sync::Arc;
use gtk4::cairo;
use gtk4::cairo::ImageSurface;
use tokio::runtime::Runtime;
use tokio::task::JoinHandle;
use crate::net::{fetch, Response};
use crate::engine::tick::DirtyFlags;
use crate::viewport::Viewport;

/// Engine instance dedicated to a specific tab
///
/// An engine instance is a single instance of the engine that deals with a specific tab. Each tab
/// has one engine instance. These instances though, can use shared processes or threads, but not
/// from other instances, but from the main engine.
pub struct EngineInstance {
    /// Is there anything that needs to be rendered or redrawn?
    pub dirty: DirtyFlags,
    /// Current URL being processed
    pub current_url: Option<String>,
    /// This should become the DOM document, but maybe we can leave the raw HTML here as well
    pub raw_html: String,
    /// True when the tab has failed loading (mostly net issues)
    pub failed: bool,

    /// Tokio runtime for async operations
    runtime: Arc<Runtime>,

    /// Handle for loading the task (async)
    loading_task: Option<JoinHandle<Result<Response, reqwest::Error>>>,

    /// Render surface onto which the tab will render things
    pub render_surface: Option<cairo::ImageSurface>,
}

impl EngineInstance {
    /// Creates a new runtime instance. Note that we pass the runtime to the engine instance so all instances
    /// uses the same runtime.
    pub(crate) fn new(runtime: Arc<Runtime>) -> EngineInstance {
        Self {
            dirty: DirtyFlags::default(),
            current_url: None,
            raw_html: String::new(),

            // Here we probably have things like the render pipeline data,
            // dom tree, and stuff like that.

            runtime,
            loading_task: None,
            failed: false,

            render_surface: None,
        }
    }

    /// Starts a task that will load the actual url
    pub fn start_loading(&mut self, url: String) {
        let handle = self.runtime.spawn(async move {
            fetch(&url).await
        });
        self.loading_task = Some(handle);
        self.failed = false;
    }

    /// Returns true when the loading of the url has been completed
    pub fn loading_complete(&self) -> bool {
        self.loading_task.is_none()
    }

    /// Returns true when the loading failed
    pub fn failed(&self) -> bool {
        self.failed
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
    pub fn set_raw_html(&mut self, html: String) {
        self.raw_html = html;
    }

    /// Start the process of rendering. This will be changed later and will trigger the render pipeline. Not sure yet how
    pub fn start_rendering(&mut self, viewport: Viewport) {
        let surface = ImageSurface::create(cairo::Format::ARgb32, viewport.width as i32, viewport.height as i32).unwrap();
        let cr = cairo::Context::new(&surface).unwrap();
        cr.set_source_rgb(0.0, 0.0, 0.1);
        cr.paint().unwrap();
        cr.set_source_rgb(1.0, 1.0, 1.0);
        cr.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Normal);
        cr.set_font_size(14.0);

        // We cut the string to strings of 120 chars, and print those each on a different line
        let lines: Vec<&str> = self.raw_html.lines().collect();
        for (i, line) in lines.iter().enumerate() {
            cr.move_to(20.0, 40.0 + (i as f64 * 20.0));
            cr.show_text(line).ok();
        }
        self.render_surface = Some(surface);
    }

    /// Returns true when we hav a rendered surface
    pub fn rendering_complete(&self) -> bool {
        self.render_surface.is_some()
    }

    /// Returns the actual surface
    pub fn surface(&self) -> Option<&ImageSurface> {
        self.render_surface.as_ref()
    }

    /// Renders an error onto the surface
    pub fn render_error(&mut self, msg: &str, viewport: Viewport) {
        let surface = ImageSurface::create(cairo::Format::ARgb32, viewport.width as i32, viewport.height as i32).unwrap();
        let cr = cairo::Context::new(&surface).unwrap();
        cr.set_source_rgb(0.2, 0.0, 0.0);
        cr.paint().unwrap();
        cr.set_source_rgb(1.0, 1.0, 1.0);
        cr.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Bold);
        cr.set_font_size(18.0);
        cr.move_to(20.0, 40.0);
        cr.show_text("Load error:").ok();
        cr.move_to(20.0, 70.0);
        cr.show_text(msg).ok();
        self.render_surface = Some(surface);
    }
}