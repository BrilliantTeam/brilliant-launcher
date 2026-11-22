use crate::bail;
use crate::config::Account;
use crate::err::R;
use crate::instance::Instance;
use anyhow::Context;
use serde_json::{json, Value};
use tauri::plugin::{Builder, PluginHandle, TauriPlugin};
use tauri::{AppHandle, Manager, Wry};

struct Game(PluginHandle<Wry>);

pub fn init() -> TauriPlugin<Wry> {
    Builder::new("brilliant-game")
        .setup(|app, api| {
            app.manage(Game(api.register_android_plugin("net.brilliantw.launcher", "GamePlugin")?));
            Ok(())
        })
        .build()
}

pub async fn export_game_settings(app: &AppHandle) -> R<String> {
    let value = call(app, "exportGameSettings", json!({})).await?;
    Ok(value["json"].as_str().unwrap_or_default().to_string())
}

pub async fn import_game_settings(app: &AppHandle, settings: &str) -> R<u64> {
    let value = call(app, "importGameSettings", json!({ "json": settings })).await?;
    Ok(value["applied"].as_u64().unwrap_or(0))
}

pub async fn play_update(app: &AppHandle, command: &'static str) -> R<Value> {
    Ok(call(app, command, json!({})).await?)
}

async fn call(app: &AppHandle, command: &'static str, payload: Value) -> anyhow::Result<Value> {
    let handle = app.state::<Game>().0.clone();
    for attempt in 0.. {
        let (handle, payload) = (handle.clone(), payload.clone());
        let result = tokio::task::spawn_blocking(move || handle.run_mobile_plugin::<Value>(command, payload)).await.context("手機端工作中斷")?;
        match result {
            Err(e) if attempt < 20 && e.to_string().contains("not initialized") => tokio::time::sleep(std::time::Duration::from_millis(150)).await,
            other => return other.map_err(|e| anyhow::anyhow!("{command}：{e}")),
        }
    }
    unreachable!()
}

pub async fn launch(app: &AppHandle, instance: &Instance, version_id: &str, account: &Account, language: &str, join: &str, proxy: &[String]) -> R<()> {
    let msa = account.kind == "msa";
    let jvm_args = std::iter::once(format!("-Dbrilliant.launcher={} -Dbrilliant.android=true", env!("CARGO_PKG_VERSION"))).chain(proxy.iter().cloned()).collect::<Vec<_>>().join(" ");
    call(app, "launch", json!({
        "versionId": version_id,
        "gameDir": crate::paths::game_dir(&instance.id).strip_prefix(crate::paths::root()).context("遊戲資料夾不在啟動器資料夾內")?.to_string_lossy().replace('\\', "/"),
        "username": account.name,
        "uuid": account.uuid,
        "accessToken": if msa { account.access_token.as_str() } else { "0" },
        "xuid": account.xuid,
        "language": language,
        "join": join,
        "jvmArgs": jvm_args,
        "controls": crate::hotfix::data_text("controls.json"),
        "mobile": crate::hotfix::data_text("mobile.json"),
    }))
    .await
    .context("遊戲執行環境啟動失敗")?;
    Ok(())
}

fn pick<'a>(language: &str, zh_tw: &'a str, zh_cn: &'a str, en: &'a str) -> &'a str {
    match language {
        "zh-CN" => zh_cn,
        "en" => en,
        _ => zh_tw,
    }
}

static ACTIVE_DOWNLOADS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

pub struct KeepAlive(AppHandle);

impl KeepAlive {
    pub fn new(app: &AppHandle) -> Self {
        if ACTIVE_DOWNLOADS.fetch_add(1, std::sync::atomic::Ordering::SeqCst) == 0 {
            Self::sync(app.clone());
        }
        Self(app.clone())
    }

