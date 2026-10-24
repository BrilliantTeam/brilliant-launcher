use crate::bail;
use crate::err::R;
use crate::paths;
use crate::state::AppState;
use anyhow::Context;
use serde::Serialize;
use std::path::{Path, PathBuf};
use tauri::State;

const KEEP: usize = 5;
const QUIET_SECS: u64 = 600;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub key: String,
    pub time: i64,
    pub count: usize,
    pub size: u64,
}

fn dir(id: &str) -> PathBuf {
    paths::instance_dir(id).join("snapshots")
}

fn valid_key(key: &str) -> bool {
    key.starts_with("mods-") && key.len() <= 40 && key.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

fn entries(base: &Path) -> Vec<(std::time::SystemTime, PathBuf)> {
    let mut list: Vec<(std::time::SystemTime, PathBuf)> = std::fs::read_dir(base)
        .map(|d| d.flatten().filter(|e| valid_key(&e.file_name().to_string_lossy())).filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.path()))).collect())
        .unwrap_or_default();
    list.sort_by_key(|e| std::cmp::Reverse(e.0));
    list
}

fn take_in(mods: &Path, base: &Path, now: u64) -> std::io::Result<bool> {
    if !mods.is_dir() {
        return Ok(false);
    }
    if let Some((newest, _)) = entries(base).first() {
        if newest.elapsed().map(|e| e.as_secs() < QUIET_SECS).unwrap_or(true) {
            return Ok(false);
        }
    }
    let target = base.join(format!("mods-{now}"));
    crate::importer::copy_tree(mods, &target)?;
    for (_, old) in entries(base).into_iter().skip(KEEP) {
        let _ = std::fs::remove_dir_all(old);
    }
    Ok(true)
}

pub async fn take(id: &str) {
    if !crate::config::load_settings().await.mod_snapshots {
        return;
    }
    let (mods, base) = (paths::game_dir(id).join("mods"), dir(id));
    let now = crate::config::now_secs().max(0) as u64;
    let _ = tokio::task::spawn_blocking(move || take_in(&mods, &base, now)).await;
}

#[tauri::command]
pub async fn list_mod_snapshots(id: String) -> Vec<Snapshot> {
    entries(&dir(&id))
        .into_iter()
        .map(|(time, path)| Snapshot {
            key: path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(),
            time: time.duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0),
            count: std::fs::read_dir(&path).map(|d| d.flatten().filter(|e| e.path().is_file()).count()).unwrap_or(0),
            size: crate::worlds::dir_size(&path),
        })
        .collect()
}

#[tauri::command]
pub async fn restore_mod_snapshot(state: State<'_, AppState>, id: String, key: String) -> R<()> {
    state.ensure_idle(&id).await?;
    if !valid_key(&key) {
        bail!("找不到這個快照");
    }
    let source = dir(&id).join(&key);
    if !source.is_dir() {
        bail!("找不到這個快照");
    }
    let mods = paths::game_dir(&id).join("mods");
    let name = crate::instance::get(&id).await.map(|i| i.name).unwrap_or_else(|_| id.clone());
    tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
        if mods.is_dir() {
            paths::discard(&format!("還原快照前的模組 {name}"), &[(&mods, "mods")]).context("把目前模組移到回收資料夾失敗")?;
        }
        crate::importer::copy_tree(&source, &mods).context("還原快照失敗")?;
        Ok(())
    })
    .await
    .context("還原工作中斷")??;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshots_are_throttled_and_capped() {
        let base = std::env::temp_dir().join(format!("bl-snap-{}", rand::random::<u32>()));
        let mods = base.join("mods");
        std::fs::create_dir_all(&mods).unwrap();
        std::fs::write(mods.join("a.jar"), b"jar").unwrap();
        let snaps = base.join("snapshots");
        assert!(take_in(&mods, &snaps, 1).unwrap());
        assert!(!take_in(&mods, &snaps, 2).unwrap(), "第二次在 10 分鐘內不重複拍");
        assert!(snaps.join("mods-1/a.jar").is_file());
        assert!(!take_in(&base.join("none"), &snaps, 3).unwrap());
        assert!(valid_key("mods-123") && !valid_key("../mods") && !valid_key("saves-1"));
        std::fs::remove_dir_all(&base).unwrap();
    }
}
