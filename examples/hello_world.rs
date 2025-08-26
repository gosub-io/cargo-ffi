use std::str::FromStr;
use std::thread::sleep;
use url::Url;
use gosub_engine::render::Viewport;

fn main() -> Result<(), gosub_engine::EngineError> {
    // Null backend means that we don't actually render anything.
    let backend = gosub_engine::render::backends::null::NullBackend::new().expect("null backend cannot be created (!?)");
    let mut engine = gosub_engine::GosubEngine::new(None, Box::new(backend));

    // Create a zone (with all default settings)
    let zone_id = engine.zone_builder().create()?;

    // Open a tab in the zone
    let viewport = Viewport::new(0, 0, 800, 600);
    let tab_id = engine.open_tab_in_zone(zone_id, viewport)?;

    // Create the compositor that connects the frame rendered by the engine to your UI.
    let compositor = &mut gosub_engine::render::DefaultCompositor::new(
        || {
            println!("Callback from the compositor is called. We can now draw the frame on the screen.");
        });

    // // Send events/commands
    engine.execute_command(tab_id, gosub_engine::EngineCommand::Navigate(Url::from_str("https://example.com").expect("url")))?;

    loop {
        let results = engine.tick(compositor);
        for (_tab_id, tick_result) in &results {
            if tick_result.page_loaded {
                println!("Page has been loaded: {}", tick_result.commited_url.clone().unwrap().to_string());
            }

            if tick_result.needs_redraw {
                println!("Page is rendered and can be drawn on the screen");
            }
        }
        sleep(std::time::Duration::from_millis(100));
    }
}