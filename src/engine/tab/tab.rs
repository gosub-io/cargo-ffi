use crate::engine::events::EngineEvent;
use crate::engine::{BrowsingContext, DEFAULT_CHANNEL_CAPACITY};
use crate::events::TabCommand;
use crate::render::backend::{ErasedSurface, PresentMode, RenderBackend, RgbaImage, SurfaceSize};
use crate::render::Viewport;
use crate::storage::types::compute_partition_key;
use crate::storage::{StorageEvent, StorageHandles};
use crate::tab::structs::{InflightLoad, TabActivityMode, TabState};
use crate::tab::{EffectiveTabServices, TabHandle};
use crate::zone::{ZoneContext, ZoneId};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use url::Url;
use uuid::Uuid;

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

/// Things shared upwards to the zone
pub struct TabSink {
    pub metrics: bool,
}

/// State for the tab task driving a single tab.
pub(crate) struct TabRuntime {
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

impl Default for TabRuntime {
    fn default() -> Self {
        let fps = 60;

        Self {
            drawing_enabled: true,
            fps,
            interval: tokio::time::interval(Duration::from_secs_f64(1.0 / fps as f64)),
            load: None,
            viewport: Viewport::default(),
            dirty: false,
        }
    }
}

/// A single browser tab within a [`Zone`](crate::engine::zone::Zone).
pub struct Tab {
    /// ID of the tab
    pub tab_id: TabId,
    /// ID of the zone in which this tab resides
    pub zone_id: ZoneId,

    /// Shared context from the tab
    zone_context: Arc<ZoneContext>,
    /// Sink for sending events upwards
    sink: Arc<TabSink>,

    /// Browsing context running for this tab
    pub context: BrowsingContext,
    /// State of the tab (idle, loading, loaded, etc.)
    pub state: TabState,
    /// Current tab mode (idle, live, background)
    pub mode: TabActivityMode,
    /// Receiver for incoming tab commands
    cmd_rx: mpsc::Receiver<TabCommand>,
    cmd_tx: mpsc::Sender<TabCommand>,
    // Effective tab services that we can use
    services: EffectiveTabServices,

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

    /// Keeps track of the tab worker runtime data
    runtime: TabRuntime,

    // Join handle for the spawned tab worker (if any)
    join_handle: Option<JoinHandle<()>>,
}

impl Tab {
    /// Creats a new tab. Does NOT spawn the tab worker
    pub fn new(
        zone_id: ZoneId,
        services: EffectiveTabServices,
        zone_context: Arc<ZoneContext>,
    ) -> anyhow::Result<Self> {
        let (cmd_tx, cmd_rx) = mpsc::channel::<TabCommand>(DEFAULT_CHANNEL_CAPACITY);
        let tab_id = TabId::new();

        Ok(Self {
            tab_id,
            zone_id,
            zone_context,
            sink: Arc::new(TabSink { metrics: false }),
            context: BrowsingContext::new(),
            state: TabState::Idle,
            mode: TabActivityMode::Active,
            cmd_rx,
            cmd_tx,
            services,
            favicon: vec![],
            title: "New Tab".to_string(),
            pending_url: None,
            current_url: None,
            is_loading: false,
            is_error: false,
            thumbnail: None,
            surface: None,
            surface_size: SurfaceSize {
                width: 1,
                height: 1,
            },
            present_mode: PresentMode::Fifo,
            committed_viewport: Default::default(),
            desired_viewport: Default::default(),
            dirty_after_inflight: false,
            runtime: TabRuntime::default(),
            join_handle: None,
        })
    }

    /// Creates a new tab and spawns the tab-worker
    pub fn new_on_thread(
        zone_id: ZoneId,
        services: EffectiveTabServices,
        zone_context: Arc<ZoneContext>,
    ) -> anyhow::Result<(TabHandle, JoinHandle<()>)> {
        let this = Self::new(zone_id, services, zone_context)?;

        let handle = this.handle();

        let join_handle = tokio::spawn(this.run());

        Ok((handle, join_handle))
    }

