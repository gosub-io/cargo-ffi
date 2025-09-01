use gosub_engine::{
    EngineConfig,
    GosubEngine,
    EngineError,
    cookies::DefaultCookieJar,
    events::EngineCommand,
    events::EngineEvent,
    render::Viewport,
    storage::{InMemoryLocalStore, InMemorySessionStore, StorageService},
    storage::types::PartitionPolicy,
    tab::TabHandle,
    zone::ZoneConfig,
    zone::ZoneServices,
};

use std::sync::Arc;
use tokio::sync::mpsc::{channel, Sender};
use winit::event::MouseButton;

#[tokio::main]
async fn main() -> Result<(), EngineError> {
    // ---- 1) Configure the engine -------------------------------------------------
    // Start with sane defaults; tweak as needed (JS/images, UA string, limits, etc.)
    let engine_cfg = EngineConfig::builder()
        .max_zones(5)
        .build().expect("Configuration is not valid")
    ;

    // ---- 2) Choose a rendering backend -------------------------------------------
    // Headless/“no-op” backend
    let backend = gosub_engine::render::backends::null::NullBackend::new();

    // ---- 4) Create the engine instance -------------------------------------------
    let engine = GosubEngine::new(Some(engine_cfg), Box::new(backend))?;
    let (event_tx, event_rx) = engine.create_event_channel(1024);

    // ---- 5) Create a zone (profile/container) ------------------------------------
    let zone_cfg = ZoneConfig::builder()
        .do_not_track(true)
        .accept_languages("fr-CH, fr;q=0.9, en;q=0.8, de;q=0.7, *;q=0.5")
        .build()
    ;
    // Create the services that will be connected to the zone (and its tabs)
    let zone_services = ZoneServices {
        storage: Arc::new(StorageService::new(
            Arc::new(InMemoryLocalStore::new()),
            Arc::new(InMemorySessionStore::new())
        )),
        cookie_store: None,
        cookie_jar: Some(DefaultCookieJar::new().into()),
        partition_policy: PartitionPolicy::None,
    };
    let zone_handle = engine.create_zone(Some(zone_cfg), zone_services, None, event_tx).await?;

    // ---- 6) Create a tab in that zone --------------------------------------------
    let mut tab_handle: TabHandle = engine.create_tab(&zone_handle).await?;

    // ---- 7) Drive the tab with a few commands ------------------------------------
    // Resize the tab’s viewport so the backend knows its render size
    tab_handle.send(EngineCommand::Resize(Viewport::new(0, 0, 1024, 768))).await?;

    // Navigate somewhere (your engine likely supports about:blank and/or http(s))
    tab_handle.send(EngineCommand::Navigate("about:blank".into())).await?;

    // Simulate a little user input (mouse move + click at 100,100)
    tab_handle.send(EngineCommand::MouseMove { x: 100.0, y: 100.0 }).await?;
    tab_handle.send(EngineCommand::MouseDown { x: 100.0, y: 100.0, button: MouseButton::Left }).await?;
    tab_handle.send(EngineCommand::MouseUp { x: 100.0, y: 100.0, button: MouseButton::Left }).await?;


    // ---- 8) Read and print engine events -----------------------------------------
    // In a real app you’d route these to your UI; for now, just println!.
    // Break out after we’ve seen a couple of interesting events.
    let mut seen_frames = 0usize;
    while let Some(ev) = event_rx.recv().await {
        match ev {
            EngineEvent::ZoneCreated { zone } => {
                println!("[event] ZoneCreated: {zone}");
            }
            EngineEvent::TabCreated { tab } => {
                println!("[event] TabCreated: {tab}");
            }
            EngineEvent::NavigationStarted { tab, url } => {
                println!("[event] NavigationStarted: tab={tab}, url={url}");
            }
            EngineEvent::NavigationFinished { tab, url, ok } => {
                println!("[event] NavigationFinished: tab={tab}, url={url}, ok={ok}");
            }
            EngineEvent::FrameReady { tab, .. } => {
                // With a real backend, you might get a handle/texture to present here.
                println!("[event] FrameReady for tab={tab}");
                seen_frames += 1;
                if seen_frames >= 2 {
                    break;
                }
            }
            other => {
                // Keep this to see what else your engine is emitting right now.
                println!("[event] {:?}", other);
            }
        }
    }

    println!("Done. Exiting.");
    Ok(())
}