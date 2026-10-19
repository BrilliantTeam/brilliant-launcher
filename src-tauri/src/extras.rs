use crate::config::now_secs;
use crate::err::R;
use crate::state::AppState;
use crate::{bail, paths};
use anyhow::Context;
use serde::Serialize;
use std::io::Write;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager, State};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Backup {
    pub file: String,
    pub kind: String,
    pub time: i64,
    pub size: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Screenshot {
    pub path: String,
    pub name: String,
    pub time: i64,
}

fn backups_dir(id: &str) -> PathBuf {
    paths::instance_dir(id).join("backups")
}

fn mtime(path: &Path) -> i64 {
    std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn zip_dir(src: &Path, dest: &Path) -> R<()> {
    zip_folder(src, dest, None)
}

pub fn zip_folder(src: &Path, dest: &Path, prefix: Option<&str>) -> R<()> {
    let tmp = dest.with_extension("part");
    let mut zip = zip::ZipWriter::new(std::fs::File::create(&tmp)?);
    let options = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    let mut stack = vec![src.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir)?.flatten() {
            let path = entry.path();
            let rel = path.strip_prefix(src).unwrap_or(&path).to_string_lossy().replace('\\', "/");
            let rel = match prefix {
                Some(p) => format!("{p}/{rel}"),
                None => rel,
            };
            if path.is_dir() {
                zip.add_directory(format!("{rel}/"), options)?;
                stack.push(path);
            } else if entry.file_name() != "session.lock" {
                zip.start_file(rel, options)?;
                zip.write_all(&std::fs::read(&path).with_context(|| format!("讀取 {} 失敗", path.display()))?)?;
            }
        }
    }
    zip.finish()?;
    std::fs::rename(&tmp, dest)?;
    Ok(())
}

pub fn backup_saves(id: &str, kind: &str, keep: u32) -> R<Option<String>> {
    let saves = paths::game_dir(id).join("saves");
    if !saves.is_dir() || std::fs::read_dir(&saves).map(|mut d| d.next().is_none()).unwrap_or(true) {
        return Ok(None);
    }
    let dir = backups_dir(id);
    std::fs::create_dir_all(&dir)?;
    let name = format!("{kind}-{}.zip", now_secs());
    zip_dir(&saves, &dir.join(&name)).map_err(|e| anyhow::anyhow!("備份存檔失敗：{:#}", e.0))?;
    if keep > 0 {
        let mut old: Vec<_> = list(id).into_iter().filter(|b| b.kind == kind).collect();
        old.sort_by_key(|b| std::cmp::Reverse(b.time));
        for b in old.into_iter().skip(keep as usize) {
            let _ = std::fs::remove_file(dir.join(b.file));
        }
    }
    Ok(Some(name))
}

fn list(id: &str) -> Vec<Backup> {
    let mut out: Vec<Backup> = std::fs::read_dir(backups_dir(id))
        .map(|d| d.flatten().collect::<Vec<_>>())
        .unwrap_or_default()
        .into_iter()
        .filter_map(|e| {
            let file = e.file_name().to_string_lossy().to_string();
            let stem = file.strip_suffix(".zip")?;
            let (kind, time) = stem.rsplit_once('-')?;
            Some(Backup {
                kind: kind.to_string(),
                time: time.parse().ok()?,
                size: e.metadata().map(|m| m.len()).unwrap_or(0),
                file,
            })
        })
        .collect();
    out.sort_by_key(|b| std::cmp::Reverse(b.time));
    out
}

fn safe_backup_name(file: &str) -> bool {
    file.ends_with(".zip") && !file.contains(['/', '\\']) && !file.contains("..")
}

#[tauri::command]
pub async fn list_backups(id: String) -> Vec<Backup> {
    list(&id)
}

#[tauri::command]
pub async fn backup_now(id: String) -> R<String> {
    tokio::task::spawn_blocking(move || backup_saves(&id, "manual", 0))
        .await
        .context("備份工作中斷")??
        .context("這個實例還沒有任何存檔")
        .map_err(Into::into)
}

#[tauri::command]
pub async fn restore_backup(state: State<'_, AppState>, id: String, file: String) -> R<()> {
    if !safe_backup_name(&file) {
        bail!("備份檔名不正確");
    }
    if state.is_running(&id).await {
        bail!("請先關閉遊戲再還原存檔");
    }
    tokio::task::spawn_blocking(move || -> R<()> {
        let archive = backups_dir(&id).join(&file);
        let mut zip = zip::ZipArchive::new(std::fs::File::open(&archive).context("找不到備份檔")?)?;
        backup_saves(&id, "before-restore", 3)?;
        let saves = paths::game_dir(&id).join("saves");
        if saves.exists() {
            std::fs::remove_dir_all(&saves).context("清除目前存檔失敗")?;
        }
        zip.extract(&saves).context("解壓縮備份失敗")?;
        Ok(())
    })
    .await
    .context("還原工作中斷")?
}

#[tauri::command]
pub async fn delete_backup(id: String, file: String) -> R<()> {
    if !safe_backup_name(&file) {
        bail!("備份檔名不正確");
    }
    tokio::fs::remove_file(backups_dir(&id).join(file)).await.context("刪除備份失敗")?;
    Ok(())
}

#[tauri::command]
pub async fn list_screenshots(id: String) -> Vec<Screenshot> {
    let mut out: Vec<Screenshot> = std::fs::read_dir(paths::game_dir(&id).join("screenshots"))
        .map(|d| d.flatten().collect::<Vec<_>>())
        .unwrap_or_default()
        .into_iter()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("png")))
        .map(|p| Screenshot {
            time: mtime(&p),
            name: p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(),
            path: p.to_string_lossy().to_string(),
        })
        .collect();
    out.sort_by_key(|s| std::cmp::Reverse(s.time));
    out
}

