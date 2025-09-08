use crate::engine::events::{CancelReason, EngineEvent, LoadEvent, NavigationEvent};
use crate::engine::BrowsingContext;
use crate::events::TabCommand;
use crate::render::backend::{ErasedSurface, PresentMode, RenderBackend, RgbaImage, SurfaceSize};
use crate::render::{DevicePixelRatio, Viewport};
use crate::storage::types::compute_partition_key;
use crate::storage::{StorageEvent, StorageHandles};
use crate::tab::{TabId, TabSink};
use crate::zone::{ZoneContext, ZoneId};
use std::sync::Arc;
use std::time::Instant;
use anyhow::Context;
use futures_util::TryStreamExt;
use tokio::select;
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;
use tokio::time::MissedTickBehavior;
use tokio_util::io::StreamReader;
use tokio_util::sync::CancellationToken;
use url::Url;
use crate::engine::types::NavigationId;
use crate::net::DocumentLoadResult;
use crate::net::types::{FetchKey, FetchRequest, FetchResult, Initiator, Priority, ResourceKind};
use crate::tab::services::EffectiveTabServices;
use crate::tab::state::{InflightLoad, TabActivityMode, TabRuntime, TabState};

pub struct TabWorker {
    /// ID of the tab
    pub tab_id: TabId,
    /// ID of the zone in which this tab resides
    pub zone_id: ZoneId,

    /// Shared context from the tab
    zone_context: Arc<ZoneContext>,
    // Effective tab services that we can use
    services: EffectiveTabServices,

    /// Sink for sending events upwards
    sink: Arc<TabSink>,

    /// Receiver for incoming tab commands
    cmd_rx: mpsc::Receiver<TabCommand>,

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

    // ** Backend rendering

    // Thumbnail image of the tab in case the tab is not visible
    pub thumbnail: Option<RgbaImage>,
    // Surface on which the browsing context can render the tab
    surface: Option<Box<dyn ErasedSurface + Send>>,
    // // Size of the surface (does not have to match viewport)
    // surface_size: SurfaceSize,
    // Present mode for the surface?
    present_mode: PresentMode,
    /// Device Pixel Ratio
    dpr: DevicePixelRatio,
    /// The viewport that was committed for the in-flight/last render
    committed_viewport: Viewport,
    /// The newest viewport requested by the tab, which may differ from the committed one.
    desired_viewport: Viewport,
    /// Set when a resize arrives while rendering. Causes an immediate re-render after finishing the current rendering.
    dirty_after_inflight: bool,

    /// Keeps track of the tab worker runtime data
    pub(crate) runtime: TabRuntime,
}


impl TabWorker {
    /// Creates a new tab. Does NOT spawn the tab worker
    pub fn new(
        tab_id: TabId,
        zone_id: ZoneId,
        services: EffectiveTabServices,
        zone_context: Arc<ZoneContext>,
        sink: Arc<TabSink>,
        cmd_rx: mpsc::Receiver<TabCommand>,
    ) -> Self {
        Self {
            tab_id,
            zone_id,
            services,
            zone_context,
            sink,
            cmd_rx,
            context: BrowsingContext::new(),
            state: TabState::Idle,
            mode: TabActivityMode::Active,
            favicon: vec![],
            title: "New Tab".to_string(),
            pending_url: None,
            current_url: None,
            is_loading: false,
            is_error: false,
            thumbnail: None,
            surface: None,
            // surface_size: SurfaceSize { width: 1, height: 1 },
            present_mode: PresentMode::Fifo,
            dpr: DevicePixelRatio(1.0),
            committed_viewport: Default::default(),
            desired_viewport: Default::default(),
            dirty_after_inflight: false,
            runtime: TabRuntime::default(),
        }
    }

    pub fn spawn_named(self, name: impl AsRef<str>) -> anyhow::Result<JoinHandle<()>> {
        let name = name.as_ref().to_owned();
        let join = tokio::task::Builder::new().name(&name).spawn(self.run())?;
        Ok(join)
    }

    pub async fn run(mut self) {
        self.sink.set_worker_started_now();

        // Announce creation
        self.send_event(EngineEvent::TabCreated {
            tab_id: self.tab_id,
            zone_id: self.zone_id,
        });

        loop {
            tokio::select! {
                // Tick for redraws
                _ = self.runtime.interval.tick(), if self.runtime.drawing_enabled => {
                    if let Err(e) = self.tick_draw().await {
                        self.state = TabState::Failed(format!("Tab {:?} tick error: {}", self.tab_id, e));
                        self.runtime.dirty = true;
                    }
                }

                // In-flight load completion
                res = async {
                    // Wait until the self.runtime.load.rx channel (if any) resolves
                    if let Some(load) = &mut self.runtime.load {
                        (&mut load.rx).await
                    } else {
                        // No self.runtime.load found, so we await indefinitately (thus not triggering this branch)
                        futures::future::pending().await
                    }
                } => {
                    self.on_load_result(res);
                }

                // Handle incoming tab commands
                msg = self.cmd_rx.recv() => {
                    let Some(cmd) = msg else { break; };
                    if self.handle_tab_command(cmd).is_break() {
                        break;
                    }
                }
            }
        }

        self.send_event(EngineEvent::TabClosed { tab_id: self.tab_id, zone_id: self.zone_id });
        self.services.storage.drop_tab(self.zone_id, self.tab_id);
    }

