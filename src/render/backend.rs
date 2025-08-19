use std::{any::Any, ptr::NonNull};
use crate::engine::BrowsingContext;
use crate::render::Viewport;

/// Size of a surface in pixels. It's a simple struct to hold width and height.
#[derive(Clone, Copy, Debug)]
#[derive(PartialEq)]
pub struct SurfaceSize { pub width: u32, pub height: u32 }

impl From<Viewport> for SurfaceSize {
    fn from(vp: Viewport) -> Self {
        Self { width: vp.width, height: vp.height }
    }
}

/// Present modes for rendering.
#[derive(Clone, Copy, Debug)]
pub enum PresentMode {
    Fifo,
    Immediate,
    // Mailbox,
    // FifoRelaxed,
}

#[derive(Clone, Copy, Debug)]
pub enum PixelFormat {
    PreMulArgb32,
}

/// Handle the host/browser can composite. Ownership & sync are backend-specific; see docs per variant.
#[derive(Clone, Debug)]
pub enum ExternalHandle {
    CairoSurface { surface: cairo::ImageSurface, width: u32, height: u32 },

    /// CPU pixels in RGBA8. Safer owned alternative to raw pointers.
    CpuPixelsOwned { width: u32, height: u32, stride: u32, pixels: Vec<u8>, format: PixelFormat },

    /// CPU pixels as a borrowed pointer. UNSAFE: caller must respect lifetime/size/stride.
    /// Valid for at least `height * stride` bytes until the next `render()` call on this surface.
    CpuPixelsPtr { width: u32, height: u32, stride: u32, ptr: NonNull<u8> },

    /// GL / GLES texture. `target` is usually GL_TEXTURE_2D or GL_TEXTURE_EXTERNAL_OES.
    /// Optional `frame_id` helps hosts avoid sampling stale frames.
    GlTexture { tex: u32, target: u32, width: u32, height: u32, frame_id: u64 },

    /// WGPU/Vello app-owned indirection. Contract: host can resolve `id` to a usable texture.
    WgpuTextureId { id: u64, width: u32, height: u32, frame_id: u64 },

    /// Skia image handle/ID (e.g., promise image). Contract to be defined with the host.
    SkiaImageId { id: u64, width: u32, height: u32, frame_id: u64 },
}

/// Small RGBA snapshot for thumbnails/tab switcher.
#[derive(Clone)]
pub struct RgbaImage {
    pub pixels: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub stride: u32,
    pub format: PixelFormat,
}

impl RgbaImage {
    pub fn from_raw(
        pixels: Vec<u8>,
        width: u32,
        height: u32,
        stride: u32,
        format: PixelFormat,
    ) -> Self {
        assert!(
            pixels.len() >= (height as usize) * (stride as usize),
            "pixel buffer too small for image dimensions"
        );

        Self {
            pixels,
            width,
            height,
            stride,
            format,
        }
    }
}

impl std::fmt::Debug for RgbaImage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RgbaImage")
            .field("width", &self.width)
            .field("height", &self.height)
            .field("len", &self.pixels.len())
            .finish()
    }
}

/// Type-erased surface so the Engine can hold it without generics.
pub trait ErasedSurface: Any {
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
    fn size(&self) -> SurfaceSize;
}

/// Core backend interface. Calls occur on the backend's owning thread.
pub trait RenderBackend {
    // fn surface_provider(&self) -> Arc<dyn SurfaceProvider>;

    /// Create a new surface with the given size and present mode.
    fn create_surface(&self, size: SurfaceSize, present: PresentMode) -> anyhow::Result<Box<dyn ErasedSurface>>;

    /// Render the current state of the browsing context to the given surface.
    fn render(&mut self, context: &mut BrowsingContext, surface: &mut dyn ErasedSurface) -> anyhow::Result<()>;

    /// Generate a small RGBA8 snapshot of the surface, suitable for thumbnails or previews.
    fn snapshot(&mut self, surface: &mut dyn ErasedSurface, max_dim: u32) -> anyhow::Result<RgbaImage>;

    /// Returns an external handle for the surface, if supported.
    fn external_handle(&mut self, surface: &mut dyn ErasedSurface) -> Option<ExternalHandle>;
}

pub trait CompositorSink {
    fn submit_frame(&mut self, tab: crate::tab::TabId, handle: ExternalHandle);
}