use crate::engine::BrowsingContext;
use crate::render::backend::GpuPixelFormat;
use crate::render::backend::{
    ErasedSurface, ExternalHandle, PresentMode, RenderBackend, RgbaImage, SurfaceSize,
};
use crate::render::DisplayItem;
use anyhow::{anyhow, Result};
use parley::{
    layout::PositionedLayoutItem,
    style::{FontFamily, FontStack, StyleProperty},
    FontContext, LayoutContext,
};
use std::any::Any;
use std::cell::RefCell;
use std::sync::Arc;
use vello::kurbo::Affine;
use vello::peniko::{Color, Fill};
use vello::wgpu;
use vello::{Glyph, RenderParams, Renderer, RendererOptions, Scene};

mod font_manager;
use font_manager::FontManager;

fn draw_text(
    fm: &mut FontManager,
    scene: &mut Scene,
    x: f32,
    y: f32,
    text: &str,
    px: f32,
    rgba: [f32; 4],
    font: &str,
) {
    // Resolve the font
    let (vello_font, resolved_name) = fm
        .resolve_ui_font(
            if font.is_empty() { None } else { Some(font) },
            fontique::Attributes::default(),
        )
        .unwrap_or_else(|_| panic!("Failed to resolve font: {}", font)); // format the var!

    let mut font_cx = FontContext::new();
    let mut layout_cx: LayoutContext<[u8; 4]> = LayoutContext::new(); // or LayoutContext::<()>::new()

    let mut builder = layout_cx.ranged_builder(&mut font_cx, text, 1.0, true);
    builder.push_default(StyleProperty::FontSize(px));
    builder.push_default(StyleProperty::FontStack(FontStack::Single(
        FontFamily::Named(resolved_name.clone().into()),
    )));
    let mut layout = builder.build(text);
    layout.break_all_lines(None);
    layout.align(
        None,
        parley::layout::Alignment::Start,
        parley::layout::AlignmentOptions::default(),
    );

    // 4) Stream glyphs to Vello
    for line in layout.lines() {
        for item in line.items() {
            if let PositionedLayoutItem::GlyphRun(run) = item {
                let glyphs_iter = run.positioned_glyphs().map(|g| Glyph {
                    // Swash glyph id → u32
                    id: g.id as u32,
                    x: g.x,
                    y: -run.baseline() + g.y,
                });

                scene
                    .draw_glyphs(&vello_font)
                    .transform(Affine::translate((x as f64, y as f64)))
                    .font_size(px)
                    .brush(Color::new(rgba))
                    .draw(Fill::NonZero, glyphs_iter);
            }
        }
    }
}

/// This trait abstracts over the wgpu context (device, queue, texture management) so we can connect
/// UI based wgpu contexts (like eframe) to the Vello backend.
pub trait WgpuContextProvider {
    fn device(&self) -> &wgpu::Device;
    fn queue(&self) -> &wgpu::Queue;
    fn create_texture(&self, width: u32, height: u32, format: wgpu::TextureFormat) -> u64;
    fn get_texture(&self, id: u64) -> Option<(wgpu::Texture, wgpu::TextureView)>;
    fn remove_texture(&self, id: u64);
}

/// A render backend that uses Vello for rendering.
pub struct VelloBackend<C: WgpuContextProvider> {
    /// The wgpu context provider that we can use for device, queue, and texture management.
    context: Arc<C>,
    /// The Vello renderer instance.
    renderer: Renderer,
    /// Standard font manager for loading fonts
    font_manager: RefCell<FontManager>,
}

impl<C: WgpuContextProvider> VelloBackend<C> {
    pub fn new(context: Arc<C>) -> Result<Self> {
        let renderer = Renderer::new(context.device(), RendererOptions::default())?;
        let font_manager = FontManager::new();

        Ok(Self {
            context,
            renderer,
            font_manager: RefCell::new(font_manager),
        })
    }

    /// Takes a scene and renders it to the given surface.
    fn render_to_surface(&mut self, surface: &VelloSurface, scene: &Scene) -> Result<()> {
        // Retrieve the texture and view from our texture store
        let (_texture, texture_view) = self
            .context
            .get_texture(surface.texture_store_id)
            .expect("invalid texture id in VelloSurface");

        self.renderer.render_to_texture(
            self.context.device(),
            self.context.queue(),
            scene,
            &texture_view,
            &RenderParams {
                base_color: Color::WHITE,
                width: surface.size.width,
                height: surface.size.height,
                antialiasing_method: vello::AaConfig::Area,
            },
        )?;

        Ok(())
    }

