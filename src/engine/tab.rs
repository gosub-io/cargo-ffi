mod handle;
mod worker;
mod tab;
mod structs;

pub use tab::TabId;
pub use handle::TabHandle;
pub use structs::OpenTabParams;

use tokio::sync::oneshot::Sender;
use tokio::task::JoinHandle;
use crate::tab::structs::TabSpawnArgs;
use crate::tab::worker::TabWorker;


/// Spawn a new tab task and acknowledge when it's ready via the provided oneshot channel.
pub(crate) fn spawn_tab_task(args: TabSpawnArgs, ack_channel: Sender<anyhow::Result<()>>) -> JoinHandle<()> {
    tokio::spawn(async move {
        let worker = match TabWorker::new(args).await {
            Ok(w) => {
                let _ = ack_channel.send(Ok(()));
                w
            }
            Err(e) => {
                let _ = ack_channel.send(Err(e));
                return;
            }
        };

        worker.run().await;
    })
}


