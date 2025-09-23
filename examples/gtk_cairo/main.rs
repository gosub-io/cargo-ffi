use crate::compositor::GtkCompositor;
use crate::tiling::{
    close_leaf, collect_leaves, compute_layout, find_leaf_at, split_leaf_into_cols, split_leaf_into_rows, LayoutHandle,
    LayoutNode, Rect,
};
use gosub_engine::cookies::SqliteCookieStore;
use gosub_engine::render::backend::ExternalHandle;
use gosub_engine::render::Viewport;
use gosub_engine::storage::{InMemorySessionStore, PartitionPolicy, SqliteLocalStore, StorageService};
use gosub_engine::zone::{ZoneConfig, ZoneId, ZoneServices};
use gosub_engine::GosubEngine;
use gtk4::glib::clone;
use gtk4::prelude::*;
use gtk4::GestureClick;
use gtk4::{
    glib, Application, ApplicationWindow, Box as GtkBox, Button, DrawingArea, Entry, EventControllerMotion, Orientation,
};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;
use url::Url;
use uuid::uuid;
use gosub_engine::events::TabCommand;
use gosub_engine::tab::{TabDefaults, TabHandle, TabId};
use once_cell::sync::Lazy;
use tokio::runtime::{Builder, Runtime};

mod compositor;
mod tiling;

const DEFAULT_MAIN_ZONE: uuid::Uuid = uuid!("95d9c701-5f1b-43ea-ba7e-bc509ee8aa54");

// Global Tokio runtime for the whole GTK app
static TOKIO_RT: Lazy<Runtime> = Lazy::new(|| {
    Builder::new_multi_thread()
        .enable_io()
        .enable_time()
        .thread_name("gosub-rt")
        .build()
        .expect("init tokio runtime")
});

// fn current_url_for_tab(eng: &GosubEngine, tab_id: gosub_engine::tab::TabId) -> Option<Url> {
//     eng.get_tab(tab_id)
//         .unwrap()
//         .lock()
//         .unwrap()
//         .current_url
//         .clone()
// }