    fn sync(app: AppHandle) {
        tauri::async_runtime::spawn(async move {
            if ACTIVE_DOWNLOADS.load(std::sync::atomic::Ordering::SeqCst) == 0 {
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                if ACTIVE_DOWNLOADS.load(std::sync::atomic::Ordering::SeqCst) > 0 {
                    return;
                }
            }
            let language = crate::config::load_settings().await.language;
            let payload = json!({
                "on": ACTIVE_DOWNLOADS.load(std::sync::atomic::Ordering::SeqCst) > 0,
                "title": pick(&language, "輝煌啟動器正在下載遊戲檔案", "辉煌启动器正在下载游戏文件", "Brilliant Launcher is downloading game files"),
                "channel": pick(&language, "下載", "下载", "Downloads"),
            });
            if let Err(e) = call(&app, "keepAlive", payload).await {
                eprintln!("[mobile] 背景下載服務切換失敗：{e:#}");
            }
        });
    }
}

impl Drop for KeepAlive {
    fn drop(&mut self) {
        if ACTIVE_DOWNLOADS.fetch_sub(1, std::sync::atomic::Ordering::SeqCst) == 1 {
            Self::sync(self.0.clone());
        }
    }
}

pub async fn web_login(app: &AppHandle, url: &str, redirect: &str) -> R<Option<String>> {
    let language = crate::config::load_settings().await.language;
    let value = call(app, "webLogin", json!({
        "url": url,
        "redirect": redirect,
        "title": pick(&language, "微軟登入", "微软登录", "Microsoft sign-in"),
        "close": pick(&language, "關閉", "关闭", "Close"),
    }))
    .await
    .context("無法開啟登入畫面")?;
    Ok(value["url"].as_str().map(str::to_string))
}

pub async fn resume_game(app: &AppHandle) -> R<()> {
    let value = call(app, "resumeGame", json!({})).await.context("無法切回遊戲")?;
    if value["found"].as_bool() != Some(true) {
        bail!("找不到執行中的遊戲");
    }
    Ok(())
}

pub async fn stop_game(app: &AppHandle) {
    let _ = call(app, "stopGame", json!({})).await;
}

pub async fn run_installer(app: &AppHandle, mc: &str, jar: &std::path::Path, target: &std::path::Path, log: &std::path::Path) -> R<bool> {
    let _ = std::fs::remove_file(log);
    let java_args = format!("-Djava.awt.headless=true -jar {} --installClient {}", jar.display(), target.display());
    call(app, "runInstaller", json!({ "versionId": mc, "javaArgs": java_args })).await.context("無法啟動載入器安裝程式")?;
    let outcome = || {
        let text = std::fs::read_to_string(log).unwrap_or_default();
        if text.contains("Successfully installed client") {
            Some(true)
        } else if text.contains("There was an error during installation") {
            Some(false)
        } else {
            None
        }
    };
    let done = || outcome() == Some(true);
    let mut started = false;
    for second in 0..1800 {
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        if let Some(ok) = outcome() {
            for _ in 0..20 {
                if !call(app, "installerRunning", json!({})).await.map(|v| v["running"].as_bool().unwrap_or(false)).unwrap_or(false) {
                    return Ok(ok);
                }
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            }
            let _ = call(app, "stopInstaller", json!({})).await;
            return Ok(ok);
        }
        let running = call(app, "installerRunning", json!({})).await.map(|v| v["running"].as_bool().unwrap_or(false)).unwrap_or(false);
        if running {
            started = true;
        } else if started {
            return Ok(done());
        } else if second >= 60 {
            bail!("載入器安裝程式沒有啟動");
        }
    }
    let _ = call(app, "stopInstaller", json!({})).await;
    bail!("載入器安裝程式執行太久，已停止等待")
}

pub async fn game_running(app: &AppHandle) -> bool {
    call(app, "gameRunning", json!({})).await.map(|v| v["running"].as_bool().unwrap_or(false)).unwrap_or(false)
}

pub async fn open_file(app: &AppHandle, path: &std::path::Path) -> R<()> {
    call(app, "openFile", json!({ "path": path.to_string_lossy() })).await.context("無法開啟這個檔案")?;
    Ok(())
}

pub async fn stage(app: &AppHandle, uri: String) -> R<String> {
    let value = call(app, "stage", json!({ "uri": uri })).await.context("讀取選取的檔案失敗")?;
    Ok(value["path"].as_str().context("讀取選取的檔案失敗")?.to_string())
}

