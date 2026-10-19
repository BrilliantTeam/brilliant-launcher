use std::collections::HashMap;
use tokio::sync::{oneshot, Mutex};

pub struct RunningGame {
    pub started_at: i64,
    pub stop: Option<oneshot::Sender<()>>,
}

#[derive(Default)]
pub struct AppState {
    pub running: Mutex<HashMap<String, RunningGame>>,
    pub accounts_lock: Mutex<()>,
    pub prep_lock: Mutex<()>,
}

impl AppState {
    pub async fn is_running(&self, instance_id: &str) -> bool {
        self.running.lock().await.contains_key(instance_id)
    }
    pub async fn running_ids(&self) -> Vec<String> {
        self.running.lock().await.keys().cloned().collect()
    }
}
