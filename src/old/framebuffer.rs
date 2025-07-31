pub const WIDTH: usize = 800;
pub const HEIGHT: usize = 600;

pub struct FrameBuffer {
    pub pixels: Vec<u8>,
    pub width: usize,
    pub height: usize,
}

impl FrameBuffer {
    pub fn new(width: usize, height: usize) -> Self {
        let pixels = vec![255; width * height * 4]; // RGBA format
        Self {
            pixels,
            width,
            height,
        }
    }

    pub fn draw_text(&mut self, text: &str) {
        for (i, c) in text.bytes().enumerate().take(self.pixels.len() / 4) {
            let offset = i * 4;
            self.pixels[offset] = c;        // R
            self.pixels[offset + 1] = c;    // G
            self.pixels[offset + 2] = c;    // B
            self.pixels[offset + 3] = 255;  // A
        }
    }
}