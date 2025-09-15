use tokio::{io::AsyncRead, time::{timeout, sleep}};
use bytes::{Bytes, BytesMut};
use std::{path::PathBuf, time::Instant};
use tokio::fs::OpenOptions;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufWriter};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use url::Url;
use crate::net::events::{NetEvent, NetObserver};
use crate::net::fs_utils::temp_path_for;
use crate::net::SharedBody;
use crate::net::types::NetError;


/// Pump Configuration
pub struct PumpCfg {
    /// Idle timeout
    pub idle: std::time::Duration,
    /// Total timeout
    pub total_deadline: Option<Instant>,
}

/// Pump targets. Each have either a shared body, or a file destination.
pub struct PumpTargets {
    // Shared body
    pub shared: Option<Arc<SharedBody>>,
    // Optional file destination
    pub file_dest: Option<PathBuf>,
    // peek buffer we need to send first
    pub peek: Vec<u8>
}

/// Spawns a single pump task that will optionally write to sharedbody and file.
/// Will honor idle and total timeouts + cancellations
pub fn spawn_pump<R>(
    // Reader we pump from
    mut reader: R,
    targets: PumpTargets,
    cfg: PumpCfg,
    cancel: CancellationToken,
    observer: Arc<dyn NetObserver>,
    url: Url,
) -> JoinHandle<Result<Option<PathBuf>, NetError>>
where
    R: AsyncRead + Unpin + Send + 'static
{
    let PumpTargets { shared, file_dest, peek } = targets;
    let idle = cfg.idle;
    let total_deadline = cfg.total_deadline;

    tokio::spawn(async move {
        // If we need to send to file, first open the file and write the peek data
        let mut writer = if let Some(dest) = &file_dest {
            let tmp_dest = match temp_path_for(dest) {
                Ok(p) => p,
                Err(e) => {
                    return Err(NetError::Io(Arc::new(e)));
                }
            };

            let mut f = OpenOptions::new()
                .create(true)
                .truncate(true)
                .write(true)
                .open(tmp_dest.path())
                .await
                .map_err(|e| NetError::Io(Arc::new(e)))?;

            // Write peek data first
            if !peek.is_empty() {
                f.write_all(&peek).await.map_err(|e| NetError::Io(Arc::new(e)))?;
            }

            Some ((tmp_dest, BufWriter::new(f)))
        } else {
            None
        };

        // Next, push the peek data to the shared first
        if let Some(s) = &shared {
            if !peek.is_empty() {
                s.push(Bytes::from(peek.clone()));
            }
        }

        // Peek writes are done. Continue with the main loop that deals with the stream
        let mut buf = BytesMut::with_capacity(16 * 1024);
        let finish_ok = loop {
            let total_left = total_deadline.map(|dl| dl.saturating_duration_since(Instant::now()));

            let read_res = tokio::select!{
                _ = cancel.cancelled() => {
                    // Cancelled
                    if let Some(s) = &shared {
                        s.error(NetError::Cancelled("Pump cancelled".into()));
                    }
                    break false;
                }
                _ = async {
                    // Wait for total time to expire, if set
                    if let Some(rem) = total_left {
                        sleep(rem).await
                    } else {
                        futures::future::pending::<()>().await
                    }
                } => {
                    if let Some(s) = &shared {
                        s.error(NetError::Timeout("Pump total timeout".into()));
                    }
                    break false;
                }
                r = timeout(idle, reader.read_buf(&mut buf)) => r,
            };

            match read_res {
                Err(_) => {
                    // Error means timeout
                    if let Some(s) = &shared {
                        s.error(NetError::Timeout("Pump idle timeout".into()));
                        break false;
                    }
                }
                Ok(Ok(0)) => {
                    // zero bytes read means EOF
                    if !buf.is_empty() {
                        let chunk = buf.split().freeze();

                        // Write chunk to shared body
                        if let Some(s) = &shared {
                            s.push(chunk.clone());
                        }

                        // Write to file
                        if let Some((_tmp, w)) = &mut writer {
                            if let Err(e) = w.write_all(&chunk).await {
                                observer.on_event(NetEvent::Io {
                                    message: format!("Failed to write to file: {}", e),
                                });
                            }
                        }

                        // Finish the shared body
                        if let Some(s) = &shared {
                            s.finish();
                        }

                        // Finally, flush the file
                        if let Some((_tmp, w)) = &mut writer {
                            if let Err(e) = w.flush().await {
                                observer.on_event(NetEvent::Warning {
                                    url: url.clone(),
                                    message: format!("Failed to flush file: {}", e)
                                });
                            }
                        }
                    }
                    break true;
                }

                Ok(Ok(_)) => {
                    // Data received
                    let chunk = buf.split().freeze();
                    if !chunk.is_empty() {
                        // Send to shared body
                        if let Some(s) = &shared {
                            s.push(chunk.clone());
                        }

                        // Send to file
                        if let Some((_tmp, w)) = &mut writer {
                            if let Err(e) = w.write_all(&chunk).await {
                                observer.on_event(NetEvent::Warning {
                                    url: url.clone(),
                                    message: format!("Failed to write to file: {}", e),
                                });
                            }
                        }
                    }
                }
                Ok(Err(e)) => {
                    // Error reading, send error to shared body. Nothing to be done for the file
                    if let Some(s) = &shared {
                        s.error(NetError::Io(Arc::new(e)));
                    }
                    break false;
                }
            }
        };

        // If we wrote to a file, and finished ok, rename the temp file to the final destination
        if let Some((tmp, _w)) = writer {
            if finish_ok {
                if let Some(dest) = file_dest {
                    tokio::fs::rename(&tmp, &dest).await.map_err(|e| NetError::Io(Arc::new(e)))?;

                    return Ok(Some(dest));
                }
            }
        }

        // @TODO: if not ok, we probably want to remove the temp file?

        Ok(None)
    })
}