use std::future::Future;
use tokio::sync::mpsc;

use crate::app::AppAction;

#[derive(Clone)]
pub struct TaskManager {
    action_tx: mpsc::Sender<AppAction>,
}

impl TaskManager {
    pub const fn new(action_tx: mpsc::Sender<AppAction>) -> Self {
        Self { action_tx }
    }

    pub fn spawn<F>(&self, future: F)
    where
        F: Future<Output = AppAction> + Send + 'static,
    {
        let tx = self.action_tx.clone();
        tokio::spawn(async move {
            let action = future.await;
            let _ = tx.send(action).await;
        });
    }

    pub async fn send(&self, action: AppAction) {
        let _ = self.action_tx.send(action).await;
    }
}
