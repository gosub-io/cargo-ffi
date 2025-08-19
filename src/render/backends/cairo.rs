use std::any::Any;
use anyhow::{anyhow, Result};
use crate::render::backend::{RenderBackend, ErasedSurface, ExternalHandle, PresentMode, RgbaImage, SurfaceSize, PixelFormat};
use crate::engine::BrowsingContext;
use crate::render::DisplayItem;

/// Cairo backend for rendering using cairo graphics library.
pub struct CairoBackend;

impl CairoBackend {
    pub fn new() -> Self {
        Self {}
    }
}

impl RenderBackend for CairoBackend {
    /// Will create a new Cairo surface with the given size and present mode.
    fn create_surface(
        &self,
        size: SurfaceSize,
        present: PresentMode,
    ) -> Result<Box<dyn ErasedSurface>> {
        Ok(Box::new(CairoSurface::new(size, present)?))
    }

    /// Renders a surface by getting the DisplayItems from the browsing context and rendering them
    /// onto the ErasedSurface
    fn render(
        &mut self,
        ctx: &mut BrowsingContext,
        surface: &mut dyn ErasedSurface,
    ) -> Result<()> {
        // Ensure the surface is a CairoSurface.
        let s = surface.as_any_mut()
            .downcast_mut::<CairoSurface>()
            .expect("CairoBackend used with non-Cairo surface");

        {
            // Get the cairo context (CR) from the surface.
            let cr = s.ctx()?;

            for item in ctx.render_list().items.iter() {
                match item {
                    DisplayItem::Clear { color } => {
                        // Clear the surface with the specified color.
                        cr.set_operator(cairo::Operator::Source);
                        cr.set_source_rgba(color.r as f64, color.g as f64, color.b as f64, color.a as f64);
                        cr.paint()?;
                        cr.set_operator(cairo::Operator::Over);

                    }
                    DisplayItem::Rect { x, y, w, h, color } => {
                        // Draw a rectangle with the specified color.
                        cr.set_source_rgba(color.r as f64, color.g as f64, color.b as f64, color.a as f64);
                        cr.rectangle(*x as f64, *y as f64, *w as f64, *h as f64);
                        cr.fill()?;
                    }
                    DisplayItem::TextRun { x, y, text, size, color } => {
                        // Draw text at the specified position with the specified size and color.
                        cr.set_source_rgba(color.r as f64, color.g as f64, color.b as f64, color.a as f64);
                        cr.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Normal);
                        cr.set_font_size(*size as f64);
                        cr.move_to(*x as f64, *y as f64);
                        cr.show_text(text)?;
                    }
                }
            }
        }

        s.frame_id = s.frame_id.wrapping_add(1);
        Ok(())
    }

    /// Generates a snapshot of the surface as a small RGBA8 image.
    fn snapshot(&mut self, surface: &mut dyn ErasedSurface, _max_dim: u32) -> Result<RgbaImage> {
        let s = surface
            .as_any_mut()
            .downcast_mut::<CairoSurface>()
            .ok_or_else(|| anyhow!("CairoBackend used with non-Cairo surface"))?;

        let ExternalHandle::CpuPixelsOwned { pixels, width, height, stride, .. } = s.take_external_owned() else {
            return Err(anyhow!("unexpected external handle kind"));
        };

        let img = RgbaImage::from_raw(pixels, width, height, stride, PixelFormat::PreMulArgb32);

        Ok(img)
    }

    fn external_handle(&mut self, surface: &mut dyn ErasedSurface) -> Option<ExternalHandle> {
        let s = surface.as_any_mut().downcast_mut::<CairoSurface>()?;
        Some(s.take_external_owned())
    }
}



pub struct CairoSurface {
    surface: cairo::ImageSurface,     // This image surface sits on top of the buf below
    buf: Box<[u8]>,       // Pixels will be written to here (through surface), but we ultimately own them
    size: SurfaceSize,
    stride: i32,
    #[allow(unused)]
    present: PresentMode,
    frame_id: u64,
}

impl CairoSurface {
    fn new(size: SurfaceSize, present: PresentMode) -> Result<Self> {
        let stride = cairo::Format::ARgb32
            .stride_for_width(size.width)
            .unwrap_or((size.width * 4) as i32);

        // Allocate a buffer large enough for the surface to be mapped on top.
        let mut buf: Box<[u8]> = vec![0u8; (size.height as usize) * (stride as usize)].into_boxed_slice();

        // SAFETY: `buf` is stored in `Self` and outlives `surface
        let slice_static: &'static mut [u8] = unsafe {
            std::mem::transmute::<&mut [u8], &'static mut [u8]>(&mut *buf)
        };
        let surface = cairo::ImageSurface::create_for_data(
            slice_static,
            cairo::Format::ARgb32,
            size.width as i32,
            size.height as i32,
            stride
        )?;

        Ok(Self {
            surface,
            buf,
            size,
            stride,
            present,
            frame_id: 0,
        })
    }

    #[inline]
    pub fn ctx(&self) -> Result<cairo::Context> {
        Ok(cairo::Context::new(&self.surface)?)
    }

    #[inline]
    pub fn stride(&self) -> i32 {
        self.stride
    }

    #[inline]
    pub fn flush(&self) {
        // Flush the surface to ensure all operations are completed.
        self.surface.flush();
    }

    /// Cheap read-only borrow of the pixels (no copy).
    /// Lifetime is tied to &self, and you must not draw while holding this slice.
    pub fn pixels_borrowed(&self) -> (&[u8], u32, u32, u32) {
        self.flush();

        (
            &self.buf,
            self.size.width,
            self.size.height,
            self.stride as u32
        )
    }

    /// Zero-copy move of the owned pixel Vec into your external handle.
    /// After this, the Cairo surface is dropped and must not be used.
    pub fn take_external_owned(&mut self) -> ExternalHandle {
        self.flush();

        let w = self.size.width as i32;
        let h = self.size.height as i32;
        let stride = self.stride;

        // fresh buffer/surface to keep this surface usable
        let mut fresh: Box<[u8]> = vec![0u8; (h as usize) * (stride as usize)].into_boxed_slice();
        let fresh_static: &'static mut [u8] = unsafe {
            std::mem::transmute::<&mut [u8], &'static mut [u8]>(&mut *fresh)
        };
        let new_surface = cairo::ImageSurface::create_for_data(
            fresh_static,
            cairo::Format::ARgb32,
            w,
            h,
            stride
        ).expect("create_for_data(fresh)");

        let old_surface = std::mem::replace(&mut self.surface, new_surface);
        let old_buf = std::mem::replace(&mut self.buf, fresh);
        drop(old_surface);

        ExternalHandle::CpuPixelsOwned {
            pixels: old_buf.into(),
            width: self.size.width,
            height: self.size.height,
            stride: self.stride as u32,
            format: PixelFormat::PreMulArgb32,
        }
    }
}

impl ErasedSurface for CairoSurface {
    fn as_any(&self) -> &dyn Any { self }
    fn as_any_mut(&mut self) -> &mut dyn Any { self }
    fn size(&self) -> SurfaceSize { self.size }
}