pub async fn data_location(app: &AppHandle) -> R<Value> {
    let mut info = call(app, "storageInfo", json!({})).await.context("讀取儲存空間失敗")?;
    let chosen = crate::paths::chosen_location();
    let root = crate::paths::root();
    info["root"] = json!(root.to_string_lossy());
    info["internal"] = json!(crate::paths::default_root().to_string_lossy());
    info["missing"] = json!(chosen.as_ref().is_some_and(|c| *c != root));
    info["chosen"] = json!(chosen.map(|c| c.to_string_lossy().to_string()));
    info["play"] = json!(crate::commands::PLAY);
    Ok(info)
}

pub async fn request_storage_access(app: &AppHandle) -> R<()> {
    call(app, "requestStorageAccess", json!({})).await.context("無法開啟權限設定")?;
    Ok(())
}

pub async fn move_data(app: &AppHandle, target: String) -> R<String> {
    use crate::paths;
    use std::path::{Component, PathBuf};
    if !app.state::<crate::state::AppState>().running.lock().await.is_empty() || game_running(app).await {
        bail!("請先結束遊戲再搬移資料");
    }
    let from = paths::root();
    let internal = paths::default_root();
    if !internal.is_absolute() {
        bail!("找不到啟動器的內部資料夾，無法搬移");
    }
    let to = match target.trim() {
        "" => internal.clone(),
        text => {
            let base = PathBuf::from(text.trim_end_matches('/'));
            if !base.is_absolute() || base.components().any(|c| matches!(c, Component::ParentDir)) {
                bail!("請輸入完整的資料夾路徑，例如 /storage/emulated/0");
            }
            if base.file_name().is_some_and(|n| n == "BrilliantLauncher") { base } else { base.join("BrilliantLauncher") }
        }
    };
    if to == from {
        bail!("資料已經在這個位置了");
    }
    if to.starts_with(&from) || from.starts_with(&to) {
        bail!("新位置不能在目前的資料夾裡面，也不能包含目前的資料夾");
    }
    let moving = to.join(".brilliant-moving");
    if moving.exists() || (to == internal && !to.join("instances").is_dir()) {
        for entry in std::fs::read_dir(&to).map(|d| d.flatten().collect::<Vec<_>>()).unwrap_or_default() {
            if paths::PRIVATE_FILES.iter().any(|p| entry.file_name() == *p) {
                continue;
            }
            let path = entry.path();
            let _ = if path.is_dir() { std::fs::remove_dir_all(&path) } else { std::fs::remove_file(&path) };
        }
    }
    let adopt = to.join("instances").is_dir();
    let occupied = std::fs::read_dir(&to).map(|d| d.flatten().any(|e| !paths::PRIVATE_FILES.iter().any(|p| e.file_name() == *p))).unwrap_or(false);
    if occupied && !adopt {
        bail!("「{}」裡已經有其他檔案，請換一個空的資料夾", to.display());
    }
    let probe = to.join(".brilliant-write-test");
    std::fs::create_dir_all(&to).and_then(|_| std::fs::write(&probe, b"ok")).and_then(|_| std::fs::remove_file(&probe)).with_context(|| format!("沒有權限寫入「{}」，請確認已開啟「所有檔案存取權」，或換一個位置", to.display()))?;
    let mut names = Vec::new();
    if !adopt {
        let need = paths::size_of(&from);
        let free = call(app, "freeSpace", json!({ "path": to.to_string_lossy() })).await.ok().and_then(|v| v["free"].as_u64()).unwrap_or(u64::MAX);
        if free < need + 256 * 1024 * 1024 {
            bail!("新位置的空間不足：需要約 {} MB，只剩 {} MB", need / 1048576 + 256, free / 1048576);
        }
        std::fs::write(&moving, b"").context("無法在新位置建立檔案")?;
        let handle = app.clone();
        let (src, dest) = (from.clone(), to.clone());
        let mut step = 0;
        names = tokio::task::spawn_blocking(move || {
            paths::relocate(&src, &dest, |done, total| {
                let now = done * 100 / total;
                if now != step {
                    step = now;
                    let _ = tauri::Emitter::emit(&handle, "launcher://relocate", json!({ "done": done, "total": total }));
                }
            })
        })
        .await
        .context("搬移工作中斷")?
        .inspect_err(|_| {
            let _ = std::fs::remove_file(&moving);
        })
        .context("搬移資料失敗，已收回複製到一半的檔案，原本的資料沒有變動")?;
        paths::rewrite_file(&paths::config_file(), &from, &to);
    }
    let saved = if to == internal {
        std::fs::remove_file(paths::location_file()).or_else(|e| if e.kind() == std::io::ErrorKind::NotFound { Ok(()) } else { Err(e) })
    } else {
        paths::write_atomic(&paths::location_file(), to.to_string_lossy().as_bytes())
    };
    if let Err(error) = saved {
        let _ = std::fs::remove_file(&moving);
        paths::remove_moved(&to, &names);
        return Err(anyhow::Error::from(error).context("儲存新位置失敗，已收回複製過去的檔案，原本的資料沒有變動").into());
    }
    let _ = std::fs::remove_file(&moving);
    *paths::location().lock().unwrap_or_else(|e| e.into_inner()) = (to != internal).then(|| to.clone());
    let _ = app.asset_protocol_scope().allow_directory(&to, true);
    paths::remove_moved(&from, &names);
    Ok(to.to_string_lossy().to_string())
}

