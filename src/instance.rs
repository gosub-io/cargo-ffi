use std::sync::Arc;
use gtk4::cairo;
use gtk4::cairo::ImageSurface;
use tokio::runtime::Runtime;
use tokio::task::JoinHandle;
use crate::net::{fetch, Response};
use crate::tick::{DirtyFlags, TickResult};

pub struct EngineInstance {
    pub dirty: DirtyFlags,              // Is there anything that needs to be redrawn?
    pub current_url: Option<String>,    // Current URL being processed
    pub raw_html: String,

    runtime: Arc<Runtime>,              // Tokio runtime for async operations
    loading_task: Option<JoinHandle<Result<Response, reqwest::Error>>>,

    pub failed: bool,

    pub render_surface: Option<cairo::ImageSurface>,
}

impl EngineInstance {
    pub(crate) fn new(runtime: Arc<Runtime>) -> EngineInstance {
        Self {
            dirty: DirtyFlags::default(),
            current_url: None,
            raw_html: String::new(),

            runtime,
            loading_task: None,

            failed: false,

            render_surface: None,
        }
    }

    pub fn tick(&mut self) -> TickResult {
        let mut result = TickResult::default();

        if let Some(handle) = &mut self.loading_task {
            use futures::FutureExt;

            if let Some(join_result) = handle.now_or_never() {
                self.loading_task = None;

                match join_result {
                    Ok(Ok(response)) => {
                        self.current_url = Some(response.url);
                        self.raw_html = String::from_utf8_lossy(&response.body).to_string();
                        result.needs_redraw = true;
                        result.page_loaded = true;
                        println!("Loaded URL: {}", self.current_url.clone().unwrap_or("".to_string()));
                    }
                    Ok(Err(e)) => {
                        eprintln!("Error loading URL: {}", e);
                        self.raw_html = "<h1>Failed to load page</h1>".to_string();
                        result.needs_redraw = true;
                    }
                    Err(e) => {
                        eprintln!("Task failed: {}", e);
                    }
                }
            }
        }

        if self.dirty.viewport {
            result.needs_redraw = true;
        }

        result
    }

    pub fn start_loading(&mut self, url: String) {
        let handle = self.runtime.spawn(async move {
            fetch(&url).await
        });
        self.loading_task = Some(handle);
        self.failed = false;
    }

    pub fn loading_complete(&self) -> bool {
        self.loading_task.is_none()
    }

    pub fn failed(&self) -> bool {
        self.failed
    }

    pub fn poll_loading(&mut self) -> Option<Result<String, String>> {
        use futures::FutureExt;

        if let Some(handle) = &mut self.loading_task {
            if let Some(join_result) = handle.now_or_never() {
                self.loading_task = None;
                return Some(match join_result {
                    Ok(Ok(resp)) => Ok(String::from_utf8_lossy(&resp.body).into_owned()),
                    Ok(Err(e)) => Err(e.to_string()),
                    Err(e) => Err(format!("Join error: {}", e)),
                });
            }
        }

        None
    }

    pub fn set_raw_html(&mut self, html: String) {
        self.raw_html = html;
    }

    pub fn start_rendering(&mut self) {
        let surface = ImageSurface::create(cairo::Format::ARgb32, 800, 600).unwrap();
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

    pub fn rendering_complete(&self) -> bool {
        self.render_surface.is_some()
    }

    pub fn surface(&self) -> Option<&ImageSurface> {
        self.render_surface.as_ref()
    }

    pub fn render_error(&mut self, msg: &str) {
        let surface = ImageSurface::create(cairo::Format::ARgb32, 800, 600).unwrap();
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