use std::io::Cursor;
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
use anyhow::{anyhow, Context};
use http::Method;
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::sync::{broadcast, mpsc, oneshot};
use tokio::select;
use tokio::task::JoinHandle;
use tokio::time::MissedTickBehavior;
use tokio_util::sync::CancellationToken;
use url::Url;
use crate::net::{NavigationError, Resource, ResourceLoadResult, SharedBody};
use crate::net::types::{FetchKeyData, FetchRequest, FetchResult, FetchResultMeta, Initiator, Priority, ResourceKind};
use crate::tab::services::EffectiveTabServices;
use crate::tab::state::{InflightLoad, TabActivityMode, TabRuntime, TabState};
use tokio::time::{sleep, Duration, Instant};
use crate::engine::types::{NavigationId, RequestId};
use crate::net::loader::{Document, NavigationOutput, ResourceMeta};
use crate::net::mime::MimeKind;

#[allow(unused)]
enum InFlightState {
    WaitingMeta {
        cancel: CancellationToken,
    },
    WaitingDecision {
        meta: FetchResultMeta,
        // lease: BodyLease,
        cancel: CancellationToken,
    },
    ConsumingByUA {
        cancel: CancellationToken,
    },
    ConsumingByEngine {
        cancel: CancellationToken,
    },
    Done,
}

