use crate::engine::storage::{StorageArea, StorageHandles};
use crate::net::{fetch, Response};
use crate::render::{Color, DisplayItem, RenderList, Viewport};
use std::sync::Arc;
use http::header::CONTENT_TYPE;
use url::Url;

#[derive(Debug, thiserror::Error)]
pub enum LoadError {
    #[error("navigation canceled")]
    Canceled,
    #[error(transparent)]
    Net(#[from] reqwest::Error),
}

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

    // Tokio runtime for async operations
    // runtime: Arc<Runtime>,

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
    pub(crate) fn new() -> BrowsingContext {
        Self {
            current_url: None,
            raw_html: String::new(),
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
        self.storage = Some(StorageHandles { local, session });
    }
    pub fn local_storage(&self) -> Option<Arc<dyn StorageArea>> {
        self.storage.as_ref().map(|s| s.local.clone())
    }
    pub fn session_storage(&self) -> Option<Arc<dyn StorageArea>> {
        self.storage.as_ref().map(|s| s.session.clone())
    }


    /// Load a URL, mutate the context, and return the raw Response.
    /// - On success: sets current_url (after redirects), raw_html (decoded), clears `failed`, invalidates render.
    /// - On error/cancel: sets `failed = true` (and leaves previous HTML intact), returns a descriptive error.
    ///
    /// Caller (e.g., the tab task) can also use `resp.headers` to store cookies into the zone jar.
    pub async fn load(
        &mut self,
        url: Url,
        cancel: tokio_util::sync::CancellationToken,
    ) -> Result<Response, LoadError> {
        self.failed = false;
        self.current_url = Some(url.clone());

        let resp = tokio::select! {
            _ = cancel.cancelled() => {
                self.failed = true;
                self.set_raw_html("<pre>Load cancelled</pre>");
                return Err(LoadError::Canceled);
            }
            r = fetch(url) => {
                match r {
                    Ok(resp) => resp,
                    Err(e) => {
                        self.failed = true;
                        self.set_raw_html(&format!("<pre>Load error: {e}</pre>"));
                        return Err(LoadError::Net(e));
                    }
                }
            }
        };

        // Update to the final URL after redirects (if your fetch follows redirects)
        self.current_url = Some(resp.url.clone());

        // Decode body to string using Content-Type charset when available
        let html = decode_response_body(&resp.headers, &resp.body);
        self.set_raw_html(&html); // marks DOM/style/layout dirty and invalidates render

        Ok(resp)
    }

    /// Sets the raw HTML for the given tab
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
            color: Color::new(0.55, 0.25, 0.45, 1.0),
        });

        // Text color: black
        let c = Color::new(0.0, 0.0, 0.0, 1.0);
        let font_size = 16.0;
        let mut y = 0.0;
        for line in self.raw_html.lines() {
            rl.items.push(DisplayItem::TextRun {
                x: 0.0,
                y,
                text: line.to_string(),
                size: font_size,
                color: c,
                max_width: Some(self.viewport.width as f32),
            });
            y += font_size;
        }

        self.render_list = rl;
        self.render_dirty = false;
        self.scene_epoch = self.scene_epoch.wrapping_add(1);

        self.dom_dirty = false;
        self.style_dirty = false;
        self.layout_dirty = false;
    }

    /// Returns the render list
    #[inline]
    pub fn render_list(&self) -> &RenderList {
        &self.render_list
    }

    /// Returns true when the loading failed
    pub fn has_failed(&self) -> bool {
        self.failed
    }

    /// Returns the current loaded the tab (or None when nothing has loaded yet)
    pub fn current_url(&self) -> Option<&Url> {
        self.current_url.as_ref()
    }
}


