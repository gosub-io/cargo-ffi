use std::sync::atomic::AtomicU64;
use tokio::sync::oneshot;
use crate::Action;

#[derive(Clone, Copy, Debug, Hash, Eq, PartialEq)]
pub struct DecisionToken(u64);

pub struct DecisionHub {
    waiters: dashmap::DashMap<DecisionToken, oneshot::Sender<Action>>,
    counter: AtomicU64,
}

impl DecisionHub {
    pub fn new() -> Self {
        Self {
            waiters: dashmap::DashMap::new(),
            counter: AtomicU64::new(0),
        }
    }

    pub fn new_token(&self) -> DecisionToken {
        DecisionToken(self.counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed))
    }

    pub fn register(&self) -> (DecisionToken, oneshot::Receiver<Action>) {
        let token = DecisionToken(self.counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed));
        let (tx, rx) = oneshot::channel();
        self.waiters.insert(token, tx);
        (token, rx)
    }

    pub fn fulfill(&self, token: DecisionToken, action: Action) -> Result<(), Action> {
        if let Some((_, tx)) = self.waiters.remove(&token) {
            tx.send(action).map_err(|a| a)
        } else {
            Err(action)
        }
    }
}