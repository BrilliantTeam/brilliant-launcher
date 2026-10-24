use crate::bail;
use crate::err::R;
use crate::paths;
use crate::state::AppState;
use anyhow::Context;
use serde::Serialize;
use std::path::{Path, PathBuf};
use tauri::State;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Part {
    pub label: String,
    pub size: u64,
    pub path: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceUsage {
    pub id: String,
    pub name: String,
    pub total: u64,
    pub parts: Vec<Part>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Suggestion {
    pub key: String,
    pub label: String,
    pub detail: String,
    pub size: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Usage {
    pub total: u64,
    pub shared: Vec<Part>,
    pub instances: Vec<InstanceUsage>,
    pub suggestions: Vec<Suggestion>,
}

const INSTANCE_PARTS: [(&str, &str); 9] = [
    ("saves", "存檔"),
    ("mods", "模組"),
    ("resourcepacks", "資源包"),
    ("shaderpacks", "光影"),
    ("screenshots", "截圖"),
    ("logs", "日誌"),
    ("crash-reports", "當機報告"),
    ("config", "模組設定"),
    ("versions", "實例專屬版本"),
];

fn size(path: &Path) -> u64 {
    match std::fs::symlink_metadata(path) {
        Ok(m) if m.is_dir() => crate::worlds::dir_size(path),
        Ok(m) if m.is_file() => m.len(),
        _ => 0,
    }
}

fn part(label: &str, path: PathBuf) -> Part {
    Part { label: label.to_string(), size: size(&path), path: path.to_string_lossy().to_string() }
}

fn old_files(dir: &Path, days: u64) -> Vec<PathBuf> {
    let limit = std::time::SystemTime::now() - std::time::Duration::from_secs(days * 86_400);
    std::fs::read_dir(dir)
        .map(|d| d.flatten().filter(|e| e.path().is_file() && e.metadata().and_then(|m| m.modified()).map(|t| t < limit).unwrap_or(false)).map(|e| e.path()).collect())
        .unwrap_or_default()
}

pub fn unused_versions(dirs: &[String], instances: &[(String, String)]) -> Vec<String> {
    dirs.iter()
        .filter(|name| !instances.iter().any(|(mc, id)| *name == mc || *name == id || name.ends_with(&format!("-{mc}"))))
        .cloned()
        .collect()
}

async fn old_logs(instances: &[crate::instance::Instance]) -> Vec<PathBuf> {
    let mut list = old_files(&paths::logs(), 7);
    for i in instances {
        let game = paths::game_dir(&i.id);
        list.extend(old_files(&game.join("logs"), 14).into_iter().filter(|p| p.file_name().is_some_and(|n| n != "latest.log")));
        list.extend(old_files(&game.join("crash-reports"), 30));
    }
    list
}

fn version_dirs() -> Vec<String> {
    std::fs::read_dir(paths::versions())
        .map(|d| d.flatten().filter(|e| e.path().is_dir()).map(|e| e.file_name().to_string_lossy().to_string()).collect())
        .unwrap_or_default()
}

async fn stale_versions(instances: &[crate::instance::Instance]) -> Vec<String> {
    let used: Vec<(String, String)> = instances.iter().map(|i| (i.mc_version.clone(), i.version_id.clone())).collect();
    if used.is_empty() {
        return Vec::new();
    }
    unused_versions(&version_dirs(), &used)
}

#[tauri::command]
pub async fn disk_usage() -> R<Usage> {
    let instances = crate::instance::list().await;
    let logs = old_logs(&instances).await;
    let versions = stale_versions(&instances).await;
    let ids: Vec<(String, String)> = instances.iter().map(|i| (i.id.clone(), i.name.clone())).collect();
    let usage = tokio::task::spawn_blocking(move || {
        let shared_root = paths::shared();
        let shared = vec![
            part("遊戲版本", paths::versions()),
            part("函式庫", paths::libraries()),
            part("遊戲素材", paths::assets()),
            part("Java 執行環境", paths::java_root()),
            part("共用資料夾", shared_root.join("folders")),
            part("輝煌模組", shared_root.join("brilliant-mods")),
            part("暫存（natives）", shared_root.join("natives")),
            part("圖片與音樂", paths::root().join("media")),
            part("範本", paths::root().join("templates")),
            part("縮圖快取", crate::insight::thumbs_dir()),
            part("啟動器日誌", paths::logs()),
            part("回收資料夾", paths::trash()),
        ];
        let mut list: Vec<InstanceUsage> = ids
            .into_iter()
            .map(|(id, name)| {
                let game = paths::game_dir(&id);
                let mut parts: Vec<Part> = INSTANCE_PARTS.iter().map(|(dir, label)| part(label, game.join(dir))).collect();
                let base = paths::instance_dir(&id);
                parts.push(part("存檔備份", base.join("backups")));
                parts.push(part("模組快照", base.join("snapshots")));
                let total = size(&base);
                let known: u64 = parts.iter().map(|p| p.size).sum();
                parts.push(Part { label: "其他".into(), size: total.saturating_sub(known), path: game.to_string_lossy().to_string() });
                parts.retain(|p| p.size > 0);
                parts.sort_by_key(|p| std::cmp::Reverse(p.size));
                InstanceUsage { id, name, total, parts }
            })
            .collect();
        list.sort_by_key(|i| std::cmp::Reverse(i.total));
        let log_size: u64 = logs.iter().map(|p| size(p)).sum();
        let version_size: u64 = versions.iter().map(|v| size(&paths::versions().join(v))).sum();
        let snapshot_size: u64 = list.iter().flat_map(|i| i.parts.iter()).filter(|p| p.label == "模組快照").map(|p| p.size).sum();
        let mut suggestions = vec![
            Suggestion { key: "trash".into(), label: "清空回收資料夾".into(), detail: "永久刪除回收資料夾裡的所有項目，刪除後無法找回".into(), size: shared[11].size },
            Suggestion { key: "logs".into(), label: "刪除舊日誌與當機報告".into(), detail: format!("{} 個檔案：啟動器 7 天前、遊戲 14 天前的日誌與 30 天前的當機報告", logs.len()), size: log_size },
            Suggestion { key: "cache".into(), label: "清除快取".into(), detail: "natives 與縮圖快取，需要時會自動重建".into(), size: shared[6].size + shared[9].size },
            Suggestion { key: "versions".into(), label: "移除沒有實例使用的遊戲版本".into(), detail: if versions.is_empty() { "沒有".into() } else { versions.join("、") }, size: version_size },
            Suggestion { key: "snapshots".into(), label: "刪除所有模組快照".into(), detail: "移到回收資料夾".into(), size: snapshot_size },
        ];
        suggestions.retain(|s| s.size > 0);
        let total = shared.iter().map(|p| p.size).sum::<u64>() + list.iter().map(|i| i.total).sum::<u64>();
        Usage { total, shared, instances: list, suggestions }
    })
    .await
    .context("分析中斷")?;
    Ok(usage)
}

#[tauri::command]
pub async fn clean_disk(state: State<'_, AppState>, key: String) -> R<u64> {
    if !state.running_ids().await.is_empty() && key != "trash" {
        bail!("遊戲執行中無法清理，請先關閉遊戲");
    }
    let instances = crate::instance::list().await;
    let freed = match key.as_str() {
        "trash" => tokio::task::spawn_blocking(|| paths::purge_trash(0)).await.context("清理中斷")?,
        "logs" => {
            let files = old_logs(&instances).await;
            tokio::task::spawn_blocking(move || files.iter().map(|p| { let s = size(p); if std::fs::remove_file(p).is_ok() { s } else { 0 } }).sum()).await.context("清理中斷")?
        }
        "cache" => tokio::task::spawn_blocking(|| {
            let mut freed = 0;
            for dir in [paths::shared().join("natives"), crate::insight::thumbs_dir()] {
                freed += size(&dir);
                let _ = std::fs::remove_dir_all(&dir);
            }
            freed
        })
        .await
        .context("清理中斷")?,
        "versions" => {
            let names = stale_versions(&instances).await;
            tokio::task::spawn_blocking(move || -> anyhow::Result<u64> {
                let freed = names.iter().map(|v| size(&paths::versions().join(v))).sum();
                let items: Vec<(PathBuf, &str)> = names.iter().map(|v| (paths::versions().join(v), v.as_str())).collect();
                if !items.is_empty() {
                    paths::discard("沒有實例使用的遊戲版本", &items)?;
                }
                Ok(freed)
            })
            .await
            .context("清理中斷")??
        }
        "snapshots" => {
            let ids: Vec<String> = instances.iter().map(|i| i.id.clone()).collect();
            tokio::task::spawn_blocking(move || -> anyhow::Result<u64> {
                let mut freed = 0;
                for id in ids {
                    let dir = paths::instance_dir(&id).join("snapshots");
                    if dir.is_dir() {
                        freed += size(&dir);
                        paths::discard(&format!("模組快照 {id}"), &[(&dir, "snapshots")])?;
                    }
                }
                Ok(freed)
            })
            .await
            .context("清理中斷")??
        }
        _ => bail!("不支援的清理項目"),
    };
    Ok(freed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_versions_any_instance_uses() {
        let dirs = vec!["26.3".to_string(), "fabric-loader-0.19.5-26.3".to_string(), "1.21.11".to_string(), "fabric-loader-0.17.2-1.21.11".to_string(), "1.20.1".to_string()];
        let used = vec![("26.3".to_string(), "fabric-loader-0.19.5-26.3".to_string()), ("1.21.11".to_string(), String::new())];
        assert_eq!(unused_versions(&dirs, &used), vec!["1.20.1".to_string()]);
    }
}