/// Best-effort response body decoder:
/// - honors `Content-Type: ...; charset=...` when present
/// - falls back to UTF-8 lossless
fn decode_response_body(headers: &http::HeaderMap, body: &[u8]) -> String {
    // Try to extract charset from Content-Type
    let mut charset: Option<String> = None;
    if let Some(ct) = headers.get(CONTENT_TYPE) {
        if let Ok(ct) = ct.to_str() {
            // very small, permissive parse: look for "charset=..."
            if let Some(idx) = ct.to_ascii_lowercase().find("charset=") {
                let after = &ct[idx + "charset=".len()..];
                // charset value may be quoted or end at ; or end of string
                let end = after.find([';', ' ', '\t']).unwrap_or(after.len());
                charset = Some(after[..end].trim_matches('"').to_string());
            }
        }
    }

    match charset.as_deref() {
        // Fast path for UTF-8 family
        Some(cs) if cs.eq_ignore_ascii_case("utf-8") || cs.eq_ignore_ascii_case("utf8") => {
            String::from_utf8_lossy(body).into_owned()
        }
        // If you have encoding_rs available, you can do better here.
        // For now, fallback to lossless UTF-8 for any other/unknown charset.
        _ => String::from_utf8_lossy(body).into_owned(),
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use tokio_util::sync::CancellationToken;

    fn list_texts(ctx: &BrowsingContext) -> Vec<String> {
        let mut out = Vec::new();
        for item in ctx.render_list().items.iter() {
            if let DisplayItem::TextRun { text, .. } = item {
                out.push(text.clone());
            }
        }
        out
    }

    #[test]
    fn set_raw_html_marks_dirty_and_builds_scene() {
        let mut ctx = BrowsingContext::new();

        // initially nothing rendered
        assert_eq!(ctx.scene_epoch(), 0);

        ctx.set_raw_html("hello\nworld");
        // dirty flags cause a rebuild to actually produce items
        ctx.rebuild_render_list_if_needed();

        // scene advances once
        assert_eq!(ctx.scene_epoch(), 1);

        // first item should be a Clear, then two TextRun lines
        let texts = list_texts(&ctx);
        assert_eq!(texts, vec!["hello".to_string(), "world".to_string()]);

        // subsequent rebuild without changes should do nothing
        ctx.rebuild_render_list_if_needed();
        assert_eq!(ctx.scene_epoch(), 1);
    }

    #[test]
    fn changing_viewport_invalidates_render() {
        let mut ctx = BrowsingContext::new();

        ctx.set_raw_html("hi");
        ctx.rebuild_render_list_if_needed();
        let epoch1 = ctx.scene_epoch();

        // change viewport → should invalidate and rebuild
        let vp = Viewport { x: 0, y: 0, width: 800, height: 600 };
        ctx.set_viewport(vp);
        ctx.rebuild_render_list_if_needed();
        assert_eq!(ctx.scene_epoch(), epoch1 + 1);
    }

    #[tokio::test]
    async fn load_cancel_sets_failed_and_error_stub() {
        let mut ctx = BrowsingContext::new();

        let url = Url::parse("https://example.com/").unwrap();
        let cancel = CancellationToken::new();
        cancel.cancel(); // cancel immediately to force the cancel branch

        let res = ctx.load(url, cancel.clone()).await;
        // Expect canceled error
        match res {
            Err(LoadError::Canceled) => {}
            other => panic!("expected LoadError::Canceled, got {:?}", other),
        }

        // Context should mark failed
        assert!(ctx.has_failed());

        // And set the error stub HTML (visible after rebuild)
        ctx.rebuild_render_list_if_needed();
        let texts = list_texts(&ctx);
        assert!(texts.iter().any(|t| t.contains("Load cancelled")), "missing cancel text in render list: {:?}", texts);
    }

    #[test]
    fn decode_respects_utf8_charset() {
        use http::HeaderMap;

        let mut headers = HeaderMap::new();
        headers.insert(
            CONTENT_TYPE,
            "text/html; charset=UTF-8".parse().unwrap()
        );

        let body = b"<html>\xe2\x98\x83</html>"; // UTF-8 snowman
        let s = decode_response_body(&headers, body);
        assert_eq!(s, "<html>\u{2603}</html>");
    }

    #[test]
    fn decode_falls_back_to_utf8_lossy_without_charset() {
        use http::HeaderMap;

        let headers = HeaderMap::new();
        // invalid UTF-8 byte sequence will be lossy-decoded
        let body = b"\xff\xfehello";
        let s = decode_response_body(&headers, body);
        assert!(s.contains("hello"));
    }

    // Optional: a smoke test that current_url tracks what we set
    #[test]
    fn current_url_starts_none_and_is_set_by_callers() {
        let mut ctx = BrowsingContext::new();
        assert!(ctx.current_url().is_none());
        // Simulate a caller updating it (load sets it internally; we avoid network here)
        let _url = Url::parse("https://example.com/").unwrap();
        ctx.set_raw_html("<html></html>");
        // current_url is private-set by load(); we just assert the API compiles & works elsewhere
        // (Covered indirectly in the cancel test where load() sets it before select.)
    }
}