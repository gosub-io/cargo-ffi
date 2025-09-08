use std::thread::{self, JoinHandle};
use std::sync::Arc;
use tokio::sync::{watch, mpsc};

use crate::net::fetcher::{Fetcher, FetcherConfig};
use crate::net::types::FetchRequest;

pub struct IoHandle {
    // Channel to submit fetch requests
    tx_submit: mpsc::UnboundedSender<FetchRequest>,
    // Send "true" when we want to shut down
    shutdown_tx: watch::Sender<bool>,
    // Join handle for shutdown sync
    join: Option<JoinHandle<()>>,
}

impl IoHandle {
    // pub fn submit(&self, req: FetchRequest) -> Result<(), ()> {
    //     self.tx_submit.send(req).map_err(|_| ())
    // }

    pub fn shutdown(mut self) {
        let _ = self.shutdown_tx.send(true);
        if let Some(j) = self.join.take() {
            let _ = j.join();
        }
    }

    pub fn subscribe(&self) -> mpsc::UnboundedSender<FetchRequest> {
        self.tx_submit.clone()
    }
}

pub fn spawn_io_thread(cfg: FetcherConfig) -> IoHandle {
    let (tx_submit, mut rx_submit) = mpsc::unbounded_channel::<FetchRequest>();
    let (shutdown_tx, mut shutdown_rx) = watch::channel(false);

    let join = thread::Builder::new()
        .name("I/O Thread".into())
        .spawn(move || {
            // Single-thread runtime => predictable & cheap context switches
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("io rt");

            rt.block_on(async move {
                let fetcher = Arc::new(Fetcher::new(cfg));
                let f = fetcher.clone();
                let stop_rx = shutdown_rx.clone();

                // Drive the scheduler
                tokio::task::Builder::new()
                    .name("I/O Fetcher Scheduler".into())
                    .spawn(async move {
                        f.run(stop_rx).await;
                    })
                    .expect("spawn fetcher");

                // Pump submissions coming from other threads into the fetcher's queues
                loop {
                    tokio::select! {
                        maybe_req = rx_submit.recv() => {
                            match maybe_req {
                                Some(req) => fetcher.submit(req).await,
                                None => break, // sender dropped -> end
                            }
                        }
                        _ = shutdown_rx.changed() => {
                            if *shutdown_rx.borrow() { break; }
                        }
                    }
                }
            });
        })
        .expect("spawn gosub-io");

    IoHandle { tx_submit, shutdown_tx, join: Some(join) }
}
