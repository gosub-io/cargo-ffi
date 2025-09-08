//! This module defines the `Fetcher` struct and its associated functionality for managing
//! and scheduling HTTP requests. It includes mechanisms for prioritizing requests, coalescing
//! identical requests, and handling streaming or buffered responses.

use crate::net::events::NetObserver;
use crate::net::fetch::{fetch_response_complete, fetch_response_top, ResponseTop};
use crate::net::shared_body::SharedBody;
use crate::net::types::{FetchRequest, FetchResult, NetError, Priority};
use crate::net::utils::{short_url, Waiter};
use crate::util::spawn_named;
use bytes::{Bytes, BytesMut};
use dashmap::{DashMap, Entry};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;
use std::{collections::VecDeque, sync::Arc, time::Duration};
use tokio::io::AsyncReadExt;
use tokio::sync::{Notify, Semaphore};
use tokio::time::{sleep, timeout};
use url::Url;

/// How many shared consumers can listen for a resource
const SHARED_MAX_CAPACITY: usize = 32;

// Configuration for the fetcher
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
    /// Max time between reads allowed
    pub read_idle_timeout: Duration,
    /// Max time for total body read
    pub total_body_timeout: Option<Duration>,
}

impl Default for FetcherConfig {
    fn default() -> Self {
        Self {
            global_slots: 32,
            h1_per_origin: 6,
            h2_per_origin: 16,
            connect_timeout: Duration::from_secs(5),
            req_timeout: Duration::from_secs(60),
            read_idle_timeout: Duration::from_secs(15),
            total_body_timeout: Some(Duration::from_secs(180)),
        }
    }
}

/// RAII guard that ensures an in-flight coalescing entry is removed from the map.
///
/// # Why
/// When the **leader** task inserts an entry into `inflight` (to coalesce
/// identical requests), that entry **must** be removed even if the task exits
/// early (e.g., cancellation, early return, or panic). If it isn’t removed,
/// future requests may keep coalescing onto a “dead” entry and never make
/// progress.
///
/// `InflightGuard` holds a clone of the `DashMap` and the key. On `Drop` it
/// removes the entry **exactly once**. You can also remove eagerly via
/// [`remove`](Self::remove), which consumes the guard and disables the
/// drop-time removal.
///
/// # Semantics
/// - **Idempotent:** dropping after `remove()` is a no-op.
/// - **Panic-safe:** removal also runs during unwinding.
/// - **Non-owning of the value:** the guard only removes by key; it doesn’t
///   keep or access the mapped value.
///
/// # Example
/// ```rust,ignore,no_run
/// let guard = InflightGuard::new(inflight.clone(), key.clone());
/// // ... acquire permits, do the fetch ...
/// // Option A: explicit cleanup
/// guard.remove(); // consumes the guard
/// // Option B: rely on Drop (automatic cleanup at scope end)
/// ```
struct InflightGuard {
    map: Arc<DashMap<String, Arc<Inflight>>>,
    key: String,
    removed: bool,
}

impl InflightGuard {
    /// Create a new guard for `key` stored in `map`.
    ///
    /// This **does not** insert anything; it only arranges for the key to be
    /// removed when the guard is dropped (or when [`remove`](Self::remove) is called).
    #[inline]
    fn new(map: Arc<DashMap<String, Arc<Inflight>>>, key: String) -> Self {
        Self {
            map,
            key,
            removed: false
        }
    }


    /// Eagerly remove the in-flight entry and consume the guard.
    ///
    /// After this call, dropping the returned value (which no longer exists)
    /// will do nothing.
    #[inline]
    fn remove(mut self) {
        let _ = self.map.remove(&self.key);
        self.removed = true;
    }
}

impl Drop for InflightGuard {
    #[inline]
    fn drop(&mut self) {
        if !self.removed {
            let _ = self.map.remove(&self.key);
        }
    }
}

/// Represents an in-flight request, including its associated waiter and streaming preference.
struct Inflight {
    /// Waiter for managing requests
    waiter: Arc<Waiter>,
    /// True when streaming is required
    wants_streaming: AtomicBool,
}