impl InFlightState {
    #[allow(unused)]
    fn is_done(&self) -> bool {
        matches!(self, InFlightState::Done)
    }
}

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
    #[allow(unused)]
    surface: Option<Box<dyn ErasedSurface + Send>>,
    // // Size of the surface (does not have to match viewport)
    // surface_size: SurfaceSize,
    // Present mode for the surface?
    #[allow(unused)]
    present_mode: PresentMode,
    /// Device Pixel Ratio
    #[allow(unused)]
    dpr: DevicePixelRatio,
    /// The viewport that was committed for the in-flight/last render
    #[allow(unused)]
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
            present_mode: PresentMode::Fifo,
            dpr: DevicePixelRatio(1.0),
            committed_viewport: Default::default(),
            desired_viewport: Default::default(),
            dirty_after_inflight: false,
            runtime: TabRuntime::default(),
        }
    }

    pub fn spawn_worker(self) -> anyhow::Result<JoinHandle<()>> {
        let name = format!("Tab Worker {}", self.tab_id);
        let join_handle = tokio::task::Builder::new().name(&name).spawn(self.run())?;

        Ok(join_handle)
    }

    async fn run(mut self) {
        self.sink.set_worker_started_now();

        // Announce creation
        self.send_event(EngineEvent::TabCreated {
            tab_id: self.tab_id,
            zone_id: self.zone_id,
        });

        loop {
            select! {
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
                    let load = self.runtime.load.take().expect("select! branch is guarded by is_some()");
                    load.rx.await
                },
                if self.runtime.load.is_some() => {
                    if let Some(loaded_url) = self.runtime.loaded_url.take() {
                        self.on_load_result(loaded_url, res);
                    }
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

    fn on_load_result(&mut self, url: Url, res: Result<(NavigationId, ResourceLoadResult), oneshot::error::RecvError>) {
        let Some(current) = self.runtime.load.as_ref() else {
            return;
        };
        let current_nav = current.nav_id;

        match res {
            Ok((completed_nav, Ok(resp))) => {
                if completed_nav != current_nav {
                    return
                }

                // Store any cookies found in the response into the cookie jar
                self.services.cookie_jar.write().store_response_cookies(&resp.meta.final_url, &resp.meta.headers);

                self.current_url = Some(resp.meta.final_url.clone());
                self.pending_url = None;
                self.is_loading = false;
                self.is_error = false;
                self.state = TabState::Loaded;
                self.runtime.dirty = true;
                self.runtime.load = None;

                self.sink.set_current_url(resp.meta.final_url.clone());

                // Set the document into the browsing context
                if let Resource::Html(doc) = resp.resource {
                    self.context.set_raw_html(doc.0.as_str())
                }
                // self.context.set_raw_html(String::from_utf8_lossy(resp.body.as_slice()).as_ref());

                self.send_event(EngineEvent::Load {
                    tab_id: self.tab_id,
                    event: LoadEvent::Finished {
                        nav_id: completed_nav,
                        url: resp.meta.final_url.clone(),
                        bytes: 0,
                        content_type: resp.meta.headers
                            .get("content_type")
                            .and_then(|v| v.to_str().ok())
                            .map(|s| s.to_string()),
                    },
                });

                self.send_event(EngineEvent::Navigation {
                    tab_id: self.tab_id,
                    event: NavigationEvent::Finished {
                        nav_id: completed_nav,
                        url: resp.meta.final_url.clone(),
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

                let err = Arc::new(anyhow!(e));

                self.send_event(EngineEvent::Load {
                    tab_id: self.tab_id,
                    event: LoadEvent::Failed {
                        nav_id: Some(completed_nav), url: url.clone(), error: err.clone(),
                    },
                });
                self.send_event(EngineEvent::Navigation {
                    tab_id: self.tab_id,
                    event: NavigationEvent::Failed {
                        nav_id: Some(completed_nav), url: url.clone(), error: err.clone(),
                    },
                });
            }
            Err(_) => {
                self.runtime.load = None;

                self.send_event(EngineEvent::Load {
                    tab_id: self.tab_id,
                    event: LoadEvent::Cancelled { nav_id: current_nav, url: url.clone(), reason: CancelReason::ExplicitCancel },
                });
                self.send_event(EngineEvent::Navigation {
                    tab_id: self.tab_id,
                    event: NavigationEvent::Cancelled { nav_id: current_nav, url: url.clone(), reason: CancelReason::ExplicitCancel },
                });
            }
        }
    }

    fn handle_tab_command(&mut self, cmd: TabCommand) -> ControlFlow {
        match cmd {
            TabCommand::CloseTab => {
//                println!("Tab {:?} received Close command, exiting", self.tab_id);
                ControlFlow::Break
            }
            TabCommand::Navigate { url } => {
                self.navigate_to(&url, false);
                ControlFlow::Continue
            }
            TabCommand::Reload { ignore_cache } => {
                let url = self.current_url.as_ref().map(|u| u.as_str()).unwrap_or("about:blank").to_string();
                self.navigate_to(url.as_str(), ignore_cache);
                ControlFlow::Continue
            }
            TabCommand::SetViewport { x, y, width, height } => {
                self.set_viewport(Viewport::new(x, y, width, height));
                self.runtime.dirty = true;
                ControlFlow::Continue
            }
            TabCommand::MouseMove { .. } |
            TabCommand::MouseDown { .. } |
            TabCommand::MouseUp { .. } |
            TabCommand::KeyDown { .. } |
            TabCommand::KeyUp { .. } |
            TabCommand::CharInput { .. } => {
                self.runtime.dirty = true;
                ControlFlow::Continue
            }
            TabCommand::ResumeDrawing { fps: wanted_fps } => {
                self.runtime.drawing_enabled = true;
                self.runtime.fps = wanted_fps.max(1) as u32;
                let period = Duration::from_secs_f64(1.0 / (self.runtime.fps as f64));
                self.runtime.interval = tokio::time::interval(period);
                self.runtime.interval.set_missed_tick_behavior(MissedTickBehavior::Delay);
                self.runtime.dirty = true;
                ControlFlow::Continue
            }
            TabCommand::SuspendDrawing => {
                self.runtime.drawing_enabled = false;
                ControlFlow::Continue
            }
            _ => {
                // Keep your other commands here
                ControlFlow::Continue
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
            log::warn!("**** Cancelling in-flight load for tab {:?}", self.tab_id);
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
                    event: NavigationEvent::FailedUrl { nav_id: None, url: unvalidated_url , error: Arc::new(e.into()) },
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
                    url: real_url.clone(),
                    error: Arc::new(e),
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
            event: NavigationEvent::Started { nav_id, url: real_url.clone() }
        });
        self.send_event(EngineEvent::Load {
            tab_id: self.tab_id,
            event: LoadEvent::Started { nav_id, url: real_url.clone() }
        });


        // Setup cancellation, the response channel and spawn the load task
        let cancel = CancellationToken::new();
        let cancel_child = cancel.child_token();
        let (tx_done, rx_done) = oneshot::channel::<(NavigationId, ResourceLoadResult)>();

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
                tab_id,
                nav_id,
                req_id: RequestId::new(),
                key_data: FetchKeyData {
                    url: request_url.clone(),
                    method: Method::GET,
                    headers: Default::default(),
                },
                priority: Priority::High,
                kind: ResourceKind::Document,
                initiator: Initiator::Navigation,
                streaming: true,
                reply: Some(tx_fetch),
                auto_decode: true,
                max_bytes: None,
                cancel: cancel_child.clone(),
            };

            if io_tx.send(req).is_err() {
                // Couldn't send the request to the I/O thread
                let _ = tx_done.send((
                    nav_id,
                    Err(NavigationError::NetworkError("I/O channel closed".into()))
                ));
                return;
            }

            // Wait for fetch to complete or cancellation
            let fetch_result = select! {
                _ = cancel_child.cancelled() => {
                    let _ = tx_done.send((nav_id, Err(NavigationError::Cancelled("Navigation cancelled".into()))));
                    return;
                }
                r = rx_fetch => match r {
                    Ok(r) => r,
                    Err(_) => {
                        let _ = tx_done.send((nav_id, Err(NavigationError::NetworkError("Fetch response channel closed".into()))));
                        return;
                    }
                }
            };

            // Handle fetch result
            match fetch_result {
                FetchResult::Stream { meta, peek, shared } => {
                    let reader = SharedBody::combined_reader(peek, shared);

                    // Do some progress reporting. TTFB. We have a Progress EngineEvent for this

                    let doc = parse_main_document_stream(
                        tab_id,
                        nav_id,
                        meta.final_url.clone(),
                        reader,
                        cancel_child.clone(),
                        ignore_cache,
                        event_tx.clone()
                    ).await;

                    if doc.is_err() {
                        let _ = tx_done.send((nav_id, Err(NavigationError::Other(doc.err().unwrap()))));
                        return;
                    }

                    let doc = doc.unwrap();

                    let resource_meta = ResourceMeta {
                        mime: MimeKind::Html,
                        final_url: meta.final_url.clone(),
                        content_length: Some(doc.0.len() as u64),
                        etag: None,
                        charset: None,
                        last_modified: None,
                        headers: meta.headers.clone(),
                    };

                    let _ = tx_done.send((nav_id, Ok(NavigationOutput{
                        meta: resource_meta,
                        resource: Resource::Html(doc),
                    })));
                }
                FetchResult::Buffered { meta, body } => {
                    let doc = parse_main_document_bytes(
                        tab_id,
                        nav_id,
                        meta.final_url.clone(),
                        &body,
                        ignore_cache,
                        event_tx.clone()
                    ).await;

                    if doc.is_err() {
                        let _ = tx_done.send((nav_id, Err(NavigationError::Other(doc.err().unwrap()))));
                        return;
                    }

                    let doc = doc.unwrap();

                    let resource_meta = ResourceMeta {
                        mime: MimeKind::Html,
                        final_url: meta.final_url.clone(),
                        content_length: Some(doc.0.len() as u64),
                        etag: None,
                        charset: None,
                        last_modified: None,
                        headers: meta.headers.clone(),
                    };

                    let _ = tx_done.send((nav_id, Ok(NavigationOutput{
                        meta: resource_meta,
                        resource: Resource::Html(Document(String::from_utf8_lossy(body.as_ref()).to_string())),
                    })));
                }
                FetchResult::Error(err) => {
                    let _ = tx_done.send((nav_id, Err(NavigationError::NetworkError(format!("Fetch error: {}", err)))));
                }
                FetchResult::DownloadStarted { .. } => {}
                FetchResult::OpenExternal { .. } => {}
                FetchResult::Cancelled => {}
            }
        });

        self.runtime.load = Some(InflightLoad { nav_id, cancel: cancel.clone(), rx: rx_done });
    }

    /// Do a draw tick. This will be called based on the FPS that is requested
    async fn tick_draw(&mut self) -> anyhow::Result<()> {
//        println!("tick_draw()");

        self.sink.inc_frame();

        let now = std::time::Instant::now();
        let elapsed = now - self.runtime.last_tick_draw;
        self.runtime.last_tick_draw = now;

        // Convert to FPS
        if elapsed.as_secs_f32() > 0.0 {
            let fps = 1.0 / elapsed.as_secs_f32();
            self.sink.set_fps(fps);
//            println!("TickDraw: FPS: {:.2}", fps);
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
    #[allow(unused)]
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
    #[allow(unused)]
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

    #[allow(unused)]
    fn begin_render(&mut self, backend: &dyn RenderBackend) -> anyhow::Result<()> {
        if self.committed_viewport != self.desired_viewport {
            self.committed_viewport = self.desired_viewport;

            let surf_sz = self.committed_viewport.to_surface_size(self.dpr);
            self.ensure_surface(backend, surf_sz)?;
            self.context.set_viewport(self.committed_viewport);
        }

        Ok(())
    }

    #[allow(unused)]
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

// const PEEK_BUF_SIZE: usize = 5 * 1024;     // 5KB peek buffer

const PROGRESS_BYTES_STEP: usize = 32 * 1024;       // Emit events after every 32KB received

const IDLE_TIMEOUT: Duration = Duration::from_secs(5);



async fn parse_main_document_stream<R>(
    tab_id: TabId,
    nav_id: NavigationId,
    final_url: Url,
    mut reader: R,
    cancel_token: CancellationToken,
    _ignore_cache: bool,
    event_tx: broadcast::Sender<EngineEvent>,
) -> anyhow::Result<Document>
where
    R: AsyncRead + Unpin + Send + 'static,
{
    let time_start = Instant::now();

    let mut buf = vec![0u8; 16 * 1024];     // 16K buffer
    let mut total: usize = 0;
    let mut last_progress_total: usize = 0;
    let idle = sleep(IDLE_TIMEOUT);
    tokio::pin!(idle);

    let mut document_buffer = Vec::new();

    loop {
        select! {
            _ = cancel_token.cancelled() => {
                return Err(NavigationError::Cancelled("stream cancelled".into()).into());
            }
            _ = &mut idle => {
                return Err(NavigationError::Timeout("stream timeout".into()).into());
            }
            read_res = reader.read(&mut buf) => {
                let n = match read_res {
                    Ok(n) => n,
                    Err(e) => return Err(e.into()),
                };

                if n == 0 {
                    if total != last_progress_total {
                        let _ = event_tx.send(EngineEvent::Load{ tab_id, event: LoadEvent::Progress {
                            nav_id,
                            url: final_url.clone(),
                            finished: false,
                            ttfb: false,
                            bytes_received: total as u64,
                            elapsed: time_start.elapsed(),
                        }});

                        // last_progress_total = total;
                    }
                    break;
                }

                idle.as_mut().reset(Instant::now() + IDLE_TIMEOUT);

                if total == 0 {
                    let _ = event_tx.send(EngineEvent::Load { tab_id, event: LoadEvent::Progress {
                        nav_id,
                        url: final_url.clone(),
                        finished: false,
                        bytes_received: 0,
                        ttfb: true,
                        elapsed: time_start.elapsed(),
                    }});
                }

                document_buffer.extend_from_slice(&buf[..n]);
                total += n;

                // @TODO: here we can feed our HTML5 parser / bytestream with more bytes


                let bytes_since = total - last_progress_total;
                if bytes_since >= PROGRESS_BYTES_STEP {
                    last_progress_total = total;

                    let _ = event_tx.send(EngineEvent::Load{tab_id, event: LoadEvent::Progress {
                        nav_id,
                        url: final_url.clone(),
                        finished: false,
                        ttfb: false,
                        bytes_received: total as u64,
                        elapsed: time_start.elapsed(),
                    }});
                }
            }
        }
    }

    // Parser should finish up and return a document

    let _ = event_tx.send(EngineEvent::Load{tab_id, event: LoadEvent::Progress {
        nav_id,
        url: final_url.clone(),
        ttfb: false,
        finished: true,
        bytes_received: total as u64,
        elapsed: time_start.elapsed(),
    }});

    match String::from_utf8(document_buffer) {
        Ok(s) => Ok(Document(s)),
        Err(e) => Err(NavigationError::Other(anyhow!("invalid utf8: {e}")).into())
    }
}

// async fn handle_stream(
//     tab_id: TabId,
//     nav_id: NavigationId,
//     meta: &ContentMeta,
//     shared: Arc<SharedBody>,
//     cancel: CancellationToken,
//     ignore_cache: bool,
//     event_tx: broadcast::Sender<EngineEvent>,
// ) -> ResourceLoadResult {
//     let stream = shared.subscribe_stream()
//         .map_err(|e: NetError| e.to_io());
//     let mut reader = StreamReader::new(stream);
//
//     let mut peek = vec![0u8; PEEK_BUF_SIZE];
//     let n_peek = match select! {
//         _ = cancel.cancelled() => return Err(NavigationError::Cancelled("navigation cancelled".into())),
//         r = reader.read(&mut peek) => r.map_err(|e| NavigationError::from(e))?,
//     };
//     peek.truncate(n_peek);
//
//     // Create a new reader that first reads from the peek buffer, then continues with the rest of the stream
//     let mut first = &peek[..];
//     let mut chain = tokio_util::io::StreamReader::new(
//         futures_util::stream::once(async move { Ok::<_, std::io::Error>(Bytes::copy_from_slice(first)) })
//     );
//
//     // Figure out from the content-type and/or the first PEEK_BUF_SIZE bytes what kind of type this resource is
//     let header_ct = meta.content_type.as_deref();
//     let kind = classify_mime(header_ct, Some(&peek));
//
//     match kind {
//         MimeKind::Html => {
//             let doc = parse_main_document_stream(
//                 tab_id,
//                 nav_id,
//                 meta.final_url.clone(),
//                 chain,
//                 cancel,
//                 ignore_cache,
//                 event_tx
//             ).await?;
//             Resource::Html(doc)
//         }
//         MimeKind::Json => {
//             let bytes = read_all_bounded(&mut chain, cancel.clone(), MAX_BUFFER_BYTES).await?;
//             let value: serde_json::Value = serde_json::from_slice(&bytes) {
//                 Ok(v) => v,
//                 Err(e) => return Err(NavigationError::Other(anyhow!("Invalid JSON: {e}"))),
//             };
//             Resource::Json(value).into()
//         }
//         MimeKind::Image => {
//             let bytes = read_all_bounded(&mut chain, cancel.clone(), MAX_BUFFER_BYTES).await?;
//             Resource::Image { bytes: bytes.into(), mime: meta.content_type.clone().unwrap_or_else(|| "image/*".into()) }.into()
//         }
//         MimeKind::Text => {
//             let bytes = read_all_bounded(&mut chain, cancel.clone(), MAX_BUFFER_BYTES).await?;
//
//             let s = String::from_utf8(bytes).unwrap_or_else(|e| String::from_utf8_lossy(e.as_bytes()).into_owned());
//             Resource::Text { text: s, mime: header_ct.unwrap_or("text/plain").to_string() }.into()
//         }
//         MimeKind::Binary => {
//             let bytes = read_all_bounded(&mut chain, cancel.clone(), MAX_BUFFER_BYTES).await?;
//             Resource::Binary { bytes: bytes.into(), mime: header_ct.unwrap_or("application/octet-stream").to_string() }.into()
//         }
//     }
// }

async fn parse_main_document_bytes(
    tab_id: TabId,
    nav_id: NavigationId,
    final_url: Url,
    bytes: &[u8],
    _ignore_cache: bool,
    event_tx: broadcast::Sender<EngineEvent>,
) -> anyhow::Result<Document> {
    let reader = Cursor::new(bytes.to_vec());       // @TODO: can we remove the to_vec() copy)
    let cancel = CancellationToken::new();

    tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(async move {
            parse_main_document_stream(
                tab_id,
                nav_id,
                final_url,
                reader,
                cancel,
                _ignore_cache,
                event_tx,
            ).await
        })
    })
}

#[cfg(test)]
mod tests {
    use bytes::Bytes;
    use futures_util::TryStreamExt;
    use crate::net::SharedBody;

    #[tokio::test]
    async fn shared_body_streamreader_eof() {
        use tokio_util::io::StreamReader;
        use tokio::io::AsyncReadExt;
        use std::io;

        let sb = SharedBody::new(16);

        // Consumer
        let mut reader = StreamReader::new(
            sb.subscribe_stream().map_err(|e| io::Error::new(io::ErrorKind::Other, e))
        );

        // Producer
        sb.push(Bytes::from_static(&[0u8; 8192]));
        sb.push(Bytes::from_static(&[0u8; 8192]));
        sb.push(Bytes::from_static(&[0u8; 8192]));
        sb.push(Bytes::from_static(&[0u8; 8192]));
        sb.push(Bytes::from_static(&[0u8; 1948]));
        sb.finish();

        // Drain all
        let mut total = 0usize;
        let mut buf = [0u8; 4096];
        loop {
            let n = reader.read(&mut buf).await.unwrap();
            if n == 0 { break; }
            total += n;
        }
        assert_eq!(total, 4*8192 + 1948);
    }
}