use std::collections::HashMap;

#[derive(Debug)]
pub struct Response {
    pub url: String,
    pub status: u16,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
}

pub async fn fetch(url: &str) -> Result<Response, reqwest::Error> {
    let client = reqwest::Client::new();
    let res = client.get(url).send().await?;
    let status = res.status().as_u16();
    let final_url = res.url().to_string();

    let headers = res
        .headers()
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
        .collect();

    let body = res.bytes().await?.to_vec();

    Ok(Response {
        url: final_url,
        status,
        headers,
        body,
    })
}