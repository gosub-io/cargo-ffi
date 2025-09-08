use dashmap::DashMap;
use std::{collections::VecDeque, sync::Arc, time::Duration};
use tokio::sync::{Semaphore};
use url::Url;
use crate::net::types::{BodyStream, FetchKey, FetchRequest, FetchResult, NetError, NetResponseMeta, Priority};
use crate::net::utils::Waiter;

// Configuratin for the fetcher
#[derive(Clone)]
pub struct FetcherConfig {
    /// Maximum number of concurrent fetches overall
    pub global_slots: usize,
    /// Maximum number of concurrent HTTP/1 connections per origin
    pub h1_per_origin: usize,
    /// Maximum number of concurrent HTTP/2 connections per origin
    pub h2_per_origin: usize,
    /// TCP connect timeout
    pub connect_timeout: Duration,
    /// Overall request timeout
    pub req_timeout: Duration,
}

impl Default for FetcherConfig {
    fn default() -> Self {
        Self {
            // Some sane defaults
            global_slots: 32,
            h1_per_origin: 6,
            h2_per_origin: 16,
            connect_timeout: Duration::from_secs(5),
            req_timeout: Duration::from_secs(60),
        }
    }
}

pub struct Fetcher {
    /// HTTP client
    client: reqwest::Client,
    /// Configuration
    cfg: FetcherConfig,

    // Slot controls
    global_slots: Arc<Semaphore>,
    per_origin: DashMap<String, Arc<Semaphore>>,

    // Priority lanes
    q_high: tokio::sync::Mutex<VecDeque<FetchRequest>>,
    q_norm: tokio::sync::Mutex<VecDeque<FetchRequest>>,
    q_low:  tokio::sync::Mutex<VecDeque<FetchRequest>>,
    q_idle: tokio::sync::Mutex<VecDeque<FetchRequest>>,

    // Waiters for coalescing identical in-flight requests
    inflight: DashMap<String, Arc<Waiter>>,
}

impl Fetcher {
    pub fn new(config: FetcherConfig) -> Self {
        // Start default client
        let client = reqwest::Client::builder()
            .http2_adaptive_window(true)
            .connect_timeout(config.connect_timeout)
            .timeout(config.req_timeout)
            .use_rustls_tls()
            .build()
            .expect("reqwest client build failed");

        Self {
            client,
            cfg: config.clone(),
            global_slots: Arc::new(Semaphore::new(config.global_slots)),
            per_origin: DashMap::new(),
            q_high: tokio::sync::Mutex::new(VecDeque::new()),
            q_norm: tokio::sync::Mutex::new(VecDeque::new()),
            q_low:  tokio::sync::Mutex::new(VecDeque::new()),
            q_idle: tokio::sync::Mutex::new(VecDeque::new()),
            inflight: DashMap::new(),
        }
    }

    /// Returns the origin of an URL as a string key.
    fn origin_key(url: &Url) -> String {
        // Use origin (scheme + host + port) as key
        url.origin().ascii_serialization()
    }

    /// Returns a string key for identifying identical requests for coalescing.
    fn inflight_key(key: &FetchKey) -> String {
        // Use full URL as key; could add Vary-relevant bits if desired
        key.url.as_str().to_string()
    }

