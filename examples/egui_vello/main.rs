use crate::compositor::VelloCompositor;
use crate::tiling::{
    close_leaf, collect_leaves, compute_layout, find_leaf_at, split_leaf_into_cols, split_leaf_into_rows, LayoutHandle,
    LayoutNode, Rect,
};
use crate::wgpu_context_provider::EguiWgpuContextProvider;
use eframe::{egui, CreationContext};
use egui::load::SizedTexture;
use egui::StrokeKind;
use gosub_engine::cookies::SqliteCookieStore;
use gosub_engine::render::backend::ExternalHandle;
use gosub_engine::render::backends::vello::WgpuContextProvider;
use gosub_engine::render::Viewport;
use gosub_engine::storage::{InMemorySessionStore, SqliteLocalStore, StorageService};
use gosub_engine::zone::{ZoneConfig, ZoneId};
use gosub_engine::{EngineCommand, EngineEvent, GosubEngine};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use tokio::sync::mpsc::channel;
use url::Url;

mod compositor;
mod tiling;
mod wgpu_context_provider;

const DEFAULT_MAIN_ZONE: &str = "95d9c701-5f1b-43ea-ba7e-bc509ee8aa54";
const DEFAULT_WIDTH: u32 = 800;
const DEFAULT_HEIGHT: u32 = 600;

// fn current_url_for_tab(eng: &GosubEngine, tab_id: gosub_engine::tab::TabId) -> Option<Url> {
//     eng.get_tab(tab_id)
//         .unwrap()
//         .lock()
//         .unwrap()
//         .current_url
//         .clone()
// }

struct GosubApp {
    engine: Arc<RefCell<GosubEngine>>,
    zone_id: ZoneId,
    root: LayoutHandle,
    active_tab: Arc<RefCell<gosub_engine::tab::TabId>>,
    last_size: Arc<RefCell<(i32, i32)>>,
    compositor: Arc<RefCell<VelloCompositor>>,
    // storage: Arc<StorageService>,
    current_url_input: String,
    needs_redraw: bool,
    pointer_pos: (f64, f64),
    backend_initialized: bool,
    ctx_provider: Arc<EguiWgpuContextProvider>,
    // Keep track of last panel size to detect changes
    last_panel_size: egui::Vec2,
}

impl GosubApp {
    fn new(cc: &CreationContext) -> Self {
        // Setup persistent cookie store
        let cookie_store = SqliteCookieStore::new(".gosub-gtk-cookie-store.db".parse().unwrap());
        let storage = Arc::new(StorageService::new(
            Arc::new(SqliteLocalStore::new(".gosub-gtk-local-storage.db").unwrap()),
            Arc::new(InMemorySessionStore::new()),
        ));

        // Set up the engine with a null backend for now. We will update this once we have an egui context
        // so we can initialize the vello renderer properly.
        let backend = gosub_engine::render::backends::null::NullBackend::new().expect("NullBackend::new failed");
        let engine = Arc::new(RefCell::new(GosubEngine::new(None, Box::new(backend))));

        let ctx_provider =
            Arc::new(EguiWgpuContextProvider::from_eframe(cc).expect("Failed to create EguiWgpuContextProvider"));

        let (tx, rx) = channel();

        // Create our main zone ID
        let config = ZoneConfig::builder().max_tabs(100).build().unwrap();
        let zone = engine
            .borrow_mut()
            .zone_builder()
            .id(ZoneId::from(DEFAULT_MAIN_ZONE))
            .storage(storage.clone())
            .cookie_store(cookie_store.clone())
            .config(config)
            .create()
            .expect("zone creation failed");

        let tabs: Vec<Tabhandle> = Vec![];

        // Open a tab in our zone
        let viewport = Viewport::new(0, 0, DEFAULT_WIDTH as u32, DEFAULT_HEIGHT as u32);
        let tab0 = engine
            .borrow_mut()
            .open_tab_in_zone(zone_id, viewport)
            .expect("open_tab failed");

        // Setup titing state and add our first tab
        let root: Rc<RefCell<LayoutNode>> = Rc::new(RefCell::new(LayoutNode::Leaf(tab0)));
        let active_tab = Arc::new(RefCell::new(tab0));
        let last_size = Arc::new(RefCell::new((DEFAULT_WIDTH as i32, DEFAULT_HEIGHT as i32)));

        // Connect compositor to the engine
        let compositor = Arc::new(RefCell::new(VelloCompositor::new()));

        Self {
            engine,
            zone_id,
            root,
            active_tab,
            last_size,
            compositor,
            // storage,
            current_url_input: String::new(),
            needs_redraw: true,
            pointer_pos: (0.0, 0.0),
            backend_initialized: false,
            ctx_provider,
            last_panel_size: egui::Vec2::ZERO,
        }
    }

