mod handle;
mod worker;
mod tab;
mod structs;
mod options;
pub mod services;

use tokio::sync::oneshot;
pub use tab::TabId;
pub use handle::TabHandle;
pub use structs::TabSpawnArgs;

use tokio::task::JoinHandle;
use crate::tab::worker::TabWorker;

pub use options::TabDefaults;
pub use options::TabOverrides;
pub use options::TabCookieJar;
pub use options::TabCacheMode;
pub use options::TabStorageScope;
pub use structs::EffectiveTabServices;


/// Spawn a new tab task and acknowledge when it's ready via the provided oneshot channel.
pub(crate) fn spawn_tab_task(
    args: TabSpawnArgs,
    ack_channel: oneshot::Sender<anyhow::Result<()>>
) -> JoinHandle<()> {
    tokio::spawn(async move {
        match TabWorker::new(args).await {
            Ok(worker) => {
                let _ = ack_channel.send(Ok(()));
                worker.run().await;
            }
            Err(e) => {
                let _ = ack_channel.send(Err(e));
            }
        };
    })
}


