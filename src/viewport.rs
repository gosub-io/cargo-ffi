use std::fmt::Debug;

// Simple width/height viewport. Used for rendering
#[derive(Clone, Eq, PartialEq)]
pub struct Viewport {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl Debug for Viewport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Viewport {{ x: {}, y: {}, width: {}, height: {} }}", self.x, self.y, self.width, self.height)
    }
}

impl Viewport {
    pub fn new(x: i32, y: i32, width: u32, height: u32) -> Self {
        Self { x, y, width, height }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.width = width;
        self.height = height;
    }

    pub fn translate(&mut self, x: i32, y: i32) {
        self.x = x;
        self.y = y;
    }

    pub fn aspect_ratio(&self) -> f32 {
        if self.height == 0 {
            0.0
        } else {
            (self.width / self.height) as f32
        }
    }
}