    fn handle_navigation(&mut self) {
        let composed_url = self.current_url_input.clone();

        // Check if composed_url starts with a scheme like http:// or https://
        let url_str = if !composed_url.starts_with("http://") && !composed_url.starts_with("https://") {
            format!("https://{}", composed_url)
        } else {
            composed_url
        };

        let Ok(url) = Url::parse(&url_str) else {
            return;
        };

        let tab_id = *self.active_tab.borrow();
        let _ = self
            .engine
            .borrow_mut()
            .execute_command(tab_id, EngineCommand::Navigate(url));
        self.needs_redraw = true;
    }

    fn handle_split_col(&mut self) {
        let (w, h) = *self.last_size.borrow();
        let new_tab = self
            .engine
            .borrow_mut()
            .open_tab_in_zone(
                self.zone_id,
                Viewport::new(0, 0, (w / 2).max(1) as u32, h as u32),
            )
            .expect("open_tab failed");

        let target = *self.active_tab.borrow();
        split_leaf_into_cols(&self.root, target, vec![new_tab]);

        // Send resizes to all leaves after split
        let mut pairs = Vec::new();
        compute_layout(&self.root.borrow(), Rect { x: 0, y: 0, w, h }, &mut pairs);
        let mut eng = self.engine.borrow_mut();
        for (tab_id, r) in pairs {
            let _ = eng.handle_event(
                tab_id,
                EngineEvent::Resize {
                    width: r.w as u32,
                    height: r.h as u32,
                },
            );
        }
        self.needs_redraw = true;
    }

    fn handle_split_row(&mut self) {
        let (w, h) = *self.last_size.borrow();
        let new_tab = self
            .engine
            .borrow_mut()
            .open_tab_in_zone(
                self.zone_id,
                Viewport::new(0, 0, w as u32, (h / 2).max(1) as u32),
            )
            .expect("open_tab failed");

        let target = *self.active_tab.borrow();
        split_leaf_into_rows(&self.root, target, vec![new_tab]);

        let mut pairs = Vec::new();
        compute_layout(&self.root.borrow(), Rect { x: 0, y: 0, w, h }, &mut pairs);
        let mut eng = self.engine.borrow_mut();
        for (tab_id, r) in pairs {
            let _ = eng.handle_event(
                tab_id,
                EngineEvent::Resize {
                    width: r.w as u32,
                    height: r.h as u32,
                },
            );
        }
        self.needs_redraw = true;
    }

    fn handle_close_pane(&mut self) {
        let target = *self.active_tab.borrow();
        if close_leaf(&self.root, target) {
            // Pick a new active from remaining leaves
            let mut leaves = Vec::new();
            collect_leaves(&self.root.borrow(), &mut leaves);
            if let Some(&first) = leaves.first() {
                *self.active_tab.borrow_mut() = first;
            }
            let (w, h) = *self.last_size.borrow();
            let mut pairs = Vec::new();
            compute_layout(&self.root.borrow(), Rect { x: 0, y: 0, w, h }, &mut pairs);
            let mut eng = self.engine.borrow_mut();
            for (tab_id, r) in pairs {
                let _ = eng.handle_event(
                    tab_id,
                    EngineEvent::Resize {
                        width: r.w as u32,
                        height: r.h as u32,
                    },
                );
            }
            self.needs_redraw = true;
        }
    }

    fn handle_click(&mut self, pos: egui::Pos2) {
        let (w, h) = *self.last_size.borrow();
        if let Some(tab_id) = find_leaf_at(
            &self.root.borrow(),
            Rect { x: 0, y: 0, w, h },
            pos.x as f64,
            pos.y as f64,
        ) {
            *self.active_tab.borrow_mut() = tab_id;
            self.needs_redraw = true;
        }
    }

