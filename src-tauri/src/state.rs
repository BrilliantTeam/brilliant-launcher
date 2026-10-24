use std::collections::{HashMap, HashSet};
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
    pub launching: std::sync::Mutex<HashSet<String>>,
}

pub struct LaunchClaim<'a> {
    state: &'a AppState,
    id: String,
}

impl Drop for LaunchClaim<'_> {
    fn drop(&mut self) {
        if let Ok(mut set) = self.state.launching.lock() {
            set.remove(&self.id);
        }
    }
}

impl AppState {
    pub async fn is_running(&self, instance_id: &str) -> bool {
        self.running.lock().await.contains_key(instance_id)
    }
    pub async fn claim(&self, instance_id: &str) -> Option<LaunchClaim<'_>> {
        let running = self.running.lock().await;
        let mut launching = self.launching.lock().ok()?;
        if running.contains_key(instance_id) || !launching.insert(instance_id.to_string()) {
            return None;
        }
        Some(LaunchClaim { state: self, id: instance_id.to_string() })
    }
    pub async fn ensure_idle(&self, instance_id: &str) -> crate::err::R<()> {
        let preparing = self.launching.lock().map(|s| s.contains(instance_id)).unwrap_or(false);
        if preparing || self.is_running(instance_id).await {
            crate::bail!("遊戲正在執行或準備中，檔案被遊戲使用著，請先關閉遊戲再進行這個操作");
        }
        Ok(())
    }
    pub async fn running_ids(&self) -> Vec<String> {
        self.running.lock().await.keys().cloned().collect()
    }
}