    fn on_load_result(&mut self, res: Result<(NavigationId, DocumentLoadResult), oneshot::error::RecvError>) {
        let Some(current) = self.runtime.load.as_ref() else {
            return;
        };
        let current_nav = current.nav_id;

        match res {
            Ok((completed_nav , Ok(resp))) => {
                if completed_nav != current_nav {
                    return
                }

                // Store any cookies found in the response into the cookie jar
                self.services.cookie_jar.write().store_response_cookies(&resp.url, &resp.headers);

                self.current_url = Some(resp.url.clone());
                self.pending_url = None;
                self.is_loading = false;
                self.is_error = false;
                self.state = TabState::Loaded;
                self.runtime.dirty = true;
                self.runtime.load = None;

                self.sink.set_current_url(resp.url.clone());

                self.context.set_raw_html(String::from_utf8_lossy(resp.body.as_slice()).as_ref());

                self.send_event(EngineEvent::Load {
                    tab_id: self.tab_id,
                    event: LoadEvent::Finished {
                        nav_id: completed_nav,
                        url: resp.url.to_string(),
                        bytes: resp.body.len() as u64,
                        content_type: resp.headers
                            .get("content_type")
                            .and_then(|v| v.to_str().ok())
                            .map(|s| s.to_string()),
                    },
                });

                self.send_event(EngineEvent::Navigation {
                    tab_id: self.tab_id,
                    event: NavigationEvent::Finished {
                        nav_id: completed_nav,
                        url: resp.url.to_string(),
                    }
                });
            }
            Ok((completed_nav, Err(e))) => {
                if completed_nav != current_nav {
                    return;
                }

                self.state = TabState::Failed(format!("Tab {:?} error: {}", self.tab_id, e));
                self.is_loading = false;
                self.is_error = true;
                self.runtime.dirty = true;
                self.runtime.load = None;

                self.send_event(EngineEvent::Load {
                    tab_id: self.tab_id,
                    event: LoadEvent::Failed {
                        nav_id: Some(completed_nav), url: "".into(), error: e.to_string()
                    },
                });
                self.send_event(EngineEvent::Navigation {
                    tab_id: self.tab_id,
                    event: NavigationEvent::Failed {
                        nav_id: Some(completed_nav), url: "".into(), error: e.to_string()
                    },
                });
            }
            Err(_) => {
                self.runtime.load = None;

                self.send_event(EngineEvent::Load {
                    tab_id: self.tab_id,
                    event: LoadEvent::Cancelled { nav_id: current_nav, url: "".into(), reason: CancelReason::ExplicitCancel },
                });
                self.send_event(EngineEvent::Navigation {
                    tab_id: self.tab_id,
                    event: NavigationEvent::Cancelled { nav_id: current_nav, url: "".into(), reason: CancelReason::ExplicitCancel },
                });
            }
        }
    }

    fn handle_tab_command(&mut self, cmd: TabCommand) -> ControlFlow {
        use ControlFlow::*;
        match cmd {
            TabCommand::CloseTab => {
                println!("Tab {:?} received Close command, exiting", self.tab_id);
                Break
            }
            TabCommand::Navigate { url } => {
                self.navigate_to(&url, false);
                Continue
            }
            TabCommand::Reload { ignore_cache } => {
                let url = self.current_url.as_ref().map(|u| u.as_str()).unwrap_or("about:blank").to_string();
                self.navigate_to(url.as_str(), ignore_cache);
                Continue
            }
            TabCommand::SetViewport { x, y, width, height } => {
                self.set_viewport(Viewport::new(x, y, width, height));
                self.runtime.dirty = true;
                Continue
            }
            TabCommand::MouseMove { .. } |
            TabCommand::MouseDown { .. } |
            TabCommand::MouseUp { .. } |
            TabCommand::KeyDown { .. } |
            TabCommand::KeyUp { .. } |
            TabCommand::CharInput { .. } => {
                self.runtime.dirty = true;
                Continue
            }
            TabCommand::ResumeDrawing { fps: wanted_fps } => {
                self.runtime.drawing_enabled = true;
                self.runtime.fps = wanted_fps.max(1) as u32;
                let period = std::time::Duration::from_secs_f64(1.0 / (self.runtime.fps as f64));
                self.runtime.interval = tokio::time::interval(period);
                self.runtime.interval.set_missed_tick_behavior(MissedTickBehavior::Delay);
                self.runtime.dirty = true;
                Continue
            }
            TabCommand::SuspendDrawing => {
                self.runtime.drawing_enabled = false;
                Continue
            }
            _ => {
                // Keep your other commands here
                Continue
            }
        }
    }