fn dir_size(path: &Path) -> u64 {
    std::fs::read_dir(path)
        .map(|d| d.flatten().map(|e| {
            let p = e.path();
            if p.is_dir() { dir_size(&p) } else { e.metadata().map(|m| m.len()).unwrap_or(0) }
        }).sum())
        .unwrap_or(0)
}

#[tauri::command]
pub async fn clean_cache(state: State<'_, AppState>) -> R<u64> {
    if !state.running_ids().await.is_empty() {
        bail!("遊戲執行中無法清理，請先關閉遊戲");
    }
    tokio::task::spawn_blocking(|| {
        let mut freed = 0;
        let natives = paths::shared().join("natives");
        freed += dir_size(&natives);
        let _ = std::fs::remove_dir_all(&natives);
        let week_ago = now_secs() - 7 * 86_400;
        for entry in std::fs::read_dir(paths::logs()).map(|d| d.flatten().collect::<Vec<_>>()).unwrap_or_default() {
            if mtime(&entry.path()) < week_ago {
                freed += entry.metadata().map(|m| m.len()).unwrap_or(0);
                let _ = std::fs::remove_file(entry.path());
            }
        }
        freed
    })
    .await
    .context("清理工作中斷")
    .map_err(Into::into)
}

pub const IMAGE_EXTS: [&str; 5] = ["png", "jpg", "jpeg", "webp", "gif"];
const VIDEO_EXTS: [&str; 2] = ["mp4", "webm"];
const AUDIO_EXTS: [&str; 6] = ["mp3", "ogg", "wav", "m4a", "flac", "opus"];

pub async fn copy_media(src: &Path, exts: &[&str], max_mb: u64, dir: &Path, prefix: &str) -> R<String> {
    let ext = src
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .filter(|e| exts.contains(&e.as_str()))
        .with_context(|| format!("只支援 {} 格式", exts.join("、").to_uppercase()))?;
    let meta = tokio::fs::metadata(src).await.context("找不到檔案")?;
    if meta.len() > max_mb * 1024 * 1024 {
        bail!("檔案太大了（上限 {max_mb} MB）");
    }
    tokio::fs::create_dir_all(dir).await?;
    for entry in std::fs::read_dir(dir).map(|d| d.flatten().collect::<Vec<_>>()).unwrap_or_default() {
        if entry.file_name().to_string_lossy().starts_with(prefix) {
            let _ = std::fs::remove_file(entry.path());
        }
    }
    let dest = dir.join(format!("{prefix}{}.{ext}", now_secs()));
    tokio::fs::copy(src, &dest).await.context("複製檔案失敗")?;
    Ok(dest.to_string_lossy().to_string())
}