impl Inflight {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            waiter: Waiter::new_arc(),
            wants_streaming: AtomicBool::new(false),
        })
    }
}

/// The `Fetcher` struct manages the scheduling and execution of HTTP requests.
/// It supports prioritization, coalescing of identical requests, and streaming or buffered responses.
pub struct Fetcher {
    /// HTTP client for making requests
    client: reqwest::Client,
    /// Configuration for the fetcher
    cfg: FetcherConfig,

    /// Semaphore for limiting global current fetches
    global_slots: Arc<Semaphore>,
    /// Map for managing per-origin limits
    per_origin: DashMap<String, Arc<Semaphore>>,

    /// Queue for high priority requests
    q_high: tokio::sync::Mutex<VecDeque<FetchRequest>>,
    /// Queue for regular priority requets
    q_norm: tokio::sync::Mutex<VecDeque<FetchRequest>>,
    /// Queue for low priority requests
    q_low: tokio::sync::Mutex<VecDeque<FetchRequest>>,
    /// Queue for idle priority requests
    q_idle: tokio::sync::Mutex<VecDeque<FetchRequest>>,

    /// Map for managing inflight requests and their associated waiters
    inflight: Arc<DashMap<String, Arc<Inflight>>>,

    /// Observer for fetch functionality to send back net events
    observer: Arc<dyn NetObserver + Send + Sync>,

    /// Notifier to wake up the fetcher when a new request is submitted
    wake: Notify,
}

