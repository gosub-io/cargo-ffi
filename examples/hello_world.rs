use gosub_engine::{
    cookies::DefaultCookieJar,
    events::EngineEvent,
    render::Viewport,
    storage::{InMemoryLocalStore, InMemorySessionStore, PartitionPolicy, StorageService},
    zone::ZoneConfig,
    zone::ZoneServices,
    EngineConfig, EngineError, GosubEngine,
};

use gosub_engine::events::{MouseButton, TabCommand};
use gosub_engine::storage::PartitionKey;
use gosub_engine::tab::{TabCacheMode, TabCookieJar, TabDefaults, TabOverrides, TabStorageScope};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

#[tokio::main]
async fn main() -> Result<(), EngineError> {
    console_subscriber::init();

    // Configure the engine through the engine config builder. This will set up the main runtime
    // configuration of the engine. It's possible for some values to be changed at runtime, but
    // not all of them
    let engine_cfg = EngineConfig::builder()
        .max_zones(5)
        .build()
        .expect("Configuration is not valid");

    // Set up a render backend. In this example we use the NullBackend which does not render
    // anything.
    let backend = gosub_engine::render::backends::null::NullBackend::new().expect("null backend");

    // Instantiate and start the engine
    let mut engine = GosubEngine::new(Some(engine_cfg), Box::new(backend));
    let engine_join_handle = engine.start().expect("cannot start engine");

    // Get our event channel to receive events from the engine. Note that you will only receive events
    // send from this point on.
    let mut event_rx = engine.subscribe_events();

    // Configure a zone. This works the same way as the engine config, using a builder
    // pattern to set up the configuration before building it.
    let zone_cfg = ZoneConfig::builder()
        .do_not_track(true)
        .accept_languages("fr-CH, fr;q=0.9, en;q=0.8, de;q=0.7, *;q=0.5")
        .build()
        .expect("ZoneConfig is not valid");

    // Create the services for this zone. These services are automatically provided to the tabs
    // created in the zone, but can be overridden on a per-tab basis if needed.
    let zone_services = ZoneServices {
        storage: Arc::new(StorageService::new(
            Arc::new(InMemoryLocalStore::new()),
            Arc::new(InMemorySessionStore::new()),
        )),
        cookie_store: None,
        cookie_jar: Some(DefaultCookieJar::new().into()),
        partition_policy: PartitionPolicy::None,
    };

    // Create the zone. Note that we can define our own zone ID to keep zones deterministic
    // (like a user profile), and we give the zone handle to the event channel so we can
    // receive events related to the zone.
    let mut zone = engine.create_zone(zone_cfg, zone_services, None)?;

    // Next, we create a tab in the zone. For now, we don't provide anything, but we should
    // be able to provide tab-specific services (like a different cookie jar, etc.)
    let def_values = TabDefaults {
        url: None,
        title: Some("New Tab".into()),
        viewport: Some(Viewport::new(0, 0, 800, 600)),
    };
    let tab = zone
        .create_tab(def_values, None)
        .await
        .expect("cannot create tab");

    // From the tab handle, we can now send commands to the engine to control the tab.
    tab.send(TabCommand::Resize {
        width: 1024,
        height: 768,
    })
    .await?;

    // Navigate somewhere
    tab.send(TabCommand::Navigate {
        url: "https://gosub.io".into(),
    })
    .await?;

    // Simulate a little user input (mouse move + click at 100,100)
    tab.send(TabCommand::MouseMove { x: 100.0, y: 100.0 })
        .await?;
    tab.send(TabCommand::MouseDown {
        x: 100.0,
        y: 100.0,
        button: MouseButton::Left,
    })
    .await?;
    tab.send(TabCommand::MouseUp {
        x: 100.0,
        y: 100.0,
        button: MouseButton::Left,
    })
    .await?;

    // We can set metadata on the zone like this
    zone.set_title("My first Zone");
    zone.set_description("This is the new description");
    zone.set_color([255, 128, 64, 255]);

    // Open a private tab inside the zone. Note that we override some of the tab options to
    // make it private (ephemeral storage, cookie jar, cache, etc). We also set an initial URL that
    // is automatically loaded when the tab is created.
    let def_values = TabDefaults {
        url: None,
        title: Some("New Private Tab".into()),
        viewport: Some(Viewport::new(0, 0, 800, 600)),
    };

    let private_tab_handle = zone
        .create_tab(
            def_values,
            Some(TabOverrides {
                partition_key: Some(PartitionKey::random()),
                cookie_jar: TabCookieJar::Ephemeral,
                storage_scope: TabStorageScope::Ephemeral,
                cache_mode: TabCacheMode::Ephemeral,
                persist_history: Some(false),
                persist_downloads: Some(false),
                ..Default::default()
            }),
        )
        .await
        .expect("cannot create tab");

    tokio::spawn(async move {
        _ = private_tab_handle
            .send(TabCommand::ResumeDrawing { fps: 10 })
            .await;
        sleep(Duration::from_secs(5)).await;
        _ = private_tab_handle.send(TabCommand::SuspendDrawing).await;
    });

    // This is the application's main loop, where we receive events from the engine and
    // act on them. In a real application, you would probably want to run this in
    // a separate task/thread, and not block the main thread.

    let mut seen_intervals = 0usize;
    let mut seen_frames = 0usize;
    let mut interval = tokio::time::interval(std::time::Duration::from_secs(1));

    loop {
        tokio::select! {
            Ok(ev) = event_rx.recv() => {
                println!("Received event: {:?}", ev);

                // Just count the frames we see for now
                if matches!(ev, EngineEvent::Redraw { .. }) {
                    seen_frames += 1;
                    println!("Total frames seen so far: {seen_frames}");
                }
                handle_event(ev);
            }
            _ = tokio::signal::ctrl_c() => {
                println!("Received Ctrl-C, shutting down...");
                break;
            }
            _ = interval.tick() => {
                println!("Ticking the UA interval");

                seen_intervals += 1;
                if seen_intervals >= 5 {
                    println!("Seen {seen_intervals} intervals, exiting main loop");
                    break;
                }
            }
        }
    }

    println!("Shutting down engine...");
    engine.shutdown().await?;

    // Wait for the engine task to finish
    if let Some(handle) = engine_join_handle {
        if let Err(join_err) = handle.await {
            eprintln!("engine task panicked: {join_err}");
        }
    }

    println!("Done. Exiting.");
    Ok(())
}

fn handle_event(ev: EngineEvent) {
    match ev {
        EngineEvent::TabCreated { tab_id, .. } => {
            // let tab = self.tabs.get(&tab_id).expect("Unknown tab");
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
        }
        other => {
            // Keep this to see what else your engine is emitting right now.
            println!("[event] {:?}", other);
        }
    }
}
