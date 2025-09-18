// use crate::tab::TabId;
// use crate::engine::types::{EventChannel, TabChannel};
// use crate::tab::services::EffectiveTabServices;
//
// /// Arguments required to spawn a new tab task.
// #[derive(Debug)]
// pub struct TabSpawnArgs {
//     /// Tab Id
//     pub tab_id: TabId,
//
//     /// Receive channel for commands for the tab
//     pub cmd_rx: TabChannel,
//
//     /// Send channel for events from the tab to the UA
//     pub event_tx: EventChannel,
//
//     /// Services available to the tab
//     pub services: EffectiveTabServices,
// }
//