fn main() {
    let app = Application::builder()
        .application_id("io.gosub.engine")
        .build();

    app.connect_activate(move |app| {

        // Enter Tokio so any internal tokio::spawn/time/io in engine works
        let _rt_guard = TOKIO_RT.enter();

        // Start the engine
        let backend = gosub_engine::render::backends::cairo::CairoBackend::new();
        let mut engine = GosubEngine::new(None, Box::new(backend));
        let _engine_join_handle = engine.start().expect("engine start failed");
        let mut event_rx = engine.subscribe_events();

        // Setup zone
        let zone_cfg = ZoneConfig::builder()
            .do_not_track(true)
            .accept_languages("fr-CH, fr;q=0.9, en;q=0.8, de;q=0.7, *;q=0.5")
            .build()
            .expect("ZoneConfig is not valid");

        let sqlite_store = SqliteCookieStore::new(".gosub-gtk-cookie-store.db".into()); // Arc<SqliteCookieStore>
        let cookie_store: gosub_engine::cookies::CookieStoreHandle = sqlite_store.into();

        let zone_services = ZoneServices {
            storage: Arc::new(StorageService::new(
                Arc::new(SqliteLocalStore::new(".gosub-gtk-local-storage.db").unwrap()),
                Arc::new(InMemorySessionStore::new()),
            )),
            cookie_store: Some(cookie_store),
            cookie_jar: None,
            partition_policy: PartitionPolicy::None,
        };

        let zone = Rc::new(RefCell::new(
            engine.create_zone(zone_cfg, zone_services, Some(ZoneId::from(DEFAULT_MAIN_ZONE)))
                .expect("create_zone failed")
        ));

        let tabs: Rc<RefCell<HashMap<TabId, TabHandle>>> = Rc::new(RefCell::new(HashMap::new()));

        let tab = TOKIO_RT.block_on(async {
            zone.borrow_mut().create_tab(TabDefaults {
                url: None,
                title: Some("New Tab".to_string()),
                viewport: Some(Viewport::new(0, 0, 800, 600)),
            }, None).await
        }).expect("create_tab failed");
        let tab_id = tab.tab_id;
        tabs.borrow_mut().insert(tab_id, tab);

        // Active tab id + last size (single source of truth)
        let active_tab: Rc<RefCell<TabId>> = Rc::new(RefCell::new(tab_id));
        let last_size: Rc<RefCell<(i32, i32)>> = Rc::new(RefCell::new((800, 600)));

        // Tiling tree stores TabId, not TabHandle
        let root: LayoutHandle = Rc::new(RefCell::new(LayoutNode::Leaf(*active_tab.borrow())));

        let address_entry = Entry::new();
        address_entry.set_placeholder_text(Some("Enter URL for active pane..."));
        address_entry.set_hexpand(true);

        let drawing_area = DrawingArea::new();
        drawing_area.set_content_width(800);
        drawing_area.set_content_height(600);
        drawing_area.set_focusable(true);

        // The compositor will call `queue_draw` on the drawing area after a frame is submitted (do we want this?)
        let compositor = Rc::new(RefCell::new(
            GtkCompositor::new({
                let da = drawing_area.clone();
                move || da.queue_draw()
            })
        ));

        // Toolbar: Split Col, Split Row, Close Pane
        let btn_split_col = Button::with_label("Split Col");
        let btn_split_row = Button::with_label("Split Row");
        let btn_close = Button::with_label("Close Pane");

        let btn_set_ls = Button::with_label("Set LS");
        let btn_get_ls = Button::with_label("Get LS");

        let btn_set_ss = Button::with_label("Set SS");
        let btn_get_ss = Button::with_label("Get SS");

        // let storage_debug = zone_services.storage.clone();
        // let active_ls_set = active_tab.clone();
        // btn_set_ls.connect_clicked(glib::clone!(@strong storage_debug, @strong eng_ls_set, @strong active_ls_set => move |_| {
        //     let tab_id = *active_ls_set.borrow();
        //     let Ok(eng_ref) = eng_ls_set.try_borrow() else { return; };
        //     let Some(url) = current_url_for_tab(&eng_ref, tab_id) else {
        //         eprintln!("[LS] No current URL for active tab; navigate somewhere first.");
        //         return;
        //     };
        //     let origin = url.origin();
        //
        //     // Default partition unless you're testing CHIPS; tweak as needed:
        //     let pk = gosub_engine::storage::PartitionKey::default();
        //
        //     let area = storage_debug.local_for(zone_id, &pk, &origin).unwrap();
        //     if let Err(e) = area.set_item("foo", "bar") {
        //         eprintln!("[LS] set_item error: {e}");
        //     } else {
        //         println!("[LS] set foo=bar for origin {}", url.origin().ascii_serialization());
        //     }
        // }));

        // let storage_debug = storage.clone();
        // let eng_ls_get = engine.clone();
        // let active_ls_get = active_tab.clone();
        // btn_get_ls.connect_clicked(glib::clone!(@strong storage_debug, @strong eng_ls_get, @strong active_ls_get => move |_| {
        //     let tab_id = *active_ls_get.borrow();
        //     let Ok(eng_ref) = eng_ls_get.try_borrow() else { return; };
        //     let Some(url) = current_url_for_tab(&eng_ref, tab_id) else {
        //         eprintln!("[LS] No current URL.");
        //         return;
        //     };
        //     let origin = url.origin();
        //     let pk = gosub_engine::storage::PartitionKey::default();
        //
        //     let area = storage_debug.local_for(zone_id, &pk, &origin).unwrap();
        //     match area.get_item("foo") {
        //         Some(v) => println!("[LS] get foo -> {v}"),
        //         None    => println!("[LS] key foo not set"),
        //     }
        // }));

        // // ---------- SessionStorage (per-(zone,tab,origin,partition)) ----------
        // let storage_debug = storage.clone();
        // let eng_ss_set = engine.clone();
        // let active_ss_set = active_tab.clone();
        //
        // // Button: Set SessionStorage key=foo, value=bar
        // // function: btn_set_ss.on_click
        // btn_set_ss.connect_clicked(glib::clone!(@strong storage_debug, @strong eng_ss_set, @strong active_ss_set => move |_| {
        //     let tab_id = *active_ss_set.borrow();
        //     let Ok(eng_ref) = eng_ss_set.try_borrow() else { return; };
        //     let Some(url) = current_url_for_tab(&eng_ref, tab_id) else {
        //         eprintln!("[SS] No current URL.");
        //         return;
        //     };
        //     let origin = url.origin();
        //     let pk = gosub_engine::storage::PartitionKey::default();
        //
        //     let area = storage_debug.session_for(zone_id, tab_id, &pk, &origin);
        //     if let Err(e) = area.set_item("foo", "bar") {
        //         eprintln!("[SS] set_item error: {e}");
        //     } else {
        //         println!("[SS] set foo=bar for origin {}", url.origin().ascii_serialization());
        //     }
        // }));

        // let storage_debug = storage.clone();
        // let eng_ss_get = engine.clone();
        // let active_ss_get = active_tab.clone();
        // btn_get_ss.connect_clicked(glib::clone!(@strong storage_debug, @strong eng_ss_get, @strong active_ss_get => move |_| {
        //     let tab_id = *active_ss_get.borrow();
        //     let Ok(eng_ref) = eng_ss_get.try_borrow() else { return; };
        //     let Some(url) = current_url_for_tab(&eng_ref, tab_id) else {
        //         eprintln!("[SS] No current URL.");
        //         return;
        //     };
        //     let origin = url.origin();
        //     let pk = gosub_engine::storage::PartitionKey::default();
        //
        //     let area = storage_debug.session_for(zone_id, tab_id, &pk, &origin);
        //     match area.get_item("foo") {
        //         Some(v) => println!("[SS] get foo -> {v}"),
        //         None    => println!("[SS] key foo not set"),
        //     }
        // }));

        // -----------------------------
        // Split handlers
        // -----------------------------
        let root_split = root.clone();
        let last_size_split = last_size.clone();
        let drawing_split = drawing_area.clone();
        let active_split = active_tab.clone();

        let zone_for_split = zone.clone();
        let tabs_for_split = tabs.clone();
        btn_split_col.connect_clicked(clone!(
            @strong root_split,
            @strong last_size_split,
            @strong drawing_split,
            @strong active_split,
            @strong zone_for_split,
            @strong tabs_for_split
            => move |_| {
            // Open a new tab sized like the active pane
            let (w, h) = *last_size_split.borrow();


            let new_tab = TOKIO_RT.block_on(async {
                zone_for_split.borrow_mut().create_tab(TabDefaults {
                    url: None,
                    title: Some("New Tab".to_string()),
                    viewport: Some(Viewport::new(0, 0, (w/2).max(1) as u32, h as u32)),
                }, None).await
            })
            .expect("create_tab failed");

            let new_id = new_tab.tab_id;
            tabs_for_split.borrow_mut().insert(new_id, new_tab);

            let target = *active_split.borrow();
            split_leaf_into_cols(&root_split, target, vec![new_id]);
            // Send resizes to all leaves after split
            let mut pairs = Vec::new();
            compute_layout(&root_split.borrow(), Rect { x:0, y:0, w, h }, &mut pairs);

            let tabs_ref = tabs_for_split.borrow();
            for (tab_id, r) in pairs {
                if let Some(tab) = tabs_ref.get(&tab_id) {
                    _ = tab.send(TabCommand::SetViewport { x:0, y: 0, width: r.w as u32, height: r.h as u32 });
                }
            }

            drawing_split.queue_draw();
        }));

        let root_split2 = root.clone();
        let last_size_split2 = last_size.clone();
        let drawing_split2 = drawing_area.clone();
        let active_split2 = active_tab.clone();
        let zone_for_split = zone.clone();
        let tabs_for_split = tabs.clone();
        btn_split_row.connect_clicked(clone!(@strong root_split2, @strong last_size_split2, @strong drawing_split2, @strong active_split2 => move |_| {
            let (w, h) = *last_size_split2.borrow();

            let new_tab = TOKIO_RT.block_on(async {
                zone_for_split.borrow_mut().create_tab(TabDefaults {
                    url: None,
                    title: Some("New Tab".to_string()),
                    viewport: Some(Viewport::new(0, 0, w as u32, (h/2).max(1) as u32)),
                }, None).await
            })
            .expect("create_tab failed");

            let tab_id = new_tab.tab_id;
            tabs_for_split.borrow_mut().insert(new_tab.tab_id, new_tab);

            let target = *active_split2.borrow();
            split_leaf_into_rows(&root_split2, target, vec![tab_id]);
            let mut pairs = Vec::new();
            compute_layout(&root_split2.borrow(), Rect { x:0, y:0, w, h }, &mut pairs);

            let tabs_ref = tabs_for_split.borrow();
            for (tab_id, r) in pairs {
                if let Some(tab) = tabs_ref.get(&tab_id) {
                    _ = tab.send(TabCommand::SetViewport { x:0, y: 0, width: r.w as u32, height: r.h as u32 });
                }
            }

            drawing_split2.queue_draw();
        }));

        let root_close = root.clone();
        let last_size_close = last_size.clone();
        let drawing_close = drawing_area.clone();
        let active_close = active_tab.clone();
        let tabs_for_close = tabs.clone();
        btn_close.connect_clicked(clone!(@strong root_close, @strong last_size_close, @strong drawing_close, @strong active_close => move |_| {
            let target = *active_close.borrow();
            if close_leaf(&root_close, target) {
                // Pick a new active from remaining leaves
                let mut leaves = Vec::new();
                collect_leaves(&root_close.borrow(), &mut leaves);
                if let Some(&first) = leaves.first() { *active_close.borrow_mut() = first; }
                let (w, h) = *last_size_close.borrow();
                let mut pairs = Vec::new();
                compute_layout(&root_close.borrow(), Rect { x:0, y:0, w, h }, &mut pairs);

                let tabs_ref = tabs_for_close.borrow();
                for (tab_id, r) in pairs {
                    if let Some(tab) = tabs_ref.get(&tab_id) {
                        _ = tab.send(TabCommand::SetViewport { x:0, y: 0, width: r.w as u32, height: r.h as u32 });
                    }
                }

                drawing_close.queue_draw();
            }
        }));

        // Drawing area
        let root_draw = root.clone();
        let active_draw = active_tab.clone();
        let compositor_draw = compositor.clone();
        drawing_area.set_draw_func(move |_area, cr, w, h| {
            let active_tab_id = *active_draw.borrow();

            // Compute the tab layouts and store in pairs
            let mut pairs = Vec::new();
            compute_layout(&root_draw.borrow(), Rect { x:0, y:0, w, h }, &mut pairs);

            // Iterate all the tabs and draw their surfaces
            for (tab_id, r) in &pairs {
                {
                    let mut binding = compositor_draw.borrow_mut();
                    if let Some(handle) = binding.frame_for_mut(*tab_id) {
                        match handle {
                            ExternalHandle::CpuPixelsPtr { width, height, stride, pixel_buf } => {
                                let w = *width as i32;
                                let h = *height as i32;
                                let st = *stride as i32;

                                // SAFETY: `ptr` must remain valid & mutable for `len` bytes during this paint.
                                let surface = unsafe {
                                    gtk4::cairo::ImageSurface::create_for_data_unsafe(
                                        pixel_buf.as_ptr(),
                                        gtk4::cairo::Format::ARgb32,
                                        w,
                                        h,
                                        st,
                                    ).expect("cairo surface over ptr")
                                };

                                surface.flush();

                                cr.save().unwrap();
                                cr.rectangle(r.x as f64, r.y as f64, r.w as f64, r.h as f64);
                                cr.clip();
                                cr.translate(r.x as f64, r.y as f64);

                                let sw = *width as f64;
                                let sh = *height as f64;
                                if sw > 0.0 && sh > 0.0 && (sw as i32 != r.w || sh as i32 != r.h) {
                                    cr.scale(r.w as f64 / sw, r.h as f64 / sh);
                                }
                                cr.set_source_surface(&surface, 0.0, 0.0).unwrap();
                                cr.paint().unwrap();
                                cr.restore().unwrap();
                                // `surface` drops here while `data` still points to valid pixels
                            }
                            ExternalHandle::CpuPixelsOwned { width, height, stride, pixels, .. } => {
                                let w = *width as i32;
                                let h = *height as i32;
                                let st = *stride as i32;

                                // Safe: create a surface over a &mut [u8]. Lifetime is tied to `surface`,
                                // which we drop before `pixels` goes out of scope in this arm.
                                // Use the unsafe pointer-based API to avoid `'static` borrow requirements.
                                let surface = unsafe {
                                    gtk4::cairo::ImageSurface::create_for_data_unsafe(
                                        pixels.as_mut_ptr(),
                                        gtk4::cairo::Format::ARgb32,
                                        w, h, st
                                    ).expect("cairo surface over Vec<u8> ptr")
                                };

                                surface.flush();

                                cr.save().unwrap();
                                cr.rectangle(r.x as f64, r.y as f64, r.w as f64, r.h as f64);
                                cr.clip();
                                cr.translate(r.x as f64, r.y as f64);

                                // Fit the frame into tile rect (simple scale-to-fill)
                                let sw = *width as f64;
                                let sh = *height as f64;
                                if sw > 0.0 && sh > 0.0 && (sw as i32 != r.w || sh as i32 != r.h) {
                                    cr.scale(r.w as f64 / sw, r.h as f64 / sh);
                                }

                                // If you need HiDPI: cr.scale(1.0/scale_factor, 1.0/scale_factor) before painting
                                cr.set_source_surface(&surface, 0.0, 0.0).unwrap();
                                cr.paint().unwrap();
                                cr.restore().unwrap();
                            },
                            _ => {
                                eprintln!("Unsupported handle type for tab {:?}: {:?}", tab_id, handle);
                            }
                        }
                    } else {
                        // draw placeholder
                        cr.save().unwrap();
                        cr.set_source_rgb(0.25, 0.25, 0.30);
                        cr.rectangle(r.x as f64 + 0.5, r.y as f64 + 0.5, (r.w - 1) as f64, (r.h - 1) as f64);
                        cr.fill().unwrap();
                        cr.restore().unwrap();
                    }
                }
            }

            // Draw an outline around the active pane
            for (tab_id, r) in &pairs {
                if *tab_id == active_tab_id {
                    cr.save().unwrap();
                    cr.set_source_rgba(0.2, 0.6, 1.0, 1.0);
                    cr.set_line_width(2.0);
                    cr.rectangle(r.x as f64 + 1.0, r.y as f64 + 1.0, (r.w - 2) as f64, (r.h - 2) as f64);
                    cr.stroke().unwrap();
                    cr.restore().unwrap();
                }
            }
        });

        // Resize pane
        let root_resize = root.clone();
        let last_size_resize = last_size.clone();
        let tabs_for_resize = tabs.clone();
        drawing_area.connect_resize(clone!(@strong root_resize, @strong last_size_resize, @strong tabs_for_resize => move |_area, w, h| {
            *last_size_resize.borrow_mut() = (w, h);
            let mut pairs = Vec::new();
            compute_layout(&root_resize.borrow(), Rect { x:0, y:0, w, h }, &mut pairs);

            for (tab_id, r) in pairs {
                if let Some(tab) = tabs_for_resize.borrow().get(&tab_id) {
                    _ = tab.send(TabCommand::SetViewport { x:0, y: 0, width: r.w as u32, height: r.h as u32 });
                }
            }
        }));

        // Mouse: select pane under cursor
        let root_pick = root.clone();
        let active_pick = active_tab.clone();
        let drawing_pick = drawing_area.clone();
        let click = GestureClick::new();
        let last_size_pick = last_size.clone();
        click.connect_pressed(move |_gest, _n_press, x, y| {
            let (w, h) = *last_size_pick.borrow();
            if let Some(tab_id) = find_leaf_at(&root_pick.borrow(), Rect { x:0, y:0, w, h }, x, y) {
                *active_pick.borrow_mut() = tab_id;
                drawing_pick.queue_draw();
            }
        });
        drawing_area.add_controller(click);

        // Address entry: navigate active tab
        let tabs_for_nav = tabs.clone();
        let active_for_nav = active_tab.clone();
        let draw_entry = drawing_area.clone();
        address_entry.connect_activate(clone!(@strong draw_entry => move |entry| {
            let mut s = entry.text().to_string();
            if !(s.starts_with("http://") || s.starts_with("https://")) {
                s = format!("https://{s}");
                entry.set_text(&s);
            }
            let Ok(url) = Url::parse(&s) else { return; };

            if let Some(tab) = tabs_for_nav.borrow().get(&*active_for_nav.borrow()) {
                _ = tab.send(TabCommand::Navigate { url: url.to_string() });
            }
            draw_entry.queue_draw();
        }));

        let last_pointer = Rc::new(RefCell::new((0.0_f64, 0.0_f64)));
        let motion = EventControllerMotion::new();
        {
            let last_pointer_m = last_pointer.clone();
            motion.connect_motion(move |_m, x, y| {
                *last_pointer_m.borrow_mut() = (x, y);
            });
        }
        drawing_area.add_controller(motion);

        // // Scroll pane
        // let root_scroll = root.clone();
        // let last_size_scroll = last_size.clone();
        // let drawing_scroll = drawing_area.clone();
        // let last_pointer_scroll = last_pointer.clone();
        //
        // let scroll = EventControllerScroll::new(EventControllerScrollFlags::BOTH_AXES);
        // scroll.connect_scroll(clone!(@strong eng_scroll, @strong root_scroll, @strong last_size_scroll, @strong drawing_scroll, @strong last_pointer_scroll => move |_ctrl, dx, dy| {
        //     // Where is the pointer?
        //     let (px, py) = *last_pointer_scroll.borrow();
        //
        //     // Which pane is under the pointer?
        //     let (w, h) = *last_size_scroll.borrow();
        //     if let Some(tab_id) = find_leaf_at(&root_scroll.borrow(), Rect { x:0, y:0, w, h }, px, py) {
        //         let line_h = 20.0_f64;
        //         let dx_px = (dx * line_h) as f32;
        //         let dy_px = (dy * line_h) as f32;
        //
        //         // Send to the engine (you implement what Scroll does per tab)
        //         let _ = eng_scroll.borrow_mut().handle_event(tab_id, EngineEvent::Scroll { dx: dx_px, dy: dy_px });
        //
        //         // Ask GTK to redraw
        //         drawing_scroll.queue_draw();
        //     }
        //
        //     return glib::Propagation::Proceed;
        // }));
        // drawing_area.add_controller(scroll);

        // Layout boxes
        let toolbar = GtkBox::new(Orientation::Horizontal, 6);
        toolbar.append(&btn_split_col);
        toolbar.append(&btn_split_row);
        toolbar.append(&btn_close);

        toolbar.append(&btn_set_ls);
        toolbar.append(&btn_get_ls);
        toolbar.append(&btn_set_ss);
        toolbar.append(&btn_get_ss);

        let url_bar = GtkBox::new(Orientation::Horizontal, 1);
        url_bar.append(&address_entry);

        let vbox = GtkBox::new(Orientation::Vertical, 6);
        vbox.append(&url_bar);
        vbox.append(&toolbar);
        vbox.append(&drawing_area);

        // Window
        let window = ApplicationWindow::builder()
            .application(app)
            .title("Gosub Browser – Tiled")
            .default_width(800)
            .default_height(600)
            .child(&vbox)
            .build();

        window.present();

        // // FrameClock tick: redraw if any visible tab needs it
        // let fc = drawing_area.frame_clock().unwrap();
        // let root_fc = root.clone();
        // let drawing_fc = drawing_area.clone();
        // let last_size_fc = last_size.clone();
        // let compositor_fc = compositor.clone();
        // fc.connect_update(clone!(@strong drawing_fc, @strong eng_fc, @strong root_fc => move |_clk| {
        //
        //     // let mut eng_mut = eng_fc.borrow_mut();
        //     // let results = eng_mut.tick(&mut *compositor_fc.borrow_mut());
        //     //
        //     // // If any leaf needs redraw, repaint
        //     // let (w, h) = *last_size_fc.borrow();
        //     // let mut pairs = Vec::new();
        //     // compute_layout(&root_fc.borrow(), Rect { x:0, y:0, w, h }, &mut pairs);
        //     //
        //     // let mut redraw = false;
        //     // for (tab_id, _r) in pairs {
        //     //     if let Some(res) = results.get(&tab_id) {
        //     //         if res.page_loaded {
        //     //             println!("Tab {:?} page loaded", tab_id);
        //     //         }
        //     //         if res.needs_redraw {
        //     //             println!("Tab {:?} needs redraw", tab_id);
        //     //             redraw = true;
        //     //         }
        //     //     }
        //     // }
        //     //
        //     // if redraw {
        //     //     println!("Requesting redraw in frame tick");
        //     //     drawing_fc.queue_draw();
        //     // }
        // }));


        // This will spawn a task in the GTK and in the tokio. If something is received in
        // tokio, it will send a message to the GTK thread, which will then request a redraw.
        use tokio::sync::mpsc;
        let (tx_ui, mut rx_ui) = mpsc::unbounded_channel::<()>();

        TOKIO_RT.spawn({
            let mut event_rx_tokio = event_rx;
            async move {
                while event_rx_tokio.recv().await.is_ok() {
                    let _ = tx_ui.send(());
                }
            }
        });
        // In GTK thread: on ping, request redraw
        let drawing_for_events = drawing_area.clone();
        glib::spawn_future_local(async move {
            while let Some(()) = rx_ui.recv().await {
                drawing_for_events.queue_draw();
            }
        });
    });

    app.run();
}
