use std::collections::HashMap;

#[derive(Debug)]
pub struct Response {
    pub url: String,
    pub status: u16,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
}