    fn send_event(&self, evt: EngineEvent) {
        match self.zone_context.event_tx.send(evt) {
            Ok(_) => {}
            Err(e) => {
                log::error!("Error sending event: {}", e);
            }
        }
    }

    fn navigate_to(&mut self, url: impl Into<String>, ignore_cache: bool) {
        // Cancel any in-flight load
        if let Some(load) = self.runtime.load.take() {
            load.cancel.cancel();
        }

        // Convert the URL string into an actual URL
        let unvalidated_url = url.into();
        let real_url = match Url::parse(&unvalidated_url) {
            Ok(u) => u,
            Err(e) => {
                log::error!("Tab[{:?}]: Cannot parse URL: {}", self.tab_id, e);
                self.send_event(EngineEvent::Navigation {
                    tab_id: self.tab_id,
                    event: NavigationEvent::Failed {
                        nav_id: None,            // no nav_id could be created
                        url: unvalidated_url,
                        error: format!("Cannot parse URL: {e}"),
                    },
                });
                return;
            }
        };

        // Prepare storage for the URL
        if let Err(e) = self.prepare_storage_for(&real_url) {
            self.send_event(EngineEvent::Navigation {
                tab_id: self.tab_id,
                event: NavigationEvent::Failed {
                    nav_id: None,
                    url: real_url.to_string(),
                    error: e.to_string(),
                },
            });
            return;
        }

        // Create new navigation ID for this navigation request
        let nav_id = NavigationId::new();
        self.sink.set_nav(nav_id);

        self.pending_url = Some(real_url.clone());
        self.is_loading = true;
        self.is_error = false;
        self.state = TabState::Loading;
        self.runtime.dirty = true;

        self.send_event(EngineEvent::Navigation {
            tab_id: self.tab_id,
            event: NavigationEvent::Started { nav_id, url: real_url.to_string() }
        });
        self.send_event(EngineEvent::Load {
            tab_id: self.tab_id,
            event: LoadEvent::Started { nav_id, url: real_url.to_string() }
        });


        // Setup cancellation, the response channel and spwan the load task
        let cancel = CancellationToken::new();
        let cancel_child = cancel.child_token();
        let (tx_done, rx_done) = oneshot::channel::<(NavigationId, DocumentLoadResult)>();

        let tab_id = self.tab_id;
        let event_tx = self.zone_context.event_tx.clone();
        let io_tx = self.zone_context.io_tx.clone();
        let request_url = real_url.clone();

        let span = tracing::info_span!(
            "tab_nav",
            tab_id=%tab_id,
            nav_id=%nav_id.0,
            scheme=%request_url.scheme(),
            host=%request_url.host_str().unwrap_or(""),
            path=%request_url.path(),
        );

        tokio::spawn(async move {
            let _e = span.enter();

            // Submit a streaming fetch for the main document
            let (tx_fetch, rx_fetch) = oneshot::channel::<FetchResult>();
            let req = FetchRequest {
                key: FetchKey {
                    url: request_url.clone(),
                    accept: None,
                    range: None,
                },
                priority: Priority::High,
                kind: ResourceKind::Document,
                initiator: Initiator::Navigation,
                tab_id,
                streaming: true,
                reply: Some(tx_fetch),
            };

            if io_tx.send(req).is_err() {
                // Coudln't send the request to the I/O thread
                let _ = tx_done.send((
                    nav_id,
                    DocumentLoadResult::NetworkError("I/O channel closed".into())
                ));
                return;
            }

            // Wait for fetch to complete or cancellation
            let fetch_result = select! {
                _ = cancel_child.cancelled() => {
                    let _ = tx_done.send((nav_id, DocumentLoadResult::Cancelled("Navigation cancelled".into())));
                    return;
                }
                r = rx_fetch => match r {
                    Ok(r) => r,
                    Err(_) => {
                        let _ = tx_done.send((nav_id, DocumentLoadResult::NetworkError("Fetch response channel closed".into())));
                        return;
                    }
                }
            };

            // Handle fetch result
            match fetch_result {
                FetchResult::Stream { meta, body } => {
                    let reader = StreamReader::new(
                        body.map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
                    );

                    // Do some progress reporting. TTFB. We have a Progress EngineEvent for this

                    let parse_res = parse_main_document_stream(
                        tab_id,
                        nav_id,
                        meta.final_url.clone(),
                        reader,
                        cancel_child.clone(),
                        ignore_cache,
                        event_tx.clone()
                    ).await;

                    let _ = tx_done.send((nav_id, parse_res));
                }
                FetchResult::Buffered { meta, body } => {
                    let parse_res = parse_main_document_bytes(
                        tab_id,
                        nav_id,
                        meta.final_url.clone(),
                        &body,
                        cancel_child.clone(),
                        ignore_cache,
                        event_tx.clone()
                    ).await;

                    let _ = tx_done.send((nav_id, parse_res));
                }

                FetchResult::Error(err) => {
                    let _ = tx_done.send((nav_id, DocumentLoadResult::NetworkError(format!("Fetch error: {}", err))));
                }
            }
        });

        self.runtime.load = Some(InflightLoad { nav_id, cancel, rx: rx_done });
    }

