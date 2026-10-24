use crate::err::R;
use crate::state::AppState;
use crate::{bail, net, paths};
use anyhow::Context;
use serde::Serialize;
use std::io::Read;
use std::path::{Path, PathBuf};
use tauri::State;

const READ_LIMIT: u64 = 8 << 20;

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LogFile {
    pub path: String,
    pub size: u64,
    pub modified: i64,
    pub kind: String,
}

fn kind_of(rel: &str) -> Option<&'static str> {
    let lower = rel.to_ascii_lowercase();
    let name = lower.rsplit('/').next().unwrap_or(&lower);
    if lower.starts_with("crash-reports/") && name.ends_with(".txt") {
        Some("crash")
    } else if lower.starts_with("logs/") && (name.ends_with(".log") || name.ends_with(".log.gz") || name.ends_with(".txt")) {
        Some("log")
    } else if !lower.contains('/') && name.starts_with("hs_err_pid") && name.ends_with(".log") {
        Some("jvm")
    } else if !lower.contains('/') && name.ends_with(".log") {
        Some("other")
    } else {
        None
    }
}

fn scan(game_dir: &Path) -> Vec<LogFile> {
    let mut out = Vec::new();
    for (sub, prefix) in [(game_dir.join("crash-reports"), "crash-reports/"), (game_dir.join("logs"), "logs/"), (game_dir.to_path_buf(), "")] {
        for entry in std::fs::read_dir(&sub).map(|d| d.flatten().collect::<Vec<_>>()).unwrap_or_default() {
            let Ok(meta) = entry.metadata() else { continue };
            if !meta.is_file() {
                continue;
            }
            let rel = format!("{prefix}{}", entry.file_name().to_string_lossy());
            let Some(kind) = kind_of(&rel) else { continue };
            let modified = meta.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_secs() as i64).unwrap_or(0);
            out.push(LogFile { path: rel, size: meta.len(), modified, kind: kind.into() });
        }
    }
    out.sort_by(|a, b| b.modified.cmp(&a.modified).then(a.path.cmp(&b.path)));
    out
}

fn resolve(id: &str, rel: &str) -> R<PathBuf> {
    let game_dir = paths::game_dir(id);
    if !scan(&game_dir).iter().any(|f| f.path == rel) {
        bail!("找不到這個紀錄檔");
    }
    Ok(game_dir.join(rel))
}

#[tauri::command]
pub async fn log_files(id: String) -> R<Vec<LogFile>> {
    Ok(scan(&paths::game_dir(&id)))
}

#[tauri::command]
pub async fn read_log(id: String, path: String) -> R<String> {
    let file = resolve(&id, &path)?;
    tokio::task::spawn_blocking(move || -> R<String> {
        let mut bytes = Vec::new();
        let reader = std::fs::File::open(&file).context("無法開啟紀錄檔")?;
        if file.to_string_lossy().to_ascii_lowercase().ends_with(".gz") {
            flate2::read::GzDecoder::new(reader).take(READ_LIMIT * 4).read_to_end(&mut bytes).context("解壓縮紀錄檔失敗")?;
        } else {
            use std::io::{Seek, SeekFrom};
            let mut reader = reader;
            let len = reader.metadata().map(|m| m.len()).unwrap_or(0);
            if len > READ_LIMIT {
                reader.seek(SeekFrom::Start(len - READ_LIMIT))?;
            }
            reader.read_to_end(&mut bytes)?;
        }
        if bytes.len() as u64 > READ_LIMIT {
            bytes.drain(..bytes.len() - READ_LIMIT as usize);
        }
        Ok(String::from_utf8_lossy(&bytes).into_owned())
    })
    .await
    .context("讀取紀錄檔中斷")?
}

#[tauri::command]
pub async fn delete_logs(state: State<'_, AppState>, id: String, paths_to_delete: Vec<String>) -> R<usize> {
    let running = state.is_running(&id).await;
    let mut items = Vec::new();
    for rel in &paths_to_delete {
        if running && rel.eq_ignore_ascii_case("logs/latest.log") {
            continue;
        }
        items.push((resolve(&id, rel)?, rel.as_str()));
    }
    if !items.is_empty() {
        let name = crate::instance::get(&id).await.map(|i| i.name).unwrap_or_else(|_| id.clone());
        paths::discard(&format!("紀錄檔 {name}"), &items).context("刪除紀錄檔失敗")?;
    }
    Ok(items.len())
}

#[tauri::command]
pub async fn upload_log(content: String) -> R<String> {
    if content.trim().is_empty() {
        bail!("紀錄檔是空的");
    }
    let mut text = crate::insight::scrub_report(&content);
    if text.len() > 10 << 20 {
        let cut = text.len() - (10 << 20);
        let cut = (cut..text.len()).find(|&i| text.is_char_boundary(i)).unwrap_or(text.len());
        text = text[cut..].to_string();
    }
    let response = net::client()
        .post("https://api.mclo.gs/1/log")
        .form(&[("content", text)])
        .send()
        .await
        .context("上傳到 mclo.gs 失敗")?;
    let body: serde_json::Value = response.json().await.context("mclo.gs 回應格式不正確")?;
    if body.get("success").and_then(|v| v.as_bool()) != Some(true) {
        bail!("mclo.gs 拒絕了這份紀錄：{}", body.get("error").and_then(|v| v.as_str()).unwrap_or("未知原因"));
    }
    Ok(body.get("url").and_then(|v| v.as_str()).unwrap_or_default().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_only_log_like_files() {
        let dir = std::env::temp_dir().join(format!("logfiles-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        for f in ["logs/latest.log", "logs/2026-10-18-1.log.gz", "crash-reports/crash-a.txt", "hs_err_pid1.log", "replay_pid1.log", "options.txt", "mods/x.log", "logs/sub/deep.log"] {
            let p = dir.join(f);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, "x").unwrap();
        }
        let mut found: Vec<_> = scan(&dir).into_iter().map(|f| (f.path, f.kind)).collect();
        found.sort();
        assert_eq!(found, vec![
            ("crash-reports/crash-a.txt".into(), "crash".into()),
            ("hs_err_pid1.log".into(), "jvm".into()),
            ("logs/2026-10-18-1.log.gz".into(), "log".into()),
            ("logs/latest.log".into(), "log".into()),
            ("replay_pid1.log".into(), "other".into()),
        ]);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
