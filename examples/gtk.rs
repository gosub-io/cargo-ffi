use gosub_engine::config::GosubEngineConfig;
use gosub_engine::GosubEngine;
use gosub_engine::viewport::Viewport;
use std::cell::RefCell;
use std::rc::Rc;
use gtk4::prelude::*;
use gtk4::{Application, ApplicationWindow, DrawingArea};
use gtk4::glib::{clone, ControlFlow};
use gtk4::glib::source::timeout_add_local;
use std::time::Duration;

fn main() {
    let app = Application::builder()
        .application_id("io.gosub.engine")
        .build();

    app.connect_activate(|app| {
        let config = GosubEngineConfig {
            viewport: Viewport::new(800, 600),
            user_agent: "GosubEngine/1.0".to_string(),
            max_groups: 4,
            tab_group_config: gosub_engine::config::TabGroupConfig {
                max_tabs: 5,  // Default max tabs per group
            },
        };

        let engine = Rc::new(RefCell::new(GosubEngine::new(config)));

        let group_id = engine
            .borrow_mut()
            .create_group()
            .expect("create_group failed");
        let tab_id = engine
            .borrow_mut()
            .open_tab(group_id)
            .expect("open_tab failed");

        let current_visible_tab_id = tab_id;

        // Should open_tab already load the URL? If so, we don't need to trigger it manually.
        engine.borrow_mut().handle_event(
            tab_id,
            gosub_engine::event::EngineEvent::LoadUrl("https://gosub.io".to_string()),
        );

        let drawing_area = DrawingArea::new();
        drawing_area.set_content_width(800);
        drawing_area.set_content_height(600);

        let engine_rc = engine.clone();
        drawing_area.set_draw_func(move |_area, cr, _w, _h| {
            // Paint the surface that has been rendered by the gosub engine instance onto the GTK drawing area
            let engine = engine_rc.borrow();
            if let Some(surface) = engine.get_surface(current_visible_tab_id) {
                cr.set_source_surface(&surface, 0.0, 0.0).unwrap();
                cr.paint().unwrap();
            }
        });

        let window = ApplicationWindow::builder()
            .application(app)
            .title("Gosub Engine GTK Example")
            .default_width(800)
            .default_height(600)
            .child(&drawing_area)
            .build();

        window.present();

        let engine_for_tick = engine.clone();
        let drawing_area_for_tick = drawing_area.clone();
        let tab_id_for_tick = tab_id;

        timeout_add_local(Duration::from_millis(16), clone!(@strong engine_for_tick, @strong drawing_area_for_tick => move || {
            let mut engine = engine_for_tick.borrow_mut();
            let tick_results = engine.tick();

            if let Some(result) = tick_results.get(&tab_id_for_tick) {
                if result.page_loaded {
                    println!("Page loaded successfully for tab {:?}", &tab_id_for_tick);
                }

                // If we have new things to paint onto the screen, and we are viewing this tab on screen, we can render that
                if result.needs_redraw && tab_id_for_tick == current_visible_tab_id {
                    drawing_area_for_tick.queue_draw();
                }
            }

            ControlFlow::Continue
        }));
    });

    app.run();
}