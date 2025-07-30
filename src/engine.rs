#[derive(Default)]
pub struct Engine {
    pub url: Option<String>,
    pub tick_count: usize,
}

impl Engine {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn load_url(&mut self, url: &str) {
        self.url = Some(url.to_string());
    }

    pub fn tick(&mut self) -> bool {
        self.tick_count += 1;
        true
    }

    pub fn render(&self) -> Vec<u8> {
        vec![0xFF, 0xFF, 0xFF, 0xFF]
    }
}
