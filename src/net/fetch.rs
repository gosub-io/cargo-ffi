use anyhow::{anyhow, Context, Result};
use futures_util::StreamExt;
use std::time::Instant;
use tokio_util::sync::CancellationToken;
use url::Url;
use crate::net::emitter::null_emitter::NullEmitter;
use crate::net::events::{NetEvent, NetObserver};
use crate::net::Response;

// Fetch function with observer to which we can send net events to.
pub async fn fetch(
    url: Url,
    cancel: CancellationToken,
    obs: Option<&dyn NetObserver>,
) -> Result<Response> {
    let obs = obs.unwrap_or(&NullEmitter {});

    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::custom({
            // It's safe to assume that our net observer is static. It won't be freed too early
            let obs = unsafe { std::mem::transmute::<&dyn NetObserver, &'static dyn NetObserver>(obs) };

            move |attempt| {
                if let (Some(prev), next) = (attempt.previous().last(), attempt.url()) {
                    // Send a net event that we encountered a redirection
                    let from = prev.clone();
                    let to = next.clone();
                    let status = attempt.status().as_u16();
                    obs.on_event(NetEvent::Redirected {
                        from,
                        to,
                        status,
                    });
                }
                attempt.follow()
            }
        }))
        .build()?;

    // Start
    obs.on_event(NetEvent::Started { url: url.clone() });

    let started = Instant::now();

    // Build the request future and pin it so we can drop on cancel.
    let req_fut = client.get(url.clone()).send();
    tokio::pin!(req_fut);

    let resp = tokio::select! {
        _ = cancel.cancelled() => {
            // Request was cancelled
            obs.on_event(NetEvent::Cancelled { url: url.clone(), reason: "cancelled" });
            return Err(anyhow!("cancelled"));
        }
        r = &mut req_fut => r.context("request send failed")?,
    };

    let status = resp.status().as_u16();
    let headers = resp.headers().clone();
    let content_type = headers
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());
    let content_len = resp.content_length();

    // Response headers found which we can emit
    obs.on_event(NetEvent::ResponseHeaders {
        url: resp.url().clone(),
        status,
        content_length: content_len,
        content_type: content_type.clone(),
    });

    // Stream the body to emit progress, but also buffer to Vec<u8> to match your Response.
    let mut body_stream = resp.bytes_stream();
    let mut received: u64 = 0;
    let mut buf: Vec<u8> = Vec::with_capacity(content_len.unwrap_or(0) as usize);

    loop {
        tokio::select! {
            _ = cancel.cancelled() => {
                // Stream cancelled
                obs.on_event(NetEvent::Cancelled { url: url.clone(), reason: "body stream cancelled" });
                return Err(anyhow!("body stream cancelled"));
            }
            next = body_stream.next() => {
                match next {
                    Some(Ok(chunk)) => {
                        received += chunk.len() as u64;
                        buf.extend_from_slice(&chunk);
                        // Emit progress. We probably want to see how chatty this will be, based on chunk.len()
                        obs.on_event(NetEvent::Progress { received_bytes: received });
                    }
                    Some(Err(e)) => {
                        // Something failed
                        obs.on_event(NetEvent::Failed { url: url.clone(), error: e.to_string() });
                        return Err(e).context("body read failed");
                    }
                    None => break, // done
                }
            }
        }
    }

    let elapsed = started.elapsed();

    // Request is finished
    obs.on_event(NetEvent::Finished {
        url: url.clone(),
        bytes: received,
        elapsed,
        content_type: content_type.clone(),
    });

    // Create actual response to caller
    let final_url = url;
    let status_text = reqwest::StatusCode::from_u16(status)
        .ok()
        .map(|s| s.canonical_reason().unwrap_or(""))
        .unwrap_or("")
        .to_string();

    let mut headers_map = reqwest::header::HeaderMap::new();
    headers_map.extend(headers.clone().into_iter());

    Ok(Response {
        url: final_url,
        status,
        status_text,
        headers: headers_map,
        body: buf,
    })
}