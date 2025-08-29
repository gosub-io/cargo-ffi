/// Builder for creating a new tab within a zone.
pub struct TabBuilder {
    event_tx: Option<Sender<EngineEvent>>,
}

impl TabBuilder {
    pub fn new() -> Self {
        Self { event_tx: None }
    }

    pub fn with_event_tx(mut self, tx: Sender<EngineEvent>) -> Self {
        self.event_tx = Some(tx);
        self
    }

    pub(crate) fn take_event_tx(&mut self) -> Sender<EngineEvent> {
        self.event_tx.take().expect("TabBuilder event_tx already taken or not yet set")
    }
}