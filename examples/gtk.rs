use gosub_engine::config::GosubEngineConfig;
use gosub_engine::event::EngineEvent;
use gosub_engine::GosubEngine;
use gosub_engine::viewport::Viewport;
use gtk4::glib::{clone, ControlFlow};
use gtk4::prelude::*;
use gtk4::{
    Application, ApplicationWindow, Box as GtkBox, Button, DrawingArea, Entry, Orientation,
};
use gtk4::glib::source::timeout_add_local;
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

fn main() {
    let app = Application::builder()
        .application_id("io.gosub.engine")
        .build();

    app.connect_activate(|app| {
        // Engine setup
        let config = GosubEngineConfig {
            viewport: Viewport::new(800, 600),
            user_agent: "GosubEngine/1.0".to_string(),
            max_groups: 4,
            tab_group_config: gosub_engine::config::TabGroupConfig { max_tabs: 5 },
        };

        let engine = Rc::new(RefCell::new(GosubEngine::new(config)));

        // Create a group and two tabs
        let group_id = engine.borrow_mut().create_group().expect("create_group failed");
        let tab1_id = engine.borrow_mut().open_tab(group_id).expect("open_tab 1 failed");
        let tab2_id = engine.borrow_mut().open_tab(group_id).expect("open_tab 2 failed");

        // Start with tab 1 visible
        let current_visible_tab_id = Rc::new(RefCell::new(tab1_id));

        // Address bar and UI
        let address_entry = Entry::new();
        address_entry.set_placeholder_text(Some("Enter URL..."));
        address_entry.set_hexpand(true);

        // Tab buttons
        let tab1_button = Button::with_label("Tab 1");
        let tab2_button = Button::with_label("Tab 2");

        // Drawing area
        let drawing_area = DrawingArea::new();
        drawing_area.set_content_width(800);
        drawing_area.set_content_height(600);

        // Handle drawing
        let engine_rc = engine.clone();
        let visible_tab_id = current_visible_tab_id.clone();
        drawing_area.set_draw_func(move |_area, cr, _w, _h| {
            let engine = engine_rc.borrow();
            if let Some(surface) = engine.get_surface(*visible_tab_id.borrow()) {
                cr.set_source_surface(&surface, 0.0, 0.0).unwrap();
                cr.paint().unwrap();
            }
        });

        // Handle address entry activation
        let engine_for_entry = engine.clone();
        let current_tab_id = current_visible_tab_id.clone();
        let drawing_area_for_entry = drawing_area.clone();
        address_entry.connect_activate(clone!(@strong engine_for_entry => move |entry| {
            let url = entry.text().to_string();
            engine_for_entry
                .borrow_mut()
                .handle_event(*current_tab_id.borrow(), EngineEvent::LoadUrl(url));
            drawing_area_for_entry.queue_draw();
        }));

        // Tab switching
        let visible_tab_for_tab1 = current_visible_tab_id.clone();
        let drawing_area_for_tab1 = drawing_area.clone();
        tab1_button.connect_clicked(move |_| {
            *visible_tab_for_tab1.borrow_mut() = tab1_id;
            drawing_area_for_tab1.queue_draw();
        });

        let visible_tab_for_tab2 = current_visible_tab_id.clone();
        let drawing_area_for_tab2 = drawing_area.clone();
        tab2_button.connect_clicked(move |_| {
            *visible_tab_for_tab2.borrow_mut() = tab2_id;
            drawing_area_for_tab2.queue_draw();
        });

        // Layout
        let tab_bar = GtkBox::new(Orientation::Horizontal, 5);
        tab_bar.append(&tab1_button);
        tab_bar.append(&tab2_button);
        tab_bar.set_spacing(5);

        let top_bar = GtkBox::new(Orientation::Horizontal, 5);
        top_bar.append(&address_entry);
        top_bar.set_spacing(5);

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

        // Tick loop
        let engine_for_tick = engine.clone();
        let drawing_area_for_tick = drawing_area.clone();
        let tab_id_for_tick = current_visible_tab_id.clone();
        timeout_add_local(Duration::from_millis(16), clone!(@strong engine_for_tick, @strong drawing_area_for_tick => move || {
            let mut engine = engine_for_tick.borrow_mut();
            let tick_results = engine.tick();
            let tab_id = *tab_id_for_tick.borrow();

            if let Some(result) = tick_results.get(&tab_id) {
                if result.page_loaded {
                    println!("Page loaded for tab {:?}", tab_id);
                }
                if result.needs_redraw {
                    drawing_area_for_tick.queue_draw();
                }
            }

            ControlFlow::Continue
        }));
    });

    app.run();
}