impl Fetcher {
    /// Creates a new `Fetcher` instance with the given configuration and optional observer.
    pub fn new(config: FetcherConfig, observer: Option<Arc<dyn NetObserver + Send + Sync>>) -> Self {
        let observer: Arc<dyn NetObserver + Send + Sync> =
            observer.unwrap_or_else(|| Arc::new(crate::net::emitter::null_emitter::NullEmitter {}));

        // Start default client
        let client = reqwest::Client::builder()
            .connection_verbose(true)
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
            q_low: tokio::sync::Mutex::new(VecDeque::new()),
            q_idle: tokio::sync::Mutex::new(VecDeque::new()),
            inflight: Arc::new(DashMap::new()),
            observer,
            wake: Notify::new(),
        }
    }

    /// Returns the origin of a URL as a string key.
    fn origin_key(url: &Url) -> String {
        // Use origin (scheme + host + port) as key
        url.origin().ascii_serialization()
    }

    /// Pick the next request to process, using weighted round-robin across priority lanes.
    /// There are probably better schedulers, but this is simple and effective.
    fn pick_lane<'a>(
        &'a self,
        high: &'a mut VecDeque<FetchRequest>,
        norm: &'a mut VecDeque<FetchRequest>,
        low: &'a mut VecDeque<FetchRequest>,
        idle: &'a mut VecDeque<FetchRequest>,
        counter: &mut u8,
    ) -> Option<FetchRequest> {
        // Weighted round-robin: 8:4:2:1

        let slot = *counter as usize;
        *counter = (*counter + 1) % 15; // 8 + 4 + 2 + 1 = 15 slots

        let try_pop = |q: &mut VecDeque<FetchRequest>| q.pop_front();

        let pick = match slot {
            0..=7 => try_pop(high)
                .or_else(|| try_pop(norm))
                .or_else(|| try_pop(low))
                .or_else(|| try_pop(idle)),
            8..=11 => try_pop(norm)
                .or_else(|| try_pop(high))
                .or_else(|| try_pop(low))
                .or_else(|| try_pop(idle)),
            12..=13 => try_pop(low)
                .or_else(|| try_pop(norm))
                .or_else(|| try_pop(high))
                .or_else(|| try_pop(idle)),
            _ => try_pop(idle)
                .or_else(|| try_pop(low))
                .or_else(|| try_pop(norm))
                .or_else(|| try_pop(high)),
        };

        pick
    }

    /// Submit a fetch request to the appropriate priority lane.
    pub async fn submit(&self, req: FetchRequest) {
        log::debug!("Submitting fetch request: {:?}", req);
        let mut lane = match req.priority {
            Priority::High => self.q_high.lock().await,
            Priority::Normal => self.q_norm.lock().await,
            Priority::Low => self.q_low.lock().await,
            Priority::Idle => self.q_idle.lock().await,
        };
        lane.push_back(req);

        self.wake.notify_one();
    }

    /// Runs the fetcher, processing requests from the priority queues
    pub async fn run(&self, mut shutdown: tokio::sync::watch::Receiver<bool>) {
        let mut lane_counter: u8 = 0;

        loop {
            // Check for shutdown
            if *shutdown.borrow() {
                break;
            }

            // Pull next request (if any)
            let next = {
                let mut high = self.q_high.lock().await;
                let mut norm = self.q_norm.lock().await;
                let mut low = self.q_low.lock().await;
                let mut idle = self.q_idle.lock().await;
                self.pick_lane(&mut high, &mut norm, &mut low, &mut idle, &mut lane_counter)
            };

            // If none, wait for notifcation of new requests, or shutdown
            let Some(mut req) = next else {
                tokio::select! {
                    _ = self.wake.notified() => {},
                    _ = shutdown.changed() => {},
                }
                continue;
            };

            // Coalescing: if an identical request is in-flight, register and move on so we don't duplicate work
            let key_opt = req.key_data.generate();
            let key_str = match key_opt {
                Some(key_str) => {
                    // Found a key we can use for coalescing
                    key_str
                }
                None => {
                    // No key found we can use for coalescing. This can happen if we have non-safe requests like POST, PUT etc.
                    // We must generate a unique one so this request is not coalesced with anything else.
                    format!(
                        "{} {} @{}",
                        req.key_data.method,
                        req.key_data.url,
                        chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)
                    )
                }
            };

            let (inflight_entry, is_leader) = match self.inflight.entry(key_str.clone()) {
                Entry::Occupied(entry) => (entry.get().clone(), false),
                Entry::Vacant(v) => {
                    let arc = Inflight::new();
                    v.insert(arc.clone());
                    (arc, true)
                }
            };

            // Register this waiter to the shared Inflight.waiter
            if let Some(tx) = req.reply.take() {
                inflight_entry.waiter.register(tx, req.streaming).await;
            }

            // Escalate if this waiter needs streaming
            if req.streaming {
                inflight_entry
                    .wants_streaming
                    .store(true, Ordering::Relaxed);
            }

            // Followers are done; leader will spawn the fetch task
            if !is_leader {
                continue;
            }

            // Spawn a task to do the actual work
            let client = self.client.clone();
            let global = self.global_slots.clone();
            let per_origin = self.per_origin.clone();
            let cfg = self.cfg.clone();
            let inflight = self.inflight.clone();
            let key_str2 = key_str.clone();
            let inflight_entry2 = inflight_entry.clone();
            let observer_clone = self.observer.clone();
            let mut shutdown_child = shutdown.clone();

            let inflight_guard = InflightGuard::new(inflight.clone(), key_str2.clone());

            let title = format!("Fetcher: {}", short_url(&req.key_data.url, 80));
            let _ = spawn_named(&title, async move {
                let origin = Fetcher::origin_key(&req.key_data.url);
                let slots = per_origin
                    .entry(origin.clone())
                    .or_insert_with(|| Arc::new(Semaphore::new(per_origin_limit_for(&cfg, &req.key_data.url))))
                    .clone();

                let g = tokio::select! { p = global.acquire_owned() => Some(p), _ = shutdown_child.changed() => None };
                if g.is_none() { return; } // guard will drop -> cleanup

                let h = tokio::select! { p = slots.acquire_owned() => Some(p), _ = shutdown_child.changed() => None };
                if h.is_none() { return; } // guard will drop -> cleanup

                let should_stream = req.streaming || inflight_entry2.wants_streaming.load(Ordering::Relaxed);

                // Perform the request
                let result = if should_stream {
                    perform_streaming(&client, observer_clone.clone(), &req, &cfg).await
                } else {
                    perform_buffered(
                        &client,
                        observer_clone.clone(),
                        &req,
                        cfg.read_idle_timeout,
                        cfg.total_body_timeout,
                    )
                    .await
                };

                // Fanout to all listeners
                inflight_entry2.waiter.finish(result).await;

                // We can remove our inflight entry now
                inflight.remove(&key_str2);

                // Remove the guard
                inflight_guard.remove();
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
async fn perform_streaming(
    client: &reqwest::Client,
    observer: Arc<dyn NetObserver + Send + Sync>,
    req: &FetchRequest,
    cfg: &FetcherConfig,
) -> FetchResult {
    // Get the response top (headers + peek)
    match fetch_response_top(
        Arc::new(client.clone()),
        req.key_data.url.clone(),
        req.cancel.clone(),
        observer.clone(),
    )
    .await
    {
        Ok(ResponseTop { meta, peek, mut reader }) => {
            let shared = SharedBody::new(SHARED_MAX_CAPACITY);
            let shared_arc = Arc::new(shared);
            let shared_clone = shared_arc.clone();
            let cancel_clone = req.cancel.clone();
            let idle = cfg.read_idle_timeout;
            let total_deadline = cfg.total_body_timeout.map(|d| Instant::now() + d);

            tokio::spawn(async move {
                let mut buf = BytesMut::with_capacity(16 * 1024);

                loop {
                    let total_left = total_deadline.map(|dl| dl.saturating_duration_since(Instant::now()));

                    tokio::select! {
                        // Stream cancelled
                        _ = cancel_clone.cancelled() => {
                            shared_clone.error(NetError::Cancelled("stream read from perform_streaming".into()));
                            break;
                        }
                        // Total timeout (if any)
                        _ = async {
                            if let Some(rem) = total_left {
                                sleep(rem).await;
                            } else {
                                futures::future::pending::<()>().await;
                            }
                        } => {
                            shared_clone.error(NetError::Timeout("timeout".into()));
                            break;
                        }
                        // Idle timeout, or read
                        res = timeout(idle, reader.read_buf(&mut buf)) => {
                            match res {
                                Err(_) => {
                                    // Timeout ocurred
                                    shared_clone.error(NetError::Timeout("read idle timeout".into()));
                                    break;
                                }
                                Ok(Ok(0)) => {
                                    // Stream emptied.
                                    if !buf.is_empty() {
                                        let chunk = buf.split().freeze();
                                        shared_clone.push(chunk);
                                    }
                                    // Send eof to the readers
                                    shared_clone.finish();
                                    break;
                                },
                                Ok(Ok(_)) => {
                                    // Just send the chunk
                                    let chunk = buf.split().freeze();
                                    if !chunk.is_empty() {
                                        shared_clone.push(chunk);
                                    }
                                },
                                Ok(Err(e)) => {
                                    // Error ocurred during reading
                                    shared_clone.error(NetError::from_anyhow(e.into()));
                                    break;
                                },
                            }
                        }
                    }
                }
            });

            FetchResult::Stream {
                meta,
                peek,
                shared: shared_arc,
            }
        }

        Err(e) => FetchResult::Error(e),
    }
}


/// Perform an HTTP request using buffered mode
async fn perform_buffered(
    // Reqwest client
    client: &reqwest::Client,
    // Observer to emit NetEvents to
    observer: Arc<dyn NetObserver + Send + Sync>,
    // Actual request
    req: &FetchRequest,
    // Max timeout between reads
    read_idle_timeout: Duration,
    // Total timeout
    total_timeout: Option<Duration>,
) -> FetchResult {
    match fetch_response_complete(
        Arc::new(client.clone()),
        req.key_data.url.clone(),
        req.cancel.clone(),
        observer,
        req.max_bytes,
        read_idle_timeout,
        total_timeout,
    )
    .await
    {
        Ok((meta, body)) => FetchResult::Buffered {
            meta,
            body: Bytes::from(body),
        },
        Err(e) => FetchResult::Error(e),
    }
}
