use gosub_engine::config::EngineConfig;
use gosub_engine::event::EngineEvent;
use gosub_engine::GosubEngine;
use gosub_engine::viewport::Viewport;
use gtk4::glib::clone;
use gtk4::prelude::*;
use gtk4::{Application, ApplicationWindow, Box as GtkBox, Button, DrawingArea, Entry, Orientation};
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Instant;

fn main() {
    let app = Application::builder()
        .application_id("io.gosub.engine")
        .build();

    app.connect_activate(|app| {
        // Engine setup
        let config = EngineConfig {
            viewport: Viewport::new(800, 600),
            user_agent: "GosubEngine/1.0".to_string(),
            max_zones: 4,
            zone_config: gosub_engine::config::ZoneConfig { max_tabs: 10 },
        };

        let engine = Rc::new(RefCell::new(GosubEngine::new(config)));

        // Create a zone and N tabs
        let zone_id = engine.borrow_mut().create_zone().expect("zone creation failed");
        let mut tab_ids = Vec::new();
        for _ in 0..3 {
            let tab_id = engine.borrow_mut().open_tab(zone_id).expect("open_tab failed");
            tab_ids.push(tab_id);
        }
        let tab_ids = Rc::new(tab_ids);

        // Start with first tab visible
        let current_visible_tab = Rc::new(RefCell::new(tab_ids[0]));

        // Address bar and UI
        let address_entry = Entry::new();
        address_entry.set_placeholder_text(Some("Enter URL..."));
        address_entry.set_hexpand(true);

        // Drawing area
        let drawing_area = DrawingArea::new();
        drawing_area.set_content_width(800);
        drawing_area.set_content_height(600);

        // Tab buttons (dynamic)
        let tab_bar = GtkBox::new(Orientation::Horizontal, 5);
        for (idx, &tid) in tab_ids.iter().enumerate() {
            let label = format!("Tab {}", idx + 1);
            let button = Button::with_label(&label);
            let eng = engine.clone();
            let vis_tab = current_visible_tab.clone();
            let drawing_area_ref = drawing_area.clone();

            let tab_ids_for_closure = tab_ids.clone();
            button.connect_clicked(clone!(@strong eng, @strong vis_tab, @strong drawing_area_ref => move |_| {
                let mut eng_mut = eng.borrow_mut();
                let now = Instant::now();
                // Demote all tabs, promote clicked
                for &other in tab_ids_for_closure.iter() {
                    if let Some(tab) = eng_mut.get_tab_mut(other) {
                        if other == tid {
                            tab.mode = gosub_engine::tab::TabMode::Active;
                            tab.last_tick = now;
                        } else {
                            tab.mode = gosub_engine::tab::TabMode::BackgroundLive;
                        }
                    }
                }
                // Update visible tab
                *vis_tab.borrow_mut() = tid;
                // Immediate render if needed
                if let Some(res) = eng_mut.tick().get(&tid) {
                    if res.needs_redraw {
                        // eng_mut.render_tab(tid);
                        drawing_area_ref.queue_draw();
                    }
                }
            }));
            tab_bar.append(&button);
        }

        // Handle drawing
        let eng_draw = engine.clone();
        let vis_draw = current_visible_tab.clone();
        drawing_area.set_draw_func(move |_area, cr, _w, _h| {
            let eng = eng_draw.borrow();
            if let Some(surface) = eng.get_surface(*vis_draw.borrow()) {
                cr.set_source_surface(&surface, 0.0, 0.0).unwrap();
                cr.paint().unwrap();
            }
        });

        // Address entry activation
        let eng_entry = engine.clone();
        let vis_entry = current_visible_tab.clone();
        let draw_entry = drawing_area.clone();
        address_entry.connect_activate(clone!(@strong eng_entry => move |entry| {
            let url = entry.text().to_string();
            eng_entry
                .borrow_mut()
                .handle_event(*vis_entry.borrow(), EngineEvent::LoadUrl(url));
            draw_entry.queue_draw();
        }));

        // Layout
        let top_bar = GtkBox::new(Orientation::Horizontal, 5);
        top_bar.append(&address_entry);

        let layout = GtkBox::new(Orientation::Vertical, 5);
        layout.append(&tab_bar);
        layout.append(&top_bar);
        layout.append(&drawing_area);

        // Main window
        let window = ApplicationWindow::builder()
            .application(app)
            .title("Gosub Browser")
            .default_width(800)
            .default_height(600)
            .child(&layout)
            .build();

        window.present();

        // FrameClock-based tick loop
        let fc = drawing_area.frame_clock().unwrap();
        let eng_fc = engine.clone();
        let draw_fc = drawing_area.clone();
        let vis_fc = current_visible_tab.clone();
        fc.connect_update(clone!(@strong draw_fc, @strong eng_fc, @strong vis_fc => move |_clk| {
            let mut eng_mut = eng_fc.borrow_mut();
            let results = eng_mut.tick();
            let tab_id = *vis_fc.borrow();
            if let Some(res) = results.get(&tab_id) {
                if res.page_loaded {
                    println!("Page loaded for tab {:?}", tab_id);
                }
                if res.needs_redraw {
                    // eng_mut.render_tab(tab_id);
                    draw_fc.queue_draw();
                }
            }
            // clk.request_phase(gdk4::FrameClockPhase::);
            // clk.request_redraw();
        }));
    });

    app.run();
}