    /// Do a draw tick. This will be called based on the FPS that is requested
    async fn tick_draw(&mut self) -> anyhow::Result<()> {
        println!("tick_draw()");

        self.sink.inc_frame();

        let now = Instant::now();
        let elapsed = now - self.runtime.last_tick_draw;
        self.runtime.last_tick_draw = now;

        // Convert to FPS
        if elapsed.as_secs_f32() > 0.0 {
            let fps = 1.0 / elapsed.as_secs_f32();
            self.sink.set_fps(fps);
            println!("TickDraw: FPS: {:.2}", fps);
        };

        Ok(())
    }


    /// Set a new viewport and schedule a re-render by transitioning to [`TabState::PendingRendering`].
    pub fn set_viewport(&mut self, vp: Viewport) {
        // Already at the viewport we want, then we can skip
        if vp == self.desired_viewport {
            return;
        }
        self.desired_viewport = vp;

        if matches!(self.state, TabState::Rendering(_)) {
            // We are currently rending, so we can cancel the current rendering
            self.dirty_after_inflight = true;
        } else {
            // Start rendering with the new viewport
            self.state = TabState::PendingRendering(self.desired_viewport)
        }

        self.runtime.dirty = true;
    }

    /// Get the current snapshot image of the tab.
    pub fn thumbnail(&self) -> Option<&RgbaImage> {
        self.thumbnail.as_ref()
    }

    /// Bind local+session storage handles into the underlying browsing context.
    /// Call this after creating the tab or when the zone’s storage changes.
    pub fn bind_storage(&mut self, storage: StorageHandles) {
        self.context.bind_storage(storage.local, storage.session);
    }

    /// Dispatch a storage event to same-origin documents in this tab (placeholder).
    /// Intended for HTML5 storage event semantics.
    pub(crate) fn dispatch_storage_events(&mut self, origin: &url::Origin, include_iframes: bool, ev: &StorageEvent) {
        println!("Tab {:?} dispatch_storage_events called", self.tab_id);
        dbg!(&origin);
        dbg!(&include_iframes);
        dbg!(&ev);

        // Pseudocode stuff. need to fill in what it actually needs to do
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
    fn ensure_surface(&mut self, backend: &dyn RenderBackend, size: SurfaceSize) -> anyhow::Result<()> {
        if let Some(ref surf) = self.surface {
            if surf.size() == size {
                return Ok(());
            }
        }
        self.surface = Some(backend.create_surface(size, self.present_mode)?);
        Ok(())
    }

    fn prepare_storage_for(&mut self, url: &Url) -> anyhow::Result<()> {
        let pk = compute_partition_key(url, self.services.partition_policy);
        let origin = url.origin().clone();

        let local = self.services
            .storage
            .local_for(self.zone_id, &pk, &origin)
            .context("cannot get local storage for tab")?;

        let session = self.services
            .storage
            .session_for(self.zone_id, self.tab_id, &pk, &origin)
            .context("cannot get session storage for tab")?;

        self.bind_storage(StorageHandles { local, session });
        Ok(())
    }


    fn begin_render(&mut self, backend: &dyn RenderBackend) -> anyhow::Result<()> {
        if self.committed_viewport != self.desired_viewport {
            self.committed_viewport = self.desired_viewport;

            let surf_sz = self.committed_viewport.to_surface_size(self.dpr);
            self.ensure_surface(backend, surf_sz)?;
            self.context.set_viewport(self.committed_viewport);
        }

        Ok(())
    }

    fn end_render(&mut self) {
        if self.dirty_after_inflight {
            self.dirty_after_inflight = false;
            self.state = TabState::PendingRendering(self.desired_viewport);
            self.runtime.dirty = true;
        } else {
            self.state = TabState::Idle;
        }
    }
}

///
enum ControlFlow {
    Continue,
    Break
}

impl ControlFlow {
    fn is_break(&self) -> bool { matches!(self, ControlFlow::Break) }
}