    fn handle_scroll(&mut self, delta: egui::Vec2) {
        let (px, py) = self.pointer_pos;
        let (w, h) = *self.last_size.borrow();

        if let Some(tab_id) = find_leaf_at(&self.root.borrow(), Rect { x: 0, y: 0, w, h }, px, py) {
            let line_h = 2.0;
            let dx_px = delta.x * line_h;
            let dy_px = delta.y * line_h;

            // Send to the engine
            let _ = self
                .engine
                .borrow_mut()
                .handle_event(tab_id, EngineEvent::Scroll { dx: dx_px, dy: dy_px });
            self.needs_redraw = true;
        }
    }

    fn init_vello_backend(&mut self) {
        // Noop if already initialized
        if self.backend_initialized {
            return;
        }

        // At this point in time, we have both the gosub engine and the WgpuContext provider
        // available. Here we can tie them together by changing the NULL backend renderer
        // that we initialized previously, with the a new Vello backend renderer that uses
        // our Wgpu context provider as the bridge between the renderer and the UI.
        match gosub_engine::render::backends::vello::VelloBackend::new(self.ctx_provider.clone()) {
            Ok(backend) => {
                self.engine
                    .borrow_mut()
                    .set_backend_renderer(Box::new(backend));
                self.backend_initialized = true;
                self.needs_redraw = true;
            }
            Err(e) => {
                eprintln!("Failed to initialize Vello backend: {}", e);
            }
        }
    }
}

impl eframe::App for GosubApp {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        ctx.request_repaint_after(std::time::Duration::from_millis(16));

        // Initialize vello backend if not already done
        self.init_vello_backend();

        // Set larger font size for the whole UI
        let mut style = (*ctx.style()).clone();
        style
            .text_styles
            .get_mut(&egui::TextStyle::Body)
            .unwrap()
            .size = 16.0;
        style
            .text_styles
            .get_mut(&egui::TextStyle::Button)
            .unwrap()
            .size = 14.0;
        ctx.set_style(style);

        // Update pointer position
        if let Some(pointer_pos) = ctx.pointer_latest_pos() {
            self.pointer_pos = (pointer_pos.x as f64, pointer_pos.y as f64);
        }

        // Handle scrolling
        let scroll_delta = ctx.input(|i| i.smooth_scroll_delta);
        if scroll_delta != egui::Vec2::ZERO {
            self.handle_scroll(scroll_delta);
        }

        // Handle mouse clicks
        if ctx.input(|i| i.pointer.primary_clicked()) {
            if let Some(click_pos) = ctx.input(|i| i.pointer.interact_pos()) {
                self.handle_click(click_pos);
            }
        }

        // Update engine and check if we need to redraw
        let results = self
            .engine
            .borrow_mut()
            .tick(&mut *self.compositor.borrow_mut());

        // check if any tab needs redraw
        let (w, h) = *self.last_size.borrow();
        let mut pairs = Vec::new();
        compute_layout(&self.root.borrow(), Rect { x: 0, y: 0, w, h }, &mut pairs);

        for (tab_id, _r) in pairs {
            if let Some(res) = results.get(&tab_id) {
                if res.page_loaded {
                    println!("Tab {:?} loaded page", tab_id);
                }
                if res.needs_redraw {
                    self.needs_redraw = true;
                }
            }
        }

        // // Update URL input from active tab
        // let tab_id = *self.active_tab.borrow();
        // if let Some(url) = current_url_for_tab(&self.engine.borrow(), tab_id) {
        //     self.current_url_input = url.to_string();
        // }

        // UI Layout
        egui::TopBottomPanel::top("address_bar")
            .frame(egui::Frame::default().inner_margin(egui::Margin::symmetric(12, 10)))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    // Make address bar text bigger
                    let response = ui.add(
                        egui::TextEdit::singleline(&mut self.current_url_input)
                            .desired_width(f32::INFINITY)
                            .frame(true)
                            .hint_text("Enter URL")
                            .char_limit(100)
                            .font(egui::TextStyle::Heading)
                            .min_size(egui::vec2(0.0, 36.0)),
                    );

