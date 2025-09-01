use std::sync::Arc;
use tokio::runtime::Runtime;
use url::Url;
use uuid::Uuid;
use crate::cookies::CookieJarHandle;
use crate::engine::BrowsingContext;
use crate::engine::events::{EngineCommand, EngineEvent};
use crate::render::backend::{ErasedSurface, PresentMode, RenderBackend, RgbaImage, SurfaceSize};
use crate::render::Viewport;
use crate::storage::{PartitionKey, StorageEvent, StorageHandles};
use crate::storage::types::PartitionPolicy;
use crate::tab::structs::{TabActivityMode, TabState};
use crate::zone::ZoneId;

/// A unique identifier for a browser tab within a [`GosubEngine`](crate::engine::GosubEngine).
///
/// Internally, a `TabId` is a wrapper around a [`Uuid`], ensuring global
/// uniqueness for each tab opened in the engine. `TabId` implements
/// common traits such as `Copy`, `Clone`, `Eq`, `Hash`, and ordering traits,
/// so it can be freely duplicated, compared, sorted, or used as a key in
/// hash maps.
///
/// **Note:** The use of [`Uuid`] is an implementation detail and may change
/// in the future without notice. You should not depend on the internal
/// representation; always treat `TabId` as an opaque handle.
///
/// # Purpose
///
/// Tabs in Gosub are lightweight handles representing an open page
/// (or a rendering context) within a [`Zone`](crate::engine::zone::Zone). `TabId` allows the engine
/// and user code to unambiguously reference and operate on a specific tab,
/// even if tabs are opened or closed dynamically.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TabId(Uuid);

impl TabId {
    /// Create a new unique `TabId` using a random UUID.
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

pub struct Tab {
    /// ID of the tab
    pub id: TabId,
    /// ID of the zone in which this tab resides
    pub zone_id: ZoneId,
    /// Browsing context running for this tab
    pub context: BrowsingContext,
    /// State of the tab (idle, loading, loaded, etc.)
    pub state: TabState,
    /// Current tab mode (idle, live, background)
    pub mode: TabActivityMode,

    /// Favicon binary data for the current tab
    pub favicon: Vec<u8>,
    /// Title of the current tab
    pub title: String,

    /// URL that ready to load or is loading
    pub pending_url: Option<Url>,
    /// Current URL that is now loaded
    pub current_url: Option<Url>,
    /// Is the current URL being loaded
    pub is_loading: bool,
    /// Is there an error in the current tab?
    pub is_error: bool,

    /// Cookie jar for this tab. This is shared with the rest of the zone tabs
    pub cookie_jar: Option<CookieJarHandle>,

    /// Storage partition key
    pub partition_key: PartitionKey,
    /// Storage partition policy
    pub partition_policy: PartitionPolicy,

    /// Backend rendering
    pub thumbnail: Option<RgbaImage>, // Thumbnail image of the tab in case the tab is not visible
    surface: Option<Box<dyn ErasedSurface>>, // Surface on which the browsing context can render the tab
    surface_size: SurfaceSize, // Size of the surface (does not have to match viewport)
    present_mode: PresentMode, // Present mode for the surface?

    /// The viewport that was committed for the in-flight/last render
    committed_viewport: Viewport,
    /// The newest viewport requested by the tab, which may differ from the committed one.
    desired_viewport: Viewport,
    /// Set when a resize arrives while rendering. Causes an immediate re-render after finihsing the current rendering.
    dirty_after_inflight: bool,
}

impl Tab {
    /// Create a new tab bound to `zone_id`, with a runtime, initial viewport,
    /// and an optional zone-shared cookie jar handle.
    ///
    /// The tab starts in [`TabState::Idle`], [`TabActivityMode::Active`], and with
    /// [`PartitionKey::None`]/[`PartitionPolicy::TopLevelOrigin`].
    pub fn new(
        zone_id: ZoneId,
        runtime: Arc<Runtime>,
        viewport: Viewport,
        cookie_jar: Option<CookieJarHandle>,
    ) -> Self {
        let mut tab = Self {
            id: TabId::new(),
            zone_id,
            state: TabState::Idle,
            context: BrowsingContext::new(runtime),

            favicon: vec![],              // Placeholder for favicon data
            title: "New Tab".to_string(), // Title of the new tab

            pending_url: None,
            current_url: None,
            is_loading: false,
            is_error: false,

            mode: TabActivityMode::Active, // Default mode is active

            cookie_jar,
            partition_key: PartitionKey::None, // Start with no partition key
            partition_policy: PartitionPolicy::TopLevelOrigin,

            surface: None,
            surface_size: SurfaceSize {
                width: 1,
                height: 1,
            },
            present_mode: PresentMode::Fifo,
            thumbnail: None, // No thumbnail initially

            committed_viewport: viewport,
            desired_viewport: viewport,
            dirty_after_inflight: false,
        };

        tab.context.set_viewport(viewport);

        tab
    }

