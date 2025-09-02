use gosub_engine::{
    EngineConfig,
    GosubEngine,
    EngineError,
    cookies::DefaultCookieJar,
    events::EngineEvent,
    render::Viewport,
    storage::{InMemoryLocalStore, InMemorySessionStore, StorageService},
    storage::types::PartitionPolicy,
    zone::ZoneConfig,
    zone::ZoneServices,
};

use std::sync::Arc;
use gosub_engine::events::{MouseButton, TabCommand};
use gosub_engine::storage::PartitionKey;
use gosub_engine::tab::{TabCacheMode, TabCookieJar, TabDefaults, TabOverrides, TabStorageScope};

#[tokio::main]
async fn main() -> Result<(), EngineError> {
    // Configure the engine through the engineconfig builder. This will setup the main runtime
    // configuration of the engine. It's possible for some values to be changed at runtime, but
    // not all of them
    let engine_cfg = EngineConfig::builder()
        .max_zones(5)
        .build().expect("Configuration is not valid")
    ;

    // Set up a render backend. In this example we use the NullBackend which does not render
    // anything.
    let backend = gosub_engine::render::backends::null::NullBackend::new().expect("null backend");

    // Instantiate the engine
    let mut engine = GosubEngine::new(Some(engine_cfg), Box::new(backend));

    // Create a channel to receive events from and to the engine
    let (event_tx, mut event_rx) = engine.create_event_channel(1024);

    // Configure a zone. This works the same way as the engine config, using a builder
    // pattern to set up the configuration before building it.
    let zone_cfg = ZoneConfig::builder()
        .do_not_track(true)
        .accept_languages("fr-CH, fr;q=0.9, en;q=0.8, de;q=0.7, *;q=0.5")
        .build().expect("ZoneConfig is not valid")
    ;

    // Create the services for this zone. These services are automatically provided to the tabs
    // created in the zone, but can be overridden on a per-tab basis if needed.
    let zone_services = ZoneServices {
        storage: Arc::new(StorageService::new(
            Arc::new(InMemoryLocalStore::new()),
            Arc::new(InMemorySessionStore::new())
        )),
        cookie_store: None,
        cookie_jar: Some(DefaultCookieJar::new().into()),
        partition_policy: PartitionPolicy::None,
    };

    // Create the zone. Note that we can define our own zone ID to keep zones deterministic
    // (like a user profile), and we give the zone handle to the event channel so we can
    // receive events related to the zone.
    let zone_handle = engine.create_zone(zone_cfg, zone_services, None, event_tx).expect("cannot create zone");

    // Next, we create a tab in the zone. For now, we don't provide anything, but we should
    // be able to provide tab-specific services (like a different cookie jar, etc).
    let def_values = TabDefaults{
        url: None,
        title: Some("New Tab".into()),
        viewport: Some(Viewport::new(0, 0, 800, 600)),
    };
    let tab_handle = zone_handle.create_tab(def_values, None).await.expect("cannot create tab");

    // From the tab handle, we can now send commands to the engine to control the tab.
    tab_handle.send(TabCommand::Resize{width: 1024, height: 768}).await?;

    // Navigate somewhere
    tab_handle.send(TabCommand::Navigate{url: "https://gosub.io".into()}).await?;

    // Simulate a little user input (mouse move + click at 100,100)
    tab_handle.send(TabCommand::MouseMove { x: 100.0, y: 100.0 }).await?;
    tab_handle.send(TabCommand::MouseDown { x: 100.0, y: 100.0, button: MouseButton::Left }).await?;
    tab_handle.send(TabCommand::MouseUp { x: 100.0, y: 100.0, button: MouseButton::Left }).await?;


    // Open a private tab inside the zone. Note that we override some of the tab options to
    // make it private (ephemeral storage, cookie jar, cache, etc). We also set an initial URL that
    // is automatically loaded when the tab is created.
    let def_values = TabDefaults{
        url: None,
        title: Some("New Private Tab".into()),
        viewport: Some(Viewport::new(0, 0, 800, 600)),
    };

    let _private_tab_handle = zone_handle.create_tab(def_values, Some(TabOverrides {
        partition_key: Some(PartitionKey::random()),
        cookie_jar: TabCookieJar::Ephemeral,
        storage_scope: TabStorageScope::Ephemeral,
        cache_mode: TabCacheMode::Ephemeral,
        persist_history: Some(false),
        persist_downloads: Some(false),
        ..Default::default()
    })).await.expect("cannot create tab");


    // This is the application's main loop, where we receive events from the engine and
    // act on them. In a real application, you would probably want to run this in
    // a separate task/thread, and not block the main thread.
    let mut seen_frames = 0usize;
    while let Some(ev) = event_rx.recv().await {
        match ev {
            // EngineEvent::ZoneCreated { zone } => {
            //     println!("[event] ZoneCreated: {zone}");
            // }
            EngineEvent::TabCreated { tab_id, .. } => {
                println!("[event] TabCreated: {tab_id:?}");
            }
            EngineEvent::LoadStarted { tab_id, url } => {
                println!("[event] NavigationStarted: tab={tab_id:?}, url={url}");
            }
            EngineEvent::LoadFinished { tab_id, url } => {
                println!("[event] NavigationFinished: tab={tab_id:?}, url={url}");
            }
            EngineEvent::Redraw { tab_id, .. } => {
                // With a real backend, you might get a handle/texture to present here.
                println!("[event] FrameReady for tab={tab_id:?}");
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