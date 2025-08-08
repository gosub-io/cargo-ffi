use crate::net::Response;

// Loads an URL and returns the response in a result if any
pub async fn fetch(url: &str) -> Result<Response, reqwest::Error> {
    let client = reqwest::Client::new();
    let res = client.get(url).send().await?;

    // Fetch results
    let final_url = res.url().clone();
    let status = res.status().as_u16();
    let status_text = res.status().canonical_reason().unwrap_or("Unknown").to_string();
    let headers = res.headers().clone();

    // Fetch body. We don't do streaming yet
    let body = res.bytes().await?.to_vec();

    Ok(Response {
        url: final_url,
        status,
        status_text,
        headers,
        body,
    })
}