                    if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        self.handle_navigation();
                    }
                });
            });

        egui::TopBottomPanel::top("toolbar")
            .frame(egui::Frame::default().inner_margin(egui::Margin::symmetric(12, 8)))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    // Bigger buttons with padding
                    let button_size = egui::vec2(120.0, 32.0);

                    if ui
                        .add(egui::Button::new("Split Col").min_size(button_size))
                        .clicked()
                    {
                        self.handle_split_col();
                    }

                    if ui
                        .add(egui::Button::new("Split Row").min_size(button_size))
                        .clicked()
                    {
                        self.handle_split_row();
                    }

                    if ui
                        .add(egui::Button::new("Close Pane").min_size(button_size))
                        .clicked()
                    {
                        self.handle_close_pane();
                    }
                });
            });

        let panel = egui::CentralPanel::default().show(ctx, |ui| {
            // Update the last_size with the current avaiable space
            let available_size = ui.available_size();
            println!("Available size: {}", available_size);
            *self.last_size.borrow_mut() = (available_size.x as i32, available_size.y as i32);

            // Compute layout for all tabs
            let (w, h) = *self.last_size.borrow();
            let mut pairs = Vec::new();
            compute_layout(&self.root.borrow(), Rect { x: 0, y: 0, w, h }, &mut pairs);

            // Draw each tab's content
            let active_tab_id = *self.active_tab.borrow();

            for (tab_id, rect) in pairs {
                let is_active = tab_id == active_tab_id;

                println!("Rect: {:?}", rect);

                // Get the compositor frame from this tab
                let mut compositor = self.compositor.borrow_mut();
                if let Some(handle) = compositor.frame_for_mut(tab_id) {
                    let rect_ui = egui::Rect::from_min_max(
                        egui::pos2(rect.x as f32, rect.y as f32),
                        egui::pos2(rect.w as f32, rect.h as f32),
                    );

                    match handle {
                        ExternalHandle::WgpuTextureId { id, width, height, .. } => {
                            let (_, view) = self.ctx_provider.get_texture(*id).unwrap();

                            let mut renderer = frame.wgpu_render_state().unwrap().renderer.write();
                            let device = &frame.wgpu_render_state().unwrap().device;

                            let tid = renderer.register_native_texture(device, &view, wgpu::FilterMode::Nearest);

                            let ppp = ui.ctx().pixels_per_point();
                            // let size_points = egui::Vec2::new(*width as f32 / ppp, *height as f32 / ppp);
                            let size_points = egui::Vec2::new((*width - 25) as f32, (*height - 25) as f32);
                            ui.add(egui::Image::new(SizedTexture::new(tid, size_points)));
                        }
                        _ => {
                            eprintln!("Unsupported handle type for tab {:?}: {:?}", tab_id, handle);
                        }
                    }

                    let col = if is_active {
                        egui::Color32::YELLOW
                    } else {
                        egui::Color32::DARK_GRAY
                    };

                    ui.painter().rect_stroke(
                        rect_ui,
                        0.0,
                        egui::Stroke::new(1.0, col),
                        StrokeKind::Outside,
                    );
                }
            }

            if self.needs_redraw {
                ctx.request_repaint();
                self.needs_redraw = false;
            }
        });

        // Check tab panel dimensions to see if we need to recalculate layouts (ie: resize occurred)
        let size = panel.response.rect.size();
        if size != self.last_panel_size {
            println!("Redraw needed due to panel size change: {}", size);
            // let pixels_per_point = ctx.pixels_per_point();
            // let size_in_pixels = size * pixels_per_point;
            // self.last_panel_size = size_in_pixels;

            self.last_panel_size = size;

            // Calculate new panel layouts
            let mut pairs = Vec::new();
            compute_layout(
                &self.root.borrow(),
                Rect {
                    x: 0,
                    y: 0,
                    w: size.x as i32,
                    h: size.y as i32,
                },
                &mut pairs,
            );

            // Signal all tabs that we have resized them
            let mut eng = self.engine.borrow_mut();
            for (tab_id, r) in pairs {
                let _ = eng.handle_event(
                    tab_id,
                    EngineEvent::Resize {
                        width: r.w as u32,
                        height: r.h as u32,
                    },
                );
            }
        }
    }
}

fn main() -> Result<(), eframe::Error> {
    // Setup eframe options
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            // .with_inner_size([DEFAULT_WIDTH as f32, DEFAULT_HEIGHT as f32])
            .with_title("Gosub Browser"),
        ..Default::default()
    };

    // And run our app
    eframe::run_native(
        "Gosub Browser",
        options,
        Box::new(|cc| {
            // Set light mode
            cc.egui_ctx.set_visuals(egui::Visuals::light());
            Ok(Box::new(GosubApp::new(cc)))
        }),
    )
}
