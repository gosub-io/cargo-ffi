use crate::engine::cookies::CookieJarHandle;
use crate::engine::storage::types::PartitionPolicy;
use crate::engine::storage::{PartitionKey, StorageEvent, StorageHandles};
use crate::engine::zone::ZoneId;
use crate::engine::BrowsingContext;
use crate::render::backend::{
    ErasedSurface, PresentMode, RenderBackend, RgbaImage, SurfaceSize,
};
use crate::render::Viewport;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::runtime::Runtime;
use tokio_util::sync::CancellationToken;
use tokio::sync::mpsc::Sender;
use tokio::sync::oneshot;
use url::Url;
use uuid::Uuid;
use crate::engine::events::{EngineCommand, EngineEvent};
use crate::net::Response;
use crate::storage::types::compute_partition_key;
use crate::zone::ZoneServices;

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


/// A handle to a tab's command channel, allowing sending commands to the tab.
pub struct TabHandle {
    pub id: TabId,
    pub cmd_tx: Sender<EngineCommand>,
}

impl TabHandle {
    pub fn id(&self) -> TabId {
        self.id
    }

    pub async fn emit(&self, cmd: EngineCommand) -> Result<(), tokio::sync::mpsc::error::SendError<EngineCommand>> {
        self.cmd_tx.send(cmd).await
    }
}

/// Builder for creating a new tab within a zone.
pub struct TabBuilder {
    event_tx: Option<Sender<EngineEvent>>,
}

impl TabBuilder {
    pub fn new() -> Self {
        Self { event_tx: None }
    }

    pub fn with_event_tx(mut self, tx: Sender<EngineEvent>) -> Self {
        self.event_tx = Some(tx);
        self
    }

    pub(crate) fn take_event_tx(&mut self) -> Sender<EngineEvent> {
        self.event_tx.take().expect("TabBuilder event_tx already taken or not yet set")
    }
}

/// Represents an in-flight network load operation. It allows for easy cancellation in case
/// the load is no longer needed (e.g., user navigated away).
struct InflightLoad {
    cancel: CancellationToken,
    rx: oneshot::Receiver<anyhow::Result<Response>>,
}

/// State for the tab task driving a single tab.
struct TabTaskState {
    /// Is drawing enabled (vs suspended)
    drawing_enabled: bool,
    /// Target frames per second when drawing is enabled
    fps: u32,
    /// Interval timer for driving ticks
    interval: tokio::time::Interval,
    /// Current in-flight load operation, if any
    load: Option<InflightLoad>,
    /// Current viewport size
    viewport: Viewport,
    /// Has something changed that requires a redraw
    dirty: bool,
}



/// Current state of the tab. This is a state machine that defines what the tab is doing at the moment.
#[derive(Default, Debug, Clone, PartialEq, Eq)]
pub enum TabState {
    /// Tab is idle (no pending network, animations, or rendering).
    #[default]
    Idle,

    /// A navigation has been requested but not started yet.
    /// The next `tick()` will transition to [`TabState::Loading`].
    PendingLoad(Url),

    /// The tab is fetching network resources (main document).
    /// When done, transitions to [`TabState::Loaded`] on success or [`TabState::Failed`] on error.
    Loading,

    /// Main document has been received and staged into the engine.
    /// The next `tick()` will begin rendering via [`TabState::PendingRendering`].
    Loaded,

    /// A render has been requested for the given viewport.
    PendingRendering(Viewport),

    /// The engine is producing a new surface for the current content.
    Rendering(Viewport),

    /// A new surface is ready for painting. The next `tick()` typically
    /// returns to [`TabState::Idle`] and sets `needs_redraw = true` in [`TickResult`].
    Rendered(Viewport),

    /// A fatal error occurred while loading or rendering.
    Failed(String),
}