    /// Returns a tab handle
    pub fn handle(&self) -> TabHandle {
        TabHandle {
            tab_id: self.tab_id,
            cmd_tx: self.cmd_tx.clone(),
            sink: self.sink.clone(),
        }
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
        origin: &url::Origin,
        include_iframes: bool,
        ev: &StorageEvent,
    ) {
        println!("Tab {:?} dispatch_storage_events called", self.tab_id);
        dbg!(&origin);
        dbg!(&include_iframes);
        dbg!(&ev);

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

    /// Main tab worker loop. Drives state transitions, rendering, and command handling.
    pub async fn run(mut self) {
        println!("Worker started for tab {:?}", self.tab_id);

        let _ = self.zone_context.event_tx.send(EngineEvent::TabCreated {
            tab_id: self.tab_id,
            zone_id: self.zone_id,
        });

        loop {
            tokio::select! {
                // Tick interval for driving the redraws
                _ = self.runtime.interval.tick(), if self.runtime.drawing_enabled => {
                    if let Err(e) = self.drive_once().await {
                        self.state = TabState::Failed(format!("Tab {:?} tick error: {}", self.tab_id, e));
                        self.runtime.dirty = true;
                    }
                }

                // Handle in-flight load completion
                res = async {
                    if let Some(load) = &mut self.runtime.load {
                        load.rx.await
                    } else {
                        futures::future::pending().await
                    }
                } => {
                    match res {
                        // Loading completed
                        Ok(Ok(resp)) => {
                            if let Some(ref jar) = self.cookie_jar {
                                jar.write().unwrap().store_response_cookies(&resp.url, &resp.headers);
                            }

                            self.current_url = Some(resp.url.clone());
                            self.is_loading = false;
                            self.is_error = false;
                            self.pending_url = None;
                            self.state = TabState::Loaded;

                            self.context.set_raw_html(
                                String::from_utf8_lossy(resp.body().as_slice()).as_ref()
                            );

                            let _ = self.zone_context.event_tx.send(EngineEvent::PageCommitted { tab: self.tab_id, url: resp.url.clone() }).await;
                            self.runtime.dirty = true;
                        }
                        // Loading errrored
                        Ok(Err(e)) => {
                            self.state = TabState::Failed(format!("Tab {:?} error: {}", self.tab_id, e));
                            self.is_loading = false;
                            self.is_error = true;
                            self.runtime.dirty = true;
                        }
                        // Loading was cancelled or replaced
                        Err(_cancelled_or_replaced) => {
                            // Load was cancelled or replaced, do nothing
                            println!("Tab {:?} load was cancelled or replaced", self.tab_id);
                        }
                    }
                }

                // Handle incoming tab commands
                msg = self.cmd_rx.recv() => {
                    let Some(cmd) = msg else {
                        // Channel closed, exit the loop
                        break;
                    };

                    match cmd {
                        TabCommand::CloseTab => {
                            println!("Tab {:?} received Close command, exiting", self.tab_id);
                            break;
                        }
                        _ => self.handle_tab_command(cmd)
                    }
                }
            }
        }

        // Cleanup tab
        println!("Tab task for tab {:?} exiting", self.tab_id);
        let _ = self.zone_context.event_tx.send(EngineEvent::TabClosed {
            tab_id: self.tab_id,
            zone_id: self.zone_id,
        });
        self.services.storage.drop_tab(self.zone_id, self.tab_id);
    }

    fn handle_tab_command(&mut self, cmd: TabCommand) {
        match cmd {
            TabCommand::Navigate { url } => {
                println!(
                    "Tab {:?} received navigate command to URL: {}",
                    self.tab_id, url
                );
                self.navigate_to(&url, false);
            }
            TabCommand::Reload { ignore_cache } => {
                println!("Tab {:?} reloading current URL", self.tab_id);
                self.navigate_to(
                    self.current_url
                        .as_ref()
                        .map(|u| u.as_str())
                        .unwrap_or("about:blank"),
                    ignore_cache,
                );
                self.runtime.dirty = true;
            }
            TabCommand::Resize { width, height } => {
                println!(
                    "Tab {:?} received resize: {} x {}",
                    self.tab_id, width, height
                );

                self.runtime.viewport.width = width;
                self.runtime.viewport.height = height;
                self.runtime.dirty = true;
            }

            TabCommand::MouseMove { x, y } => {
                println!("Tab {:?} received mouse move: {},{}", self.tab_id, x, y);
                self.runtime.dirty = true;
            }

            TabCommand::MouseDown { button, x, y } => {
                println!(
                    "Tab {:?} received mouse down: {} / {}, {}",
                    self.tab_id, button, x, y
                );
                self.runtime.dirty = true;
            }

            TabCommand::MouseUp { button, x, y } => {
                println!(
                    "Tab {:?} received mouse up: {} / {}, {}",
                    self.tab_id, button, x, y
                );
                self.runtime.dirty = true;
            }

            TabCommand::KeyDown {
                key,
                code,
                modifiers,
            } => {
                println!(
                    "Tab {:?} received key down: {} / {} / {}",
                    self.tab_id, key, code, modifiers
                );
                self.runtime.dirty = true;
            }

            TabCommand::KeyUp {
                key,
                code,
                modifiers,
            } => {
                println!("Tab {:?} received key up: {} / {}", self.tab_id, key, code);
                self.runtime.dirty = true;
            }

            TabCommand::InputChar { character } => {
                println!("Tab {:?} received char input: {}", self.tab_id, character);
                self.runtime.dirty = true;
            }

            TabCommand::ResumeDrawing { fps: wanted_fps } => {
                self.runtime.drawing_enabled = true;
                self.runtime.fps = wanted_fps.max(1) as u32;
                self.runtime.interval =
                    tokio::time::interval(Duration::from_millis(1000 / (self.runtime.fps as u64)));
                self.runtime.dirty = true;
                println!(
                    "Tab {:?} resumed drawing FPS: {} / {}",
                    self.tab_id, self.runtime.fps, self.runtime.drawing_enabled
                );
            }
            TabCommand::SuspendDrawing => {
                self.runtime.drawing_enabled = false;
                println!(
                    "Tab {:?} suspended drawing: at fps: {} / {}",
                    self.tab_id, self.runtime.fps, self.runtime.drawing_enabled
                );
            }
            _ => {
                println!("Tab {:?} received command: {:?}", self.tab_id, cmd);
                self.runtime.dirty = true;
            }
        }
    }

    // /// Navigate to a URL (string is parsed into a `Url`). On success, moves the
    // /// tab to [`TabState::PendingLoad`]. Invalid URLs are ignored and logged.
    // pub fn navigate_to(&mut self, url: impl Into<String>) {
    //     let url = match Url::parse(&url.into()) {
    //         Ok(url) => url,
    //         Err(e) => {
    //             // Can't parse string to a URL to load
    //             log::error!("Tab[{:?}]: Cannot parse URL: {}", self.tab_id, e);
    //             return;
    //         }
    //     };
    //
    //     self.state = TabState::PendingLoad(url.into());
    //     self.is_loading = true;
    // }

    fn navigate_to(&mut self, url: impl Into<String>, ignore_cache: bool) {
        // Cancel any in-flight load
        if let Some(load) = self.runtime.load.take() {
            load.cancel.cancel();
        }

        let unvalidated_url = url.into();

        let real_url = Url::parse(&unvalidated_url);
        let real_url = match real_url {
            Ok(url) => url,
            Err(e) => {
                log::error!("Tab[{:?}]: Cannot parse URL: {}", self.tab_id, e);
                self.zone_context
                    .event_tx
                    .send(EngineEvent::NavigationFailed {
                        tab_id: self.tab_id,
                        url: unvalidated_url,
                        error: format!("Cannot parse URL: {}", e),
                    });
                return;
            }
        };

        // Compute storage and bind @TODO: do we need to do this for each navigation?
        let pk = compute_partition_key(&real_url, self.services.partition_policy);
        let origin = real_url.origin().clone();
        let local = self
            .services
            .storage
            .local_for(self.zone_id, &pk, &origin)
            .expect("cannot get local storage for tab");
        let session = self
            .services
            .storage
            .session_for(self.zone_id, self.tab_id, &pk, &origin)
            .expect("cannot get session storage for tab");
        self.bind_storage(StorageHandles { local, session });

        let cancel = CancellationToken::new();
        let fut = self.context.load(real_url.clone(), cancel.child_token());

        // tokio::select! {
        //                 res = fut => {
        //                 }
        //             }
        // let (tx, rx) = oneshot::channel();
        //
        // let cancel_child = cancel.child_token();
        // tokio::spawn(async move {
        //     let res = load_main_document(url.clone(), cancel_child).await;
        //     let _ = tx.send(res);
        // });

        self.runtime.load = Some(InflightLoad { cancel, rx });
        self.state = TabState::Loading;
        self.runtime.dirty = true;
        // let _ = event_tx.send(EngineEvent::ConnectionEstablished { tab: tab_id, url: url.clone() }).await;

        let url = match Url::parse(url) {
            Ok(url) => url,
            Err(e) => {
                log::error!("Tab[{:?}]: Cannot parse URL: {}", self.tab_id, e);
                return;
            }
        };

        self.state = TabState::PendingLoad(url.clone());
        self.is_loading = true;
        self.pending_url = Some(url);
    }

    /// "Tick" the tab once, driving state transitions, rendering, etc.
    async fn drive_once(&mut self) -> anyhow::Result<()> {
        match self.state.clone() {
            TabState::Idle => {
                if self.dirty {
                    self.state = TabState::PendingRendering(*self.context.viewport());
                }
            }

            TabState::PendingLoad(url) => {
                self.state = TabState::Loading;
                self.is_loading = true;
                self.pending_url = Some(url.clone());
                self.context.start_loading(url.clone());
            }
            _ => {
                // Handle other states as needed
                println!("Tab {:?} in state: {:?}", self.tab_id, self.state);
            }
        }

        Ok(())
    }
}