#[tauri::command]
pub async fn set_media(kind: String, path: String) -> R<String> {
    let src = PathBuf::from(&path);
    let media = paths::root().join("media");
    match kind.as_str() {
        "background" => copy_media(&src, &[&IMAGE_EXTS[..], &VIDEO_EXTS[..]].concat(), 500, &media, "background-").await,
        "music" => copy_media(&src, &AUDIO_EXTS, 200, &media, "music-").await,
        _ => bail!("不支援的媒體類型"),
    }
}

pub fn set_autostart(on: bool) -> R<()> {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";
        let exe = std::env::current_exe()?;
        let mut cmd = std::process::Command::new("reg");
        if on {
            cmd.args(["add", KEY, "/v", "BrilliantLauncher", "/t", "REG_SZ", "/f", "/d"])
                .arg(format!("\"{}\"", exe.display()));
        } else {
            cmd.args(["delete", KEY, "/v", "BrilliantLauncher", "/f"]);
        }
        let status = cmd.creation_flags(0x0800_0000).output()?.status;
        if on && !status.success() {
            bail!("無法設定開機自動啟動");
        }
    }
    let _ = on;
    Ok(())
}

pub fn hook_command(script: &str, instance: &crate::instance::Instance) -> tokio::process::Command {
    let mut cmd = if cfg!(target_os = "windows") {
        let mut c = tokio::process::Command::new("cmd");
        c.arg("/C").arg(script);
        c
    } else {
        let mut c = tokio::process::Command::new("sh");
        c.arg("-c").arg(script);
        c
    };
    cmd.current_dir(paths::game_dir(&instance.id))
        .env("INST_ID", &instance.id)
        .env("INST_NAME", &instance.name)
        .env("INST_MC_VER", &instance.mc_version)
        .env("INST_DIR", paths::game_dir(&instance.id))
        .stdin(std::process::Stdio::null());
    crate::mc::java::hide_console(&mut cmd);
    cmd
}

pub fn setup_tray(app: &tauri::App) -> tauri::Result<()> {
    use tauri::menu::{Menu, MenuItem};
    use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
    use tauri::Emitter;

    let show = MenuItem::with_id(app, "show", "顯示啟動器", true, None::<&str>)?;
    let play = MenuItem::with_id(app, "play", "開始遊戲", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "結束輝煌啟動器", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &play, &quit])?;
    let reveal = |app: &AppHandle| {
        if let Some(window) = app.get_webview_window("main") {
            let _ = window.show();
            let _ = window.unminimize();
            let _ = window.set_focus();
        }
    };
    let mut tray = TrayIconBuilder::with_id("main")
        .tooltip("輝煌啟動器")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(move |app, event| match event.id().as_ref() {
            "show" => reveal(app),
            "play" => {
                reveal(app);
                let _ = app.emit("launcher://tray-play", ());
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(move |tray, event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                reveal(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }
    tray.build(app)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zip_round_trip_skips_session_lock() {
        let base = std::env::temp_dir().join(format!("bl-zip-{}", now_secs()));
        let src = base.join("saves/World/region");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::write(src.join("r.0.0.mca"), b"chunk").unwrap();
        std::fs::write(base.join("saves/World/session.lock"), b"lock").unwrap();
        let archive = base.join("out.zip");
        zip_dir(&base.join("saves"), &archive).unwrap();
        let out = base.join("restored");
        zip::ZipArchive::new(std::fs::File::open(&archive).unwrap()).unwrap().extract(&out).unwrap();
        assert_eq!(std::fs::read(out.join("World/region/r.0.0.mca")).unwrap(), b"chunk");
        assert!(!out.join("World/session.lock").exists());
        assert!(safe_backup_name("auto-1.zip") && !safe_backup_name("../x.zip"));
        let _ = std::fs::remove_dir_all(base);
    }
}