    fn convert_browsing_context_to_scene(&self, ctx: &mut BrowsingContext) -> Result<Scene> {
        // Build a scene from your DisplayItems
        let vp = ctx.viewport();
        let offset_x = vp.x as f32;
        let offset_y = vp.y as f32;

        let mut scene = Scene::new();
        for item in ctx.render_list().items.iter() {
            print!("[VelloBackend] Rendering item: {item:?} ");
            match item {
                DisplayItem::Clear { color } => {
                    // full-frame clear
                    scene.fill(
                        Fill::NonZero,
                        Affine::IDENTITY,
                        Color::new([color.r, color.g, color.b, color.a]),
                        None,
                        &vello::kurbo::Rect::new(0.0, 0.0, vp.width as f64, vp.height as f64),
                    );
                }
                DisplayItem::Rect { x, y, w, h, color } => {
                    let x = (*x as f32) - offset_x;
                    let y = (*y as f32) - offset_y;
                    let w = *w as f32;
                    let h = *h as f32;
                    scene.fill(
                        Fill::NonZero,
                        Affine::IDENTITY,
                        Color::new([color.r, color.g, color.b, color.a]),
                        None,
                        &vello::kurbo::Rect::new(
                            x as f64,
                            y as f64,
                            (x + w) as f64,
                            (y + h) as f64,
                        ),
                    );
                }
                DisplayItem::TextRun {
                    x,
                    y,
                    text,
                    size,
                    color,
                } => {
                    let x = (*x as f32) - offset_x;
                    let y = (*y as f32) - offset_y;
                    draw_text(
                        &mut self.font_manager.borrow_mut(),
                        &mut scene,
                        x,
                        y,
                        text,
                        *size,
                        [color.r, color.g, color.b, color.a],
                        "Comic Sans",
                    );
                }
            }
        }

        scene.fill(
            Fill::NonZero,
            Affine::IDENTITY,
            Color::new([255.0, 0.0, 0.0, 1.0]),
            None,
            &vello::kurbo::Rect::new(0.0, 0.0, 100.0, 200.0),
        );

        Ok(scene)
    }
}

impl<C: WgpuContextProvider> RenderBackend for VelloBackend<C> {
    fn create_surface(
        &self,
        size: SurfaceSize,
        _present: PresentMode,
    ) -> Result<Box<dyn ErasedSurface>> {
        let texture_store_id =
            self.context
                .create_texture(size.width, size.height, wgpu::TextureFormat::Rgba8Unorm);

        Ok(Box::new(VelloSurface {
            texture_store_id,
            size,
            frame_id: 1,
        }))
    }

    fn render(&mut self, ctx: &mut BrowsingContext, surface: &mut dyn ErasedSurface) -> Result<()> {
        // Downcast
        let s = surface
            .as_any_mut()
            .downcast_mut::<VelloSurface>()
            .ok_or_else(|| anyhow!("VelloBackend used with non-vello surface"))?;

        // Generate a scene which contains the gpu render commands
        let scene = self.convert_browsing_context_to_scene(ctx)?;

        // Render the scene to the surface
        self.render_to_surface(s, &scene)?;

        // Increment frame id, since we have rendered a new frame onto the surface
        s.frame_id = s.frame_id.wrapping_add(1);

        Ok(())
    }

    /// Takes a snapshot of the surface and returns it as an RGBA image
    fn snapshot(&mut self, _surface: &mut dyn ErasedSurface, _max_dim: u32) -> Result<RgbaImage> {
        Err(anyhow!("VelloBackend snapshot not implemented"))
    }

    /// Converts a surface into an external handle for sending to the compositor
    fn external_handle(&mut self, surface: &mut dyn ErasedSurface) -> Option<ExternalHandle> {
        let s = surface.as_any_mut().downcast_mut::<VelloSurface>()?;

        Some(ExternalHandle::WgpuTextureId {
            id: s.texture_store_id,
            width: s.size.width,
            height: s.size.height,
            format: GpuPixelFormat::Rgba8UnormSrgb, // Not used for now
            frame_id: s.frame_id,
        })
    }
}

/// A vello surface that wraps a wgpu texture.
struct VelloSurface {
    texture_store_id: u64,
    size: SurfaceSize,
    frame_id: u64,
}

impl ErasedSurface for VelloSurface {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn size(&self) -> SurfaceSize {
        self.size
    }
}