fn offered_mods(marker: &str) -> Vec<String> {
    let listed: Vec<String> = marker.lines().map(str::trim).filter(|l| !l.is_empty()).map(str::to_string).collect();
    if listed.is_empty() {
        ["ferrite-core", "lithium", "entityculling"].map(String::from).to_vec()
    } else {
        listed
    }
}

pub async fn performance_mods(app: &AppHandle, instance: &Instance, settings: &crate::config::Settings) -> Option<String> {
    let marker = crate::paths::instance_dir(&instance.id).join("mobile-performance-mods");
    let mut offered = match std::fs::read_to_string(&marker) {
        Ok(text) => offered_mods(&text),
        Err(_) => Vec::new(),
    };
    let pending: Vec<(String, String)> = crate::hotfix::data::<Vec<(String, String)>>("mobile-mods.json").into_iter().filter(|(p, _)| !offered.iter().any(|o| o == p)).collect();
    if pending.is_empty() {
        return None;
    }
    crate::net::emit_stage(app, "mods", "安裝手機版效能模組");
    let mut added = Vec::new();
    for (project, name) in pending {
        match crate::modrinth::install(app, &instance.id, crate::modrinth::Kind::Mod, &instance.loader, &project, &instance.mc_version, instance.wants_rich_presence(settings), instance.channel(settings)).await {
            Ok(files) => {
                if !files.is_empty() {
                    added.push(name);
                }
                offered.push(project.to_string());
            }
            Err(e) if format!("{:#}", e.0).contains("沒有支援 Minecraft") => offered.push(project.to_string()),
            Err(_) => {}
        }
    }
    let _ = std::fs::write(&marker, offered.join("\n"));
    (!added.is_empty()).then(|| format!("已為手機版安裝效能模組：{}，不需要可以到「模組」頁移除", added.join("、")))
}

#[cfg(test)]
mod performance_tests {
    use super::*;

    #[test]
    fn old_empty_marker_means_first_three_offered() {
        assert_eq!(offered_mods(""), vec!["ferrite-core", "lithium", "entityculling"]);
        assert_eq!(offered_mods("sodium\nlithium\n"), vec!["sodium", "lithium"]);
    }
}

pub async fn install_apk(app: &AppHandle, path: &std::path::Path) -> R<String> {
    let value = call(app, "installApk", json!({ "path": path.to_string_lossy() })).await.context("無法開啟安裝畫面")?;
    Ok(value["status"].as_str().unwrap_or("started").to_string())
}

pub async fn take_shortcut(app: &AppHandle) -> String {
    call(app, "takeShortcut", json!({})).await.ok().and_then(|v| v["action"].as_str().map(str::to_string)).unwrap_or_default()
}
