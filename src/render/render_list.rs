#[derive(Debug, Clone, Copy)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32
}

#[allow(unused)]
impl Color {
    pub fn new(r: f32, g: f32, b: f32, a: f32) -> Color {
        Color {
            r: r as f32,
            g: g as f32,
            b: b as f32,
            a: a as f32,
        }
    }

    pub fn from_u8(r: u8, g: u8, b: u8, a: u8) -> Color {
        Color {
            r: r as f32 / 255.0,
            g: g as f32 / 255.0,
            b: b as f32 / 255.0,
            a: a as f32 / 255.0,
        }
    }

    fn r_u8(&self) -> u8 {
        (self.r * 255.0) as u8
    }
    fn g_u8(&self) -> u8 {
        (self.g * 255.0) as u8
    }
    fn b_u8(&self) -> u8 {
        (self.b * 255.0) as u8
    }
    fn a_u8(&self) -> u8 {
        (self.a * 255.0) as u8
    }
}

// This is some temporary code for the paintlist system. Since we already have a good render pipeline, we will be using
// that one after we have implemented this system.
#[derive(Clone, Debug)]
pub enum DisplayItem {
    Clear   { color: Color },
    Rect    { x: f32, y: f32, w: f32, h: f32, color: Color },
    TextRun { x: f32, y: f32, text: String, size: f32 , color: Color },
}


#[derive(Clone, Debug, Default)]
pub struct RenderList {
    pub items: Vec<DisplayItem>,
    // More stuff when needed
}

impl RenderList {
    pub fn new() -> Self {
        RenderList {
            items: Vec::new(),
        }
    }

    pub fn add_command(&mut self, command: DisplayItem) {
        self.items.push(command);
    }

    pub fn clear(&mut self) {
        self.items.clear();
    }
}