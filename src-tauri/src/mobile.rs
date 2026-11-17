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

async fn call(app: &AppHandle, command: &'static str, payload: Value) -> anyhow::Result<Value> {
    let handle = app.state::<Game>().0.clone();
    tokio::task::spawn_blocking(move || handle.run_mobile_plugin::<Value>(command, payload))
        .await
        .context("手機端工作中斷")?
        .map_err(|e| anyhow::anyhow!("{e}"))
}

pub async fn launch(app: &AppHandle, instance: &Instance, account: &Account, language: &str) -> R<()> {
    if crate::forge::is_forge_like(&instance.loader) {
        bail!("手機版目前只支援原版與 Fabric 實例");
    }
    let msa = account.kind == "msa";
    call(app, "launch", json!({
        "versionId": instance.version_id,
        "gameDir": crate::paths::game_dir(&instance.id).strip_prefix(crate::paths::root()).context("遊戲資料夾不在啟動器資料夾內")?.to_string_lossy().replace('\\', "/"),
        "username": account.name,
        "uuid": account.uuid,
        "accessToken": if msa { account.access_token.as_str() } else { "0" },
        "xuid": account.xuid,
        "language": language,
        "jvmArgs": format!("-Dbrilliant.launcher={} -Dbrilliant.android=true", env!("CARGO_PKG_VERSION")),
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