    /// Pick the next request to process, using weighted round-robin across priority lanes.
    /// There are probably better schedulers, but this is simple and effective.
    fn pick_lane<'a>(
        &'a self,
        high: &'a mut VecDeque<FetchRequest>,
        norm: &'a mut VecDeque<FetchRequest>,
        low: &'a mut VecDeque<FetchRequest>,
        idle: &'a mut VecDeque<FetchRequest>,
        counter: &mut u8
    ) -> Option<FetchRequest> {
        // Weighted round-robin: 8:4:2:1

        let slot = *counter as usize;
        *counter = (*counter + 1) % 15; // 8 + 4 + 2 + 1 = 15 slots

        let try_pop = |q: &mut VecDeque<FetchRequest>| q.pop_front();

        let pick = match slot {
            0..=7   => try_pop(high)
                .or_else(|| try_pop(norm))
                .or_else(|| try_pop(low))
                .or_else(|| try_pop(idle)),
            8..=11  => try_pop(norm)
                .or_else(|| try_pop(high))
                .or_else(|| try_pop(low))
                .or_else(|| try_pop(idle)),
            12..=13 => try_pop(low)
                .or_else(|| try_pop(norm))
                .or_else(|| try_pop(high))
                .or_else(|| try_pop(idle)),
            _       => try_pop(idle)
                .or_else(|| try_pop(low))
                .or_else(|| try_pop(norm))
                .or_else(|| try_pop(high)),
        };

        pick
    }

    /// Submit a fetch request to the appropriate priority lane.
    pub async fn submit(&self, req: FetchRequest) {
        let mut lane = match req.priority {
            Priority::High => self.q_high.lock().await,
            Priority::Normal => self.q_norm.lock().await,
            Priority::Low => self.q_low.lock().await,
            Priority::Idle => self.q_idle.lock().await,
        };
        lane.push_back(req);
    }

    pub async fn run(&self, mut shutdown: tokio::sync::watch::Receiver<bool>) {
        let mut lane_counter: u8 = 0;

        loop {
            // Check for shutdown
            if *shutdown.borrow() { break; }

            // Pull next request (if any)
            let next = {
                let mut high = self.q_high.lock().await;
                let mut norm = self.q_norm.lock().await;
                let mut low  = self.q_low.lock().await;
                let mut idle = self.q_idle.lock().await;
                self.pick_lane(&mut high, &mut norm, &mut low, &mut idle, &mut lane_counter)
            };

            // If none, wait a bit and check shutdown again
            let Some(mut req) = next else {
                tokio::select! {
                    _ = tokio::time::sleep(Duration::from_millis(2)) => {},
                    _ = shutdown.changed() => {},
                }
                continue;
            };

            // Coalescing: if an identical request is in-flight, register and move on so we don't duplicate work
            let key_str = Self::inflight_key(&req.key);
            if let Some(waiter) = self.inflight.get(&key_str) {
                // only coalesce if *not yet started streaming*; here we assume not started
                if let Some(tx) = req.reply {
                    waiter.register(tx).await;
                }
                continue;
            }

            // Reserve waiter
            let waiter = Waiter::new();
            if let Some(tx) = req.reply.take() {
                waiter.register(tx).await;
            }
            self.inflight.insert(key_str.clone(), waiter.clone());

            // Spawn a task to do the actual work
            let client = self.client.clone();
            let global = self.global_slots.clone();
            let per_origin = self.per_origin.clone();
            let cfg = self.cfg.clone();
            let inflight = self.inflight.clone();
            let key_str2 = key_str.clone();
            let waiter2 = waiter.clone();
            let req = req;

            let _ = tokio::task::Builder::new()
                .name(&format!("Fetcher: {}", req.key.url.to_string()))
                .spawn(async move {
                    let origin = Fetcher::origin_key(&req.key.url);
                    let slots = per_origin
                        .entry(origin.clone())
                        .or_insert_with(|| Arc::new(Semaphore::new( per_origin_limit_for(&cfg, &req.key.url) )))
                        .clone();

                    // Acquire slots (drops when _guards go out of scope)
                    let _g = global.acquire().await.ok();
                    let _h = slots.acquire().await.ok();

                    // Perform the request
                    let result = perform(&client, &req).await;

                    // Fanout to all listeners
                    waiter2.finish(result).await;

                    // We can remove our inflight entry now
                    inflight.remove(&key_str2);
                });
        }
    }
}

// Choose per-origin limit based on scheme/alpn (rough heuristic here).
fn per_origin_limit_for(cfg: &FetcherConfig, url: &Url) -> usize {
    match url.scheme() {
        // reqwest will use h2 when it can; safe cap
        "http" | "https" => cfg.h2_per_origin,
        _ => cfg.h1_per_origin,
    }
}

/// Perform the actual HTTP request using reqwest.
async fn perform(client: &reqwest::Client, req: &FetchRequest) -> FetchResult {
    // @TODO: Expand with headers, credentials, redirects, cache, etc.
    let resp = match client.get(req.key.url.clone()).send().await {
        Ok(r) => r,
        Err(e) => return FetchResult::Error(NetError::Reqwest(e.into())),
    };

    let status = resp.status();
    let meta = NetResponseMeta {
        final_url: resp.url().clone(),
        status: status.as_u16(),
        reason: status.canonical_reason().unwrap_or("").to_string(),
        headers: resp.headers().clone(),
    };

    if req.streaming {
        let stream = resp.bytes_stream();
        let body: BodyStream = Box::pin(stream);
        FetchResult::Stream { meta, body }
    } else {
        match resp.bytes().await {
            Ok(b) => FetchResult::Buffered { meta, body: b },
            Err(e) => FetchResult::Error(NetError::Reqwest(e.into())),
        }
    }
}