/// Activity mode for a [`Tab`]. Schedulers can allocate CPU/time by mode.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum TabActivityMode {
    /// Foreground: fully active (network, layout, paint, animations ~60 Hz).
    Active,

    /// Background with animations alive but throttled (e.g., ~10 Hz).
    BackgroundLive,

    /// Background with minimal ticking (network/JS timers only, e.g., ~1 Hz).
    BackgroundIdle,

    /// Suspended: no ticking until an event or visibility change.
    Suspended,
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
    pub(crate) fn dispatch_storage_event_to_same_origin_docs(
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
        //         doc.runtime().dispatch_storage_event(
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



pub(crate) fn spawn_tab_task(
    tab_id: TabId,
    mut cmd_rx: tokio::sync::mpsc::Receiver<EngineCommand>,
    event_tx: Sender<EngineEvent>,
    services: ZoneServices,
    viewport: Viewport,
    runtime: Arc<Runtime>,
    backend: Arc<Mutex<Box<dyn RenderBackend + Send>>>,
) {
    tokio::spawn(async move {
        println!("Spawned tab task for tab {:?}", tab_id);

        let mut tab = Tab::new(
            services.zone_id,
            runtime.clone(),
            viewport,
            Some(services.cookie_jar.clone()),
        );

        let fps = 60;
        let mut state = TabTaskState {
            drawing_enabled: false,
            fps,
            interval: tokio::time::interval(std::time::Duration::from_millis(1000/fps as u64)),
            load: None,
            viewport,
            dirty: true,
        };

        let _ = event_tx.send(EngineEvent::TabCreated { tab: tab_id }).await;

        loop {
            tokio::select! {
                // Tick interval for driving the redraws
                _ = state.interval.tick(), if state.drawing_enabled => {
                    if let Err(e) = drive_once(&mut tab, &backend, &event_tx, &mut state.dirty).await {
                        tab.state = TabState::Failed(format!("Tab {:?} tick error: {}", tab_id, e));
                        state.dirty = true;
                    }
                }

                // Handle in-flight load completion
                res = async {
                    if let Some(load) = &mut state.load {
                        load.rx.await
                    } else {
                        futures::future::pending().await
                    }
                } => {
                    match res {
                        Ok(Ok(resp)) => {
                            if let Some(ref jar) = tab.cookie_jar {
                                jar.write().unwrap().store_response_cookies(&resp.url, &resp.headers);
                            }

                            tab.current_url = Some(resp.url.clone());
                            tab.is_loading = false;
                            tab.is_error = false;
                            tab.pending_url = None;
                            tab.state = TabState::Loaded;

                            tab.context.set_raw_html(
                                String::from_utf8_lossy(resp.body().as_slice()).as_ref()
                            );

                            let _ = event_tx.send(EngineEvent::PageCommitted { tab: tab_id, url: resp.url.clone() }).await;
                            state.dirty = true;
                        }
                        Ok(Err(e)) => {
                            tab.state = TabState::Failed(format!("Tab {:?} error: {}", tab_id, e));
                            tab.is_loading = false;
                            tab.is_error = true;
                            state.dirty = true;
                        }
                        Err(_cancelled_or_replaced) => {
                            // Load was cancelled or replaced, do nothing
                            println!("Tab {:?} load was cancelled or replaced", tab_id);
                        }
                    }
                }

                // Handle incoming commands
                msg = cmd_rx.recv() => {
                    let Some(cmd) = msg else {
                        // Channel closed, exit the loop
                        break;
                    };

                    match cmd {
                        EngineCommand::Navigate { url } => {
                            println!("Tab {:?} navigating to URL: {}", tab_id, url);

                            // Cancel any in-flight load
                            if let Some(load) = state.load.take() {
                                load.cancel.cancel();
                            }

                            // Compute storage and bind @TODO: do we need to do this for each navigation?
                            let pk = compute_partition_key(&url, &services.partition_policy);
                            let origin = url.origin().clone();
                            let local = services.storage.local_for(services.zone_id, &pk, &origin).expect("cannot get local storage for tab");
                            let session = services.storage.session_for(services.zone_id, tab_id, &pk, &origin).expect("cannot get session storage for tab");
                            tab.bind_storage(StorageHandles { local, session });

                            let cancel = CancellationToken::new();
                            let (tx, rx) = oneshot::channel();

                            let cancel_child = cancel.child_token();
                            tokio::spawn(async move {
                                let res = load_main_document(url.clone(), cancel_child).await;
                                let _ = tx.send(res);
                            });

                            state.load = Some(InflightLoad { cancel, rx });
                            tab.state = TabState::Loading;
                            state.dirty = true;
                            // let _ = event_tx.send(EngineEvent::ConnectionEstablished { tab: tab_id, url: url.clone() }).await;
                        }
                        EngineCommand::Reload(..) => {
                            tab.execute_command(EngineCommand::Reload());
                            state.dirty = true;
                        }
                        EngineCommand::Resize { width, height } => {
                            state.viewport.width = width;
                            state.viewport.height = height;
                            tab.handle_event(EngineEvent::Resize { width, height });
                            state.dirty = true;
                        }

                        EngineCommand::MouseMove { x, y } => {
                            tab.handle_event(EngineEvent::MouseMove { x, y });
                            state.dirty = true;
                        }

                        EngineCommand::MouseDown { button, x, y } => {
                            tab.handle_event(EngineEvent::MouseDown { button, x, y });
                            state.dirty = true;
                        }

                        EngineCommand::MouseUp { button, x, y } => {
                            tab.handle_event(EngineEvent::MouseUp { button, x, y });
                            state.dirty = true;
                        }

                        EngineCommand::KeyDown { key, code, modifiers } => {
                            tab.handle_event(EngineEvent::KeyDown { key, code, modifiers });
                            state.dirty = true;
                        }

                        EngineCommand::KeyUp { key, code, modifiers } => {
                            tab.handle_event(EngineEvent::KeyUp { key, code, modifiers });
                            state.dirty = true;
                        }

                        EngineCommand::InputChar { character } => {
                            tab.handle_event(EngineEvent::InputChar { character });
                            state.dirty = true;
                        }

                        EngineCommand::ResumeDrawing { fps: wanted_fps } => {
                            state.drawing_enabled = true;
                            state.fps = wanted_fps.max(1) as u32;
                            state.interval = tokio::time::interval(Duration::from_millis(1000 / (state.fps as u64)));
                            state.dirty = true;
                            println!("Tab {:?} resumed drawing FPS: {} / {}", tab_id, state.fps, state.drawing_enabled);
                        }
                        EngineCommand::SuspendDrawing=> {
                            state.drawing_enabled = false;
                            println!("Tab {:?} suspended drawing: at fps: {} / {}", tab_id, state.fps, state.drawing_enabled);
                        }
                        _ => {
                            println!("Tab {:?} received command: {:?}", tab_id, cmd);
                            state.dirty = true;
                        }
                    }
                }
            }
        }

        // Cleanup code here
        println!("Tab task for tab {:?} exiting", tab_id);
        let _ = event_tx.send(EngineEvent::TabClosed { tab: tab_id }).await;
        services.storage.drop_tab(services.zone_id, tab_id);
    });
}

async fn drive_once(
    tab: &mut Tab,
    _backend: &Arc<Mutex<Box<dyn RenderBackend + Send>>>,
    _event_tx: &Sender<EngineEvent>,
    dirty: &mut bool,
) -> anyhow::Result<()> {

    match tab.state.clone() {
        TabState::Idle => {
            if *dirty {
                tab.state = TabState::PendingRendering(*tab.context.viewport());
            }
        }

        TabState::PendingLoad(url) => {
            tab.state = TabState::Loading;
            tab.is_loading = true;
            tab.pending_url = Some(url.clone());
            tab.context.start_loading(url.clone());
        }
        _ => {
            // Handle other states as needed
            println!("Tab {:?} in state: {:?}", tab.id, tab.state);
        }
    }

    Ok(())
}
