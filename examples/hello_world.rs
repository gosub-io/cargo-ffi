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
        // Tick is the main driving force of the engine. It will iterate all tabs from all zones and do the necessary work.
        // The result of the tick is a map of tab_id -> TickResult, which tells us what happened in each tab during this
        // tick so we can update the UI accordingly.
        let results = engine.tick(compositor);
        for (_tab_id, tick_result) in &results {

            // If true, a page has been loaded during this tick. It may or may not be rendered yet.
            if tick_result.page_loaded {
                println!("Page has been loaded: {}", tick_result.commited_url.clone().unwrap().to_string());
            }

            // If true, the page has been rendered and can be drawn on the screen.
            if tick_result.needs_redraw {
                println!("Page is rendered and can be drawn on the screen");
            }
        }
        sleep(std::time::Duration::from_millis(100));
    }
}