    /// Navigate to a URL (string is parsed into a `Url`). On success, moves the
    /// tab to [`TabState::PendingLoad`]. Invalid URLs are ignored and logged.
    pub fn navigate_to(&mut self, url: impl Into<String>) {
        let url = match Url::parse(&url.into()) {
            Ok(url) => url,
            Err(e) => {
                // Can't parse string to a URL to load
                log::error!("Tab[{:?}]: Cannot parse URL: {}", self.id, e);
                return;
            }
        };

        self.state = TabState::PendingLoad(url.into());
        self.is_loading = true;
    }

    /// Bind local+session storage handles into the underlying browsing context.
    /// Call this after creating the tab or when the zone’s storage changes.
    pub fn bind_storage(&mut self, storage: StorageHandles) {
        self.context.bind_storage(storage.local, storage.session);
    }

    /// Set a new viewport and schedule a re-render
    /// by transitioning to [`TabState::PendingRendering`].
    pub fn set_viewport(&mut self, viewport: Viewport) {
        self.surface_size = SurfaceSize {
            width: viewport.width,
            height: viewport.height,
        };

        self.context.set_viewport(viewport);
        self.desired_viewport = viewport;

        if let TabState::Rendering(_) = self.state {
            // Mark the fact that we have triggered a resize during the rendering of the tab
            self.dirty_after_inflight = true;
        } else {
            self.state = TabState::PendingRendering(self.desired_viewport);
        }
    }

    /// Get the current snapshotted image of the tab.
    pub fn thumbnail(&self) -> Option<&RgbaImage> {
        self.thumbnail.as_ref()
    }

    /// Dispatch a storage event to same-origin documents in this tab (placeholder).
    /// Intended for HTML5 storage event semantics.
    pub(crate) fn dispatch_storage_events(
        &mut self,
        _origin: &url::Origin,
        _include_iframes: bool,
        _ev: &StorageEvent,
    ) {
        // Pseudocode stuff.. need to fill in what it actually needs to do
        // for doc in self.iter_documents(include_iframes) {
        //     if doc.origin() == origin {
        //         // Don’t fire the event at the *mutating document* itself.
        //         if Some(self.id) == ev.source_tab && doc.is_the_mutating_document() {
        //             continue;
        //         }
        //         doc.A().dispatch_storage_event(
        //             ev.key.as_deref(),
        //             ev.old_value.as_deref(),
        //             ev.new_value.as_deref(),
        //             doc.url().to_string(),
        //             match ev.scope { StorageScope::Local => "local", StorageScope::Session => "session" }
        //         );
        //     }
        // }
    }

    /// Ensure the tab has a surface of the given size, creating it if necessary.
    fn ensure_surface(
        &mut self,
        backend: &dyn RenderBackend,
        size: SurfaceSize,
    ) -> anyhow::Result<()> {
        if let Some(ref surf) = self.surface {
            if surf.size() == size {
                return Ok(());
            }
        }
        self.surface = Some(backend.create_surface(size, self.present_mode)?);
        Ok(())
    }

    pub fn handle_event(_event: EngineEvent) -> anyhow::Result<()> {
        Ok(())
    }

    pub fn execute_command(_cmd: EngineCommand) -> anyhow::Result<()> {
        Ok(())
    }
}
