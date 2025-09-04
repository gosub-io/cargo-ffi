/*
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use crate::engine::DEFAULT_CHANNEL_CAPACITY;
use crate::events::{EngineEvent, TabCommand};
use crate::render::Viewport;
use crate::tab::structs::TabState;
use crate::tab::{EffectiveTabServices, TabHandle, TabId, TabSink};
use crate::zone::ZoneContext;


pub struct TabWorker {
    /// ID of the tab
    tab_id: TabId,
    /// State of the tab
    state: TabState,
    /// Title of the tab
    title: String,
    /// Viewport
    viewport: Viewport,
}


impl TabWorker {
    pub fn new(services: EffectiveTabServices, zone_context: Arc<ZoneContext>) -> anyhow::Result<Self> {
        let (cmd_tx, cmd_rx) = mpsc::channel::<TabCommand>(DEFAULT_CHANNEL_CAPACITY);
        let tab_id = TabId::new();

        Ok(Self {
            zone_context,
            sink: Arc::new(TabSink {
                metrics: false,
            }),
            cmd_rx,
            cmd_tx,
            tab_id,
            services,
            state: TabState::Idle,
            title: "".to_string(),
            viewport: Default::default(),
        })
    }

    pub fn handle(&self) -> TabHandle {
        TabHandle {
            tab_id: self.tab_id,
            cmd_tx: self.cmd_tx.clone(),
            sink: self.sink.clone(),
        }
    }

    pub fn new_on_thread(services: EffectiveTabServices, zone_context: Arc<ZoneContext>) -> anyhow::Result<TabHandle> {
        let this = Self::new(services, zone_context)?;

        let handle = this.handle();

        tokio::spawn(this.run());

        Ok(handle)
    }

    pub async fn run(mut self) {
        println!("Worker started for tab {:?}", self.tab_id);

        // Ticker for driving periodic tasks (like rendering)
        let mut ticker = tokio::time::interval(Duration::from_millis(750));

        loop {
            tokio::select! {
                Some(cmd) = self.cmd_rx.recv() => {
                    if cmd == TabCommand::CloseTab {
                        println!("Tab {:?} received Close command, exiting", self.tab_id);
                        break;
                    }

                    self.handle_command(cmd).await;
                }
                _ = ticker.tick() => {
                    println!("Tab {:?} ticker ticked", self.tab_id);
                    self.tick().await;
                }
                else => break, // graceful shutdown
            }
        }

        println!("Worker exiting for tab {:?}", self.tab_id);
    }

    pub async fn handle_command(&mut self, cmd: TabCommand) {
        println!("Handling tab command: {:?}", cmd);
        match cmd {
            TabCommand::Navigate { url } => {
                println!("tab.run() wants to navigate to {url}");
                self.zone_context.event_tx.send(EngineEvent::LoadStarted { tab_id: self.tab_id, url: url.to_string()}).unwrap();
            }
            TabCommand::SetViewport { x, y, width, height } => {
                // set the viewport
                self.viewport = Viewport::new(x, y, width, height);
                // trigger redraw

                self.zone_context.event_tx.send(EngineEvent::TabResized { tab_id: self.tab_id, viewport: self.viewport }).unwrap();
            }
            TabCommand::SetTitle { title } => {
                // set the title
                self.title = title;

                self.zone_context.event_tx.send(EngineEvent::TabTitleChanged { tab_id: self.tab_id, title: self.title.clone() }).unwrap();
            }
            _ => {
                println!("Not yet implemented tab command: {:?}", cmd);
            }
        }
    }

    pub async fn tick(&mut self) {
        // println!("Doing a tab tick()")
    }
}


// pub struct TabWorkerHandle {
//     pub tab_id: TabId,
//     pub cmd_tx: mpsc::Sender<TabCommand>,
// }





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
                            let fut = self.context.load(url.clone(), cancel.child_token());

                            tokio::select! {
                                res = fut => {


                                }
                            }
                            // let (tx, rx) = oneshot::channel();
                            //
                            // let cancel_child = cancel.child_token();
                            // tokio::spawn(async move {
                            //     let res = load_main_document(url.clone(), cancel_child).await;
                            //     let _ = tx.send(res);
                            // });

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
    _backend: &Arc<Mutex<Box<dyn RenderBackend + Send + Sync>>>,
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

 */