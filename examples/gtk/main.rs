use gosub_engine::{GosubEngine, EngineCommand, EngineEvent, Viewport};
use gosub_engine::cookies::{CookieStore, SqliteCookieStore};
use gosub_engine::ZoneId;
use gtk4::prelude::*;
use gtk4::{glib, Application, ApplicationWindow, Box as GtkBox, Button, DrawingArea, Entry, EventControllerMotion, EventControllerScroll, EventControllerScrollFlags, Orientation};
use gtk4::{GestureClick};
use gtk4::glib::clone;
use std::cell::RefCell;
use std::rc::Rc;
use crate::tiling::{close_leaf, collect_leaves, compute_layout, find_leaf_at, split_leaf_into_cols, split_leaf_into_rows, LayoutHandle, LayoutNode, Rect};

mod tiling;

const DEFAULT_MAIN_ZONE : &str = "95d9c701-5f1b-43ea-ba7e-bc509ee8aa54";

fn main() {
    let app = Application::builder().application_id("io.gosub.engine").build();

    // Persistent cookie store
    let cookie_store = SqliteCookieStore::new(".gosub-gtk-cookie-store.db".parse().unwrap());

    app.connect_activate(move |app| {
        let engine = Rc::new(RefCell::new(GosubEngine::new(None)));
        let viewport = Viewport::new(0, 0, 800, 600);

        // Let's create our default zone
        let zone_id = engine.borrow_mut().create_zone(Some(ZoneId::from(DEFAULT_MAIN_ZONE)), None).expect("zone creation failed");

        // Add sqlite cookie jar to the zone
        let zone_arc = engine.borrow_mut().get_zone_mut(zone_id).expect("get_zone_mut failed");
        let mut zone = zone_arc.lock().expect("lock zone failed");
        let cookie_jar = cookie_store.get_jar(zone_id).expect("get cookie jar failed");
        zone.set_cookie_jar(cookie_jar);
        drop(zone);

        // Start with 1 tab
        let tab0 = engine.borrow_mut().open_tab(zone_id, &viewport).expect("open_tab failed");

        let root: LayoutHandle = Rc::new(RefCell::new(LayoutNode::Leaf(tab0)));
        let active_tab = Rc::new(RefCell::new(tab0));
        let last_size = Rc::new(RefCell::new((800i32, 600i32)));


        let address_entry = Entry::new();
        address_entry.set_placeholder_text(Some("Enter URL for active pane..."));
        address_entry.set_hexpand(true);

        let drawing_area = DrawingArea::new();
        drawing_area.set_content_width(800);
        drawing_area.set_content_height(600);
        drawing_area.set_focusable(true);

        // Toolbar: Split Col, Split Row, Close Pane
        let btn_split_col = Button::with_label("Split Col");
        let btn_split_row = Button::with_label("Split Row");
        let btn_close = Button::with_label("Close Pane");

        // -----------------------------
        // Split handlers
        // -----------------------------
        let eng_split = engine.clone();
        let root_split = root.clone();
        let last_size_split = last_size.clone();
        let drawing_split = drawing_area.clone();
        let active_split = active_tab.clone();
        btn_split_col.connect_clicked(clone!(@strong eng_split, @strong root_split, @strong last_size_split, @strong drawing_split, @strong active_split => move |_| {
            // Open a new tab sized like the active pane
            let (w, h) = *last_size_split.borrow();
            let new_tab = eng_split.borrow_mut().open_tab(zone_id, &Viewport::new(0, 0, (w/2).max(1) as u32, h as u32)).expect("open_tab failed");

            let target = *active_split.borrow();
            split_leaf_into_cols(&root_split, target, vec![new_tab]);
            // Send resizes to all leaves after split
            let mut pairs = Vec::new();
            compute_layout(&root_split.borrow(), Rect { x:0, y:0, w, h }, &mut pairs);
            let mut eng = eng_split.borrow_mut();
            for (tid, r) in pairs { let _ = eng.handle_event(tid, EngineEvent::Resize{ width: r.w as u32, height: r.h as u32 }); }
            drawing_split.queue_draw();
        }));

        let eng_split2 = engine.clone();
        let root_split2 = root.clone();
        let last_size_split2 = last_size.clone();
        let drawing_split2 = drawing_area.clone();
        let active_split2 = active_tab.clone();
        btn_split_row.connect_clicked(clone!(@strong eng_split2, @strong root_split2, @strong last_size_split2, @strong drawing_split2, @strong active_split2 => move |_| {
            let (w, h) = *last_size_split2.borrow();
            let new_tab = eng_split2.borrow_mut().open_tab(zone_id, &Viewport::new(0, 0, w as u32, (h/2).max(1) as u32)).expect("open_tab failed");

            let target = *active_split2.borrow();
            split_leaf_into_rows(&root_split2, target, vec![new_tab]);
            let mut pairs = Vec::new();
            compute_layout(&root_split2.borrow(), Rect { x:0, y:0, w, h }, &mut pairs);
            let mut eng = eng_split2.borrow_mut();
            for (tid, r) in pairs { let _ = eng.handle_event(tid, EngineEvent::Resize{ width: r.w as u32, height: r.h as u32 }); }
            drawing_split2.queue_draw();
        }));

        let eng_close = engine.clone();
        let root_close = root.clone();
        let last_size_close = last_size.clone();
        let drawing_close = drawing_area.clone();
        let active_close = active_tab.clone();
        btn_close.connect_clicked(clone!(@strong eng_close, @strong root_close, @strong last_size_close, @strong drawing_close, @strong active_close => move |_| {
            let target = *active_close.borrow();
            if close_leaf(&root_close, target) {
                // Pick a new active from remaining leaves
                let mut leaves = Vec::new();
                collect_leaves(&root_close.borrow(), &mut leaves);
                if let Some(&first) = leaves.first() { *active_close.borrow_mut() = first; }
                let (w, h) = *last_size_close.borrow();
                let mut pairs = Vec::new();
                compute_layout(&root_close.borrow(), Rect { x:0, y:0, w, h }, &mut pairs);
                let mut eng = eng_close.borrow_mut();
                for (tid, r) in pairs { let _ = eng.handle_event(tid, EngineEvent::Resize{ width: r.w as u32, height: r.h as u32 }); }
                drawing_close.queue_draw();
            }
        }));

        // Drawing area
        let eng_draw = engine.clone();
        let root_draw = root.clone();
        let active_draw = active_tab.clone();
        drawing_area.set_draw_func(move |_area, cr, w, h| {
            let mut pairs = Vec::new();
            compute_layout(&root_draw.borrow(), Rect { x:0, y:0, w, h }, &mut pairs);
            let eng = eng_draw.borrow();
            let active = *active_draw.borrow();

            for (tid, r) in &pairs {
                if let Some(surface) = eng.get_surface(*tid) {
                    cr.save().unwrap();
                    cr.rectangle(r.x as f64, r.y as f64, r.w as f64, r.h as f64);
                    cr.clip();
                    cr.translate(r.x as f64, r.y as f64);

                    let sw = surface.width() as f64;
                    let sh = surface.height() as f64;
                    if sw > 0.0 && sh > 0.0 && (sw as i32 != r.w || sh as i32 != r.h) {
                        cr.scale(r.w as f64 / sw, r.h as f64 / sh);
                    }
                    cr.set_source_surface(&surface, 0.0, 0.0).unwrap();
                    cr.paint().unwrap();
                    cr.restore().unwrap();
                }
            }

            // Draw an outline around the active pane
            for (tid, r) in &pairs {
                if *tid == active {
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
        let eng_resize = engine.clone();
        let root_resize = root.clone();
        let last_size_resize = last_size.clone();
        drawing_area.connect_resize(clone!(@strong eng_resize, @strong root_resize, @strong last_size_resize => move |_area, w, h| {
            *last_size_resize.borrow_mut() = (w, h);
            let mut pairs = Vec::new();
            compute_layout(&root_resize.borrow(), Rect { x:0, y:0, w, h }, &mut pairs);
            let mut eng = eng_resize.borrow_mut();
            for (tid, r) in pairs {
                let _ = eng.handle_event(tid, EngineEvent::Resize{ width: r.w as u32, height: r.h as u32 });
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
            if let Some(tid) = find_leaf_at(&root_pick.borrow(), Rect { x:0, y:0, w, h }, x, y) {
                *active_pick.borrow_mut() = tid;
                drawing_pick.queue_draw();
            }
        });
        drawing_area.add_controller(click);

        // Address entry: navigate active tab
        let eng_entry = engine.clone();
        let active_entry = active_tab.clone();
        let draw_entry = drawing_area.clone();
        address_entry.connect_activate(clone!(@strong eng_entry, @strong active_entry, @strong draw_entry => move |entry| {
            let url = entry.text().to_string();
            let tid = *active_entry.borrow();
            let _ = eng_entry.borrow_mut().execute_command(tid, EngineCommand::Navigate(url));
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

        // Scroll pane
        let eng_scroll = engine.clone();
        let root_scroll = root.clone();
        let last_size_scroll = last_size.clone();
        let drawing_scroll = drawing_area.clone();
        let last_pointer_scroll = last_pointer.clone();

        let scroll = EventControllerScroll::new(EventControllerScrollFlags::BOTH_AXES);
        scroll.connect_scroll(clone!(@strong eng_scroll, @strong root_scroll, @strong last_size_scroll, @strong drawing_scroll, @strong last_pointer_scroll => move |_ctrl, dx, dy| {
            // Where is the pointer?
            let (px, py) = *last_pointer_scroll.borrow();

            // Which pane is under the pointer?
            let (w, h) = *last_size_scroll.borrow();
            if let Some(tid) = find_leaf_at(&root_scroll.borrow(), Rect { x:0, y:0, w, h }, px, py) {
                // Scale deltas: touchpads give smooth deltas; mouse wheel often ~±1 step.
                // Tweak this multiplier for your content’s line/px semantics.
                let line_h = 20.0_f64; // about 40 px per "wheel step"
                let dx_px = (dx * line_h) as f32;
                let dy_px = (dy * line_h) as f32;

                // Send to the engine (you implement what Scroll does per tab)
                let _ = eng_scroll.borrow_mut().handle_event(tid, EngineEvent::Scroll { dx: dx_px, dy: dy_px });

                // Ask GTK to redraw
                drawing_scroll.queue_draw();
            }

            return glib::Propagation::Proceed;
        }));
        drawing_area.add_controller(scroll);

        // Layout boxes
        let toolbar = GtkBox::new(Orientation::Horizontal, 6);
        toolbar.append(&btn_split_col);
        toolbar.append(&btn_split_row);
        toolbar.append(&btn_close);
        toolbar.append(&address_entry);

        let vbox = GtkBox::new(Orientation::Vertical, 6);
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

        // FrameClock tick: redraw if any visible tab needs it
        let fc = drawing_area.frame_clock().unwrap();
        let eng_fc = engine.clone();
        let root_fc = root.clone();
        let drawing_fc = drawing_area.clone();
        let last_size_fc = last_size.clone();
        fc.connect_update(clone!(@strong drawing_fc, @strong eng_fc, @strong root_fc => move |_clk| {
            let mut eng_mut = eng_fc.borrow_mut();
            let results = eng_mut.tick();

            // If any leaf needs redraw, repaint
            let (w, h) = *last_size_fc.borrow();
            let mut pairs = Vec::new();
            compute_layout(&root_fc.borrow(), Rect { x:0, y:0, w, h }, &mut pairs);

            let mut redraw = false;
            for (tid, _r) in pairs {
                if let Some(res) = results.get(&tid) {
                    if res.page_loaded {
                        println!("Page loaded for tab {:?}", tid);
                    }
                    if res.needs_redraw { redraw = true; }
                }
            }
            if redraw { drawing_fc.queue_draw(); }
        }));
    });

    app.run();
}
