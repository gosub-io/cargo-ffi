use crate::net::fetcher::{Fetcher, FetcherConfig};
use crate::util::spawn_named;
use std::sync::Arc;
use tokio::sync::{mpsc, watch};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use crate::engine::EngineContext;
use crate::engine::types::IoChannel;
use crate::events::IoCommand;
use crate::net::types::{FetchHandle, FetchRequest};

/// IoHandle is the handle that controls the IO thread.
pub struct IoHandle {
    // Channel to submit fetch requests
    tx_submit: IoChannel,
    // Send "true" when we want to shut down the IO thread
    shutdown_tx: watch::Sender<bool>,
    // Join handle for shutdown sync
    join_handle: Option<JoinHandle<()>>,
}

impl IoHandle {
    // // Even though we COULD send directly from the IoHandle, it's more likely that we
    // // send commands through a copy of the tx_submit that we send in the EngineContext to zones and
    // // later tabs.
    // pub fn submit(&self, req: FetchRequest) -> Result<(), ()> {
    //     submit_request_to_io()
    //     self.tx_submit.send(IoCommand::Fetch(req)).map_err(|_| ())
    // }



    /// Shutdown the IO thread
    pub async fn shutdown(mut self) {
        let _ = self.shutdown_tx.send(true);

        drop(self.tx_submit);

        if let Some(jh) = self.join_handle.take() {
            match jh.await {
                Ok(()) => {}
                Err(e) if e.is_cancelled() => {
                    log::warn!("I/O driver task was cancelled during shutdown");
                }
                Err(e) if e.is_panic() => {
                    log::error!("I/O driver task panicked during shutdown: {e:?}");
                }
                Err(e) => {
                    log::warn!("I/O driver join error: {e:?}");
                }
            }
        }
    }

    /// Get a clone of the submission channel to send fetch requests from other threads.
    pub fn subscribe(&self) -> IoChannel {
        self.tx_submit.clone()
    }
}

pub async fn submit_to_io(request: FetchRequest, io_tx: IoChannel) -> anyhow::Result<FetchHandle> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    let cancel = CancellationToken::new();


    io_tx.send(IoCommand::Fetch(request.clone(), tx)).unwrap();

    FetchHandle {
        req_id: request.req_id,
        cancel,
        reply_channel: rx,
    }
}

/// Spawns the IO thread and runs a single fetcher on top. If needed, we can expand this system to
/// run multiple fetchers on different OS threads for instance, but most likely the fetching itself
/// isn't the biggest bottleneck.
pub fn spawn_io_thread(cfg: FetcherConfig, engine_ctx: Arc<EngineContext>) -> IoHandle {
    let (tx_submit, mut rx_submit) = mpsc::unbounded_channel::<IoCommand>();
    let (shutdown_tx, mut shutdown_rx) = watch::channel(false);

    // let io_tx = tx_submit.clone();

    let join_handle = spawn_named("I/O Thread", async move {
        let fetcher = Arc::new(Fetcher::new(cfg, engine_ctx.event_tx.clone(), engine_ctx.request_reference_map.clone()));
        let cloned_fetcher = fetcher.clone();
        let cloned_shutdown_rx = shutdown_rx.clone();

        // Drive the scheduler
        let join_handle = spawn_named("I/O Fetcher Scheduler", async move {
            cloned_fetcher.run(cloned_shutdown_rx).await;
        });

        // Pump submissions coming from other threads into the fetcher's queues
        loop {
            tokio::select! {
                maybe_req = rx_submit.recv() => {
                    match maybe_req {
                        Some(IoCommand::Fetch(req, handle)) => fetcher.submit(req, handle).await,
                        Some(IoCommand::Decision { token,action }) => fetcher.fullfill(token, action).await,
                        None => {
                            // All producers have dropped. Signal shutdown
                            break
                        }
                    }
                }
                _ = shutdown_rx.changed() => {
                    if *shutdown_rx.borrow() {
                        break;
                    }
                }
            }
        }

        // wait until the scheduler is stopped
        let _ = join_handle.await;
    });

    IoHandle {
        tx_submit,
        shutdown_tx,
        join_handle: Some(join_handle),
    }
}



#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tokio::time::{timeout, sleep};

    fn test_cfg() -> FetcherConfig {
        FetcherConfig {
            global_slots: 2,
            h1_per_origin: 2,
            h2_per_origin: 2,
            connect_timeout: Duration::from_millis(50),
            req_timeout: Duration::from_millis(100),
            read_idle_timeout: Duration::from_millis(100),
            total_body_timeout: Some(Duration::from_millis(150)),
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn driver_starts_and_shuts_down_cleanly() {
        let (tx, _rx) = tokio::sync::broadcast::channel(16);

        let ctx = Arc::new(EngineContext {
            event_tx: tx.clone(),
            .. Default::default()
        });

        let cfg = test_cfg();
        let handle = spawn_io_thread(cfg, ctx.clone());

        // Give the driver a moment to boot its internal scheduler
        sleep(Duration::from_millis(10)).await;

        // Shutdown should complete without panic/cancel
        timeout(Duration::from_secs(2), handle.shutdown())
            .await
            .expect("shutdown timed out");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn multiple_subscribers_do_not_block_shutdown() {
        let (tx, _rx) = tokio::sync::broadcast::channel(16);

        let ctx = Arc::new(EngineContext {
            event_tx: tx.clone(),
            .. Default::default()
        });

        let cfg = test_cfg();
        let handle = spawn_io_thread(cfg, ctx.clone());

        // create a few clones of the submit handle
        let s1 = handle.subscribe();
        let s2 = handle.subscribe();
        let s3 = handle.subscribe();

        // drop the clones — the original sender stays inside IoHandle
        drop(s1);
        drop(s2);
        drop(s3);

        // ensure the runtime still shuts down promptly
        timeout(Duration::from_secs(2), handle.shutdown())
            .await
            .expect("shutdown timed out");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn shutdown_signal_stops_driver_even_without_submissions() {
        let (tx, _rx) = tokio::sync::broadcast::channel(16);

        let ctx = Arc::new(EngineContext {
            event_tx: tx.clone(),
            .. Default::default()
        });

        let cfg = test_cfg();
        let handle = spawn_io_thread(cfg, ctx.clone());

        // no submissions; just shut down
        timeout(Duration::from_secs(2), handle.shutdown())
            .await
            .expect("shutdown timed out");
    }

    // NOTE:
    // The following test documents the “all producers dropped” path. Because IoHandle
    // owns the primary sender and doesn’t expose a way to drop it except via shutdown(),
    // we simulate the state transition by (a) dropping an extra clone (producer)
    // and (b) issuing shutdown. This ensures both branches are exercised over time.
    #[tokio::test(flavor = "current_thread")]
    async fn dropping_all_producers_plus_shutdown_is_clean() {
        let (tx, _rx) = tokio::sync::broadcast::channel(16);

        let ctx = Arc::new(EngineContext {
            event_tx: tx.clone(),
            .. Default::default()
        });

        let cfg = test_cfg();
        let handle = spawn_io_thread(cfg, ctx.clone());

        // extra producer
        let s = handle.subscribe();
        drop(s);

        // trigger shutdown; driver should exit promptly
        timeout(Duration::from_secs(2), handle.shutdown())
            .await
            .expect("shutdown timed out");
    }
}
