use crate::config::{self, Account, Settings};
use crate::err::R;
use crate::instance::Instance;
use crate::mc::{java::JavaInfo, manifest};
use crate::state::AppState;
use crate::{auth, fabric, instance, launch, modinject, modrinth, paths, ping};
use crate::bail;
use anyhow::Context;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

const SITE: &str = "https://www.brilliantw.net";

#[tauri::command]
pub async fn get_settings() -> Settings {
    config::load_settings().await
}

#[tauri::command]
pub async fn save_settings(settings: Settings) -> R<()> {
    let before = config::load_settings().await;
    if before.autostart != settings.autostart {
        crate::extras::set_autostart(settings.autostart)?;
    }
    crate::net::set_concurrency(settings.download_threads);
    if before.proxy.trim() != settings.proxy.trim() {
        crate::net::set_proxy(&settings.proxy)?;
    }
    config::save_settings(&settings).await
}

#[tauri::command]
pub fn launcher_paths() -> Vec<(String, String)> {
    vec![
        ("資料夾".into(), paths::root().to_string_lossy().to_string()),
        ("共用快取".into(), paths::shared().to_string_lossy().to_string()),
        ("實例".into(), paths::instances().to_string_lossy().to_string()),
        ("日誌".into(), paths::logs().to_string_lossy().to_string()),
    ]
}

#[tauri::command]
pub async fn list_accounts() -> Vec<Account> {
    config::load_accounts()
        .await
        .into_iter()
        .map(|mut a| {
            a.access_token.clear();
            a.refresh_token.clear();
            a
        })
        .collect()
}

async fn upsert_account(state: &State<'_, AppState>, account: Account) -> R<()> {
    let _guard = state.accounts_lock.lock().await;
    let mut accounts = config::load_accounts().await;
    match accounts.iter_mut().find(|a| a.id == account.id) {
        Some(existing) => *existing = account.clone(),
        None => accounts.push(account.clone()),
    }
    config::save_accounts(&accounts).await?;

    let mut settings = config::load_settings().await;
    if settings.selected_account.is_empty() {
        settings.selected_account = account.id;
        config::save_settings(&settings).await?;
    }
    Ok(())
}

#[tauri::command]
pub async fn login_microsoft(app: AppHandle, state: State<'_, AppState>) -> R<Account> {
    const LABEL: &str = "msa-login";
    if let Some(window) = app.get_webview_window(LABEL) {
        let _ = window.set_focus();
        bail!("登入視窗已經開著了");
    }
    let (url, pkce) = auth::msa::authorize_url(config::MSA_CLIENT_ID);
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<Result<String, String>>();
    let nav_tx = tx.clone();
    let want_state = pkce.state.clone();
    let window = tauri::WebviewWindowBuilder::new(&app, LABEL, tauri::WebviewUrl::External(url.parse().context("登入網址不正確")?))
        .title("微軟登入")
        .inner_size(500.0, 640.0)
        .resizable(false)
        .on_navigation(move |url| match auth::msa::read_redirect(url, &want_state) {
            auth::msa::Redirect::Ignore => true,
            auth::msa::Redirect::Code(code) => { let _ = nav_tx.send(Ok(code)); false }
            auth::msa::Redirect::Denied(why) => { let _ = nav_tx.send(Err(why)); false }
        })
        .build()
        .context("無法開啟登入視窗")?;
    window.on_window_event(move |event| {
        if let tauri::WindowEvent::Destroyed = event {
            let _ = tx.send(Err("已取消登入".into()));
        }
    });
    let result = rx.recv().await.unwrap_or_else(|| Err("已取消登入".into()));
    let _ = window.destroy();
    let code = result.map_err(|why| anyhow::anyhow!(why))?;
    let account = auth::msa::exchange(config::MSA_CLIENT_ID, &code, &pkce).await?;
    upsert_account(&state, account.clone()).await?;
    Ok(Account { access_token: String::new(), refresh_token: String::new(), ..account })
}

#[tauri::command]
pub async fn add_offline_account(state: State<'_, AppState>, name: String) -> R<Account> {
    let account = auth::make_offline(&name)?;
    upsert_account(&state, account.clone()).await?;
    Ok(account)
}

#[tauri::command]
pub async fn remove_account(state: State<'_, AppState>, id: String) -> R<()> {
    let _guard = state.accounts_lock.lock().await;
    let mut accounts = config::load_accounts().await;
    accounts.retain(|a| a.id != id);
    config::save_accounts(&accounts).await?;

    let mut settings = config::load_settings().await;
    if settings.selected_account == id {
        settings.selected_account = accounts.first().map(|a| a.id.clone()).unwrap_or_default();
        config::save_settings(&settings).await?;
    }
    Ok(())
}

#[tauri::command]
pub async fn select_account(id: String) -> R<()> {
    let mut settings = config::load_settings().await;
    settings.selected_account = id;
    config::save_settings(&settings).await
}

pub async fn active_account(settings: &Settings) -> R<Account> {
    let accounts = config::load_accounts().await;
    if accounts.is_empty() {
        bail!("還沒有任何帳號，請先在「帳號」頁登入");
    }
    let account = accounts
        .iter()
        .find(|a| a.id == settings.selected_account)
        .or_else(|| accounts.first())
        .cloned()
        .expect("accounts 非空");

    crate::skins::fresh(account, 60).await
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionList {
    pub latest_release: String,
    pub latest_snapshot: String,
    pub versions: Vec<manifest::VersionEntry>,
    pub mod_versions: Vec<String>,
    pub recommended: String,
}

#[tauri::command]
pub async fn list_versions() -> R<VersionList> {
    let settings = config::load_settings().await;
    let (m, info) = tokio::join!(manifest::fetch(settings.use_mirror), modinject::info());
    let m = m?;
    let info = info.unwrap_or_default();
    let versions = m
        .versions
        .into_iter()
        .filter(|v| match v.kind.as_str() {
            "release" => true,
            "snapshot" => settings.show_snapshots,
            _ => settings.show_old,
        })
        .collect();
    Ok(VersionList {
        latest_release: m.latest.release,
        latest_snapshot: m.latest.snapshot,
        versions,
        mod_versions: info.versions,
        recommended: info.recommended,
    })
}

#[tauri::command]
pub async fn list_fabric_loaders(game_version: String) -> R<Vec<fabric::LoaderInfo>> {
    let settings = config::load_settings().await;
    fabric::loaders(&game_version, settings.use_mirror).await
}

#[tauri::command]
pub async fn list_java() -> Vec<JavaInfo> {
    crate::mc::java::scan().await
}

#[tauri::command]
pub async fn list_instances(state: State<'_, AppState>) -> R<(Vec<Instance>, Vec<String>)> {
    Ok((instance::list().await, state.running_ids().await))
}

async fn resolve_version(draft: &mut Instance, settings: &Settings) -> R<()> {
    if draft.mc_version.trim().is_empty() {
        bail!("請先選擇 Minecraft 版本");
    }
    draft.version_id = if draft.loader == "fabric" {
        if draft.loader_version.trim().is_empty() {
            let latest = fabric::loaders(&draft.mc_version, settings.use_mirror)
                .await?
                .into_iter()
                .find(|l| l.stable)
                .context("這個版本還沒有穩定版的 Fabric Loader")?;
            draft.loader_version = latest.version;
        }
        fabric::install(&draft.mc_version, &draft.loader_version, settings.use_mirror).await?
    } else {
        draft.loader = "vanilla".into();
        draft.loader_version.clear();
        draft.mc_version.clone()
    };
    Ok(())
}

#[tauri::command]
pub async fn create_instance(mut draft: Instance) -> R<Instance> {
    let settings = config::load_settings().await;
    resolve_version(&mut draft, &settings).await?;
    let previous = instance::list().await.into_iter().next().map(|i| i.id);
    let created = instance::create(draft).await?;
    if settings.inherit_options {
        let (to, from) = (created.id.clone(), previous);
        let _ = tokio::task::spawn_blocking(move || crate::boost::inherit_settings(&to, from.as_deref())).await;
    }
    Ok(created)
}

#[tauri::command]
pub async fn update_instance(updated: Instance) -> R<()> {
    let current = instance::get(&updated.id).await?;
    let mut updated = updated;
    if updated.mc_version != current.mc_version
        || updated.loader != current.loader
        || updated.loader_version != current.loader_version
    {
        let settings = config::load_settings().await;
        resolve_version(&mut updated, &settings).await?;
    }
    updated.created_at = current.created_at;
    updated.play_seconds = current.play_seconds;
    instance::save(&updated).await
}

#[tauri::command]
pub async fn set_instance_image(id: String, path: String) -> R<Instance> {
    let mut current = instance::get(&id).await?;
    let dir = paths::instance_dir(&id);
    if path.is_empty() {
        for entry in std::fs::read_dir(&dir).map(|d| d.flatten().collect::<Vec<_>>()).unwrap_or_default() {
            if entry.file_name().to_string_lossy().starts_with("cover-") {
                let _ = std::fs::remove_file(entry.path());
            }
        }
        current.image.clear();
    } else {
        current.image = crate::extras::copy_media(std::path::Path::new(&path), &crate::extras::IMAGE_EXTS, 15, &dir, "cover-").await?;
    }
    instance::save(&current).await?;
    Ok(current)
}

#[tauri::command]
pub async fn delete_instance(id: String) -> R<()> {
    instance::delete(&id).await
}

#[tauri::command]
pub async fn duplicate_instance(id: String, name: String) -> R<Instance> {
    instance::duplicate(&id, &name).await
}

#[tauri::command]
pub async fn select_instance(id: String) -> R<()> {
    let mut settings = config::load_settings().await;
    settings.selected_instance = id;
    config::save_settings(&settings).await
}

#[tauri::command]
pub async fn set_mod_enabled(id: String, file: String, enabled: bool) -> R<()> {
    instance::set_mod_enabled(&id, &file, enabled).await
}

#[tauri::command]
pub async fn content_search(id: String, kind: String, query: String, category: String, sort: String, offset: u32) -> R<modrinth::SearchResult> {
    let instance = instance::get(&id).await?;
    modrinth::search(modrinth::Kind::parse(&kind)?, &query, &instance.mc_version, &category, &sort, offset).await
}

#[tauri::command]
pub async fn content_detail(project: String, kind: Option<String>, id: Option<String>) -> R<modrinth::Detail> {
    let kind = kind.as_deref().map(modrinth::Kind::parse).transpose()?;
    let mc = match id {
        Some(id) => instance::get(&id).await.ok().map(|i| i.mc_version),
        None => None,
    };
    modrinth::detail(&project, kind, mc.as_deref()).await
}

#[tauri::command]
pub async fn content_install(app: AppHandle, id: String, kind: String, project: String) -> R<Vec<String>> {
    let settings = config::load_settings().await;
    let instance = instance::get(&id).await?;
    let kind = modrinth::Kind::parse(&kind)?;
    modrinth::install(&app, &id, kind, &project, &instance.mc_version, instance.wants_rich_presence(&settings), instance.channel(&settings)).await
}

#[tauri::command]
pub async fn content_list(id: String, kind: String, mc: Option<String>) -> R<Vec<modrinth::Installed>> {
    let instance = instance::get(&id).await?;
    let mc = mc.filter(|m| !m.is_empty() && m.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-')).unwrap_or_else(|| instance.mc_version.clone());
    let settings = config::load_settings().await;
    modrinth::installed(&id, modrinth::Kind::parse(&kind)?, &mc, &instance.pinned, instance.wants_rich_presence(&settings), instance.channel(&settings), instance.freeze_mods).await
}

#[tauri::command]
pub async fn content_versions(id: String, kind: String, project: String, all: bool) -> R<Vec<modrinth::VersionInfo>> {
    let instance = instance::get(&id).await?;
    modrinth::versions(modrinth::Kind::parse(&kind)?, &project, (!all).then_some(instance.mc_version.as_str())).await
}

#[tauri::command]
pub async fn set_mod_profiles(id: String, profiles: Vec<instance::ModProfile>, active: String) -> R<()> {
    let mut current = instance::get(&id).await?;
    if profiles.len() > 30 || profiles.iter().any(|p| p.name.trim().is_empty() || p.name.chars().count() > 24) {
        bail!("組合名稱不正確");
    }
    current.mod_profiles = profiles;
    current.active_profile = active;
    instance::save(&current).await
}

#[tauri::command]
pub async fn content_pin(id: String, project: String, pinned: bool) -> R<()> {
    let mut current = instance::get(&id).await?;
    current.pinned.retain(|p| *p != project);
    if pinned {
        current.pinned.push(project);
    }
    instance::save(&current).await
}

#[tauri::command]
pub async fn content_update(app: AppHandle, id: String, kind: String, file: String, update: modrinth::Update) -> R<String> {
    modrinth::replace(&app, &id, modrinth::Kind::parse(&kind)?, &file, update).await
}

#[tauri::command]
pub async fn content_remove(id: String, kind: String, file: String) -> R<()> {
    modrinth::remove(&id, modrinth::Kind::parse(&kind)?, &file).await
}

#[tauri::command]
pub async fn launcher_info() -> R<modinject::LauncherInfo> {
    modinject::info().await
}

fn update_dir() -> std::path::PathBuf {
    std::env::temp_dir().join("BrilliantLauncher-update")
}

pub fn update_kind() -> &'static str {
    if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "mac"
    } else if std::env::var_os("APPIMAGE").is_some() && std::env::var_os("FLATPAK_ID").is_none() {
        "linux"
    } else {
        ""
    }
}

pub fn update_suffix(kind: &str) -> Option<&'static str> {
    match kind {
        "windows" => Some("_x64-setup.exe"),
        "mac" => Some("_universal.app.tar.gz"),
        "linux" => Some("_amd64.AppImage"),
        _ => None,
    }
}

fn update_file_ok(file: &str) -> bool {
    update_suffix(update_kind()).is_some_and(|suffix| modinject::safe_file_name(file, "BrilliantLauncher_", suffix))
}

#[tauri::command]
pub fn update_platform() -> String {
    update_kind().to_string()
}

#[cfg(test)]
mod update_tests {
    use super::*;

    #[test]
    fn update_files_match_platform() {
        assert_eq!(update_suffix("mac"), Some("_universal.app.tar.gz"));
        assert_eq!(update_suffix("linux"), Some("_amd64.AppImage"));
        assert_eq!(update_suffix(""), None);
        if cfg!(target_os = "windows") {
            assert!(update_file_ok("BrilliantLauncher_1.0.27_x64-setup.exe"));
            assert!(!update_file_ok("BrilliantLauncher_1.0.27_universal.app.tar.gz"));
            assert!(!update_file_ok("BrilliantLauncher_../x64-setup.exe"));
        }
    }
}

#[cfg(unix)]
fn relaunch_after_exit(target: &std::path::Path, cleanup: &[std::path::PathBuf]) -> R<()> {
    let opener = if cfg!(target_os = "macos") { "open \"$1\"" } else { "nohup \"$1\" >/dev/null 2>&1 &" };
    let script = format!("while kill -0 {} 2>/dev/null; do sleep 0.2; done; {opener}; shift; rm -rf \"$@\"", std::process::id());
    std::process::Command::new("sh")
        .arg("-c")
        .arg(script)
        .arg("sh")
        .arg(target)
        .args(cleanup)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .context("無法重新啟動啟動器")?;
    Ok(())
}

#[cfg(target_os = "macos")]
fn install_update(archive: &std::path::Path) -> R<()> {
    let exe = std::env::current_exe()?;
    let bundle = exe
        .ancestors()
        .nth(3)
        .filter(|p| p.extension().is_some_and(|e| e == "app"))
        .context("找不到啟動器的 .app，請到官網下載新版")?
        .to_path_buf();
    let shown = bundle.to_string_lossy();
    if shown.contains("/AppTranslocation/") || shown.starts_with("/Volumes/") {
        bail!("請先把輝煌啟動器拖進「應用程式」資料夾，再從那裡開啟，之後就能自動更新");
    }
    let parent = bundle.parent().context("找不到啟動器所在的資料夾")?;
    let staging = parent.join(".brilliant-update");
    let _ = std::fs::remove_dir_all(&staging);
    std::fs::create_dir_all(&staging).context("沒有權限更新「應用程式」裡的輝煌啟動器，請到官網下載新版")?;
    let ok = std::process::Command::new("tar").arg("-xzf").arg(archive).arg("-C").arg(&staging).status()?.success();
    let fresh = std::fs::read_dir(&staging)?
        .flatten()
        .map(|e| e.path())
        .find(|p| p.extension().is_some_and(|e| e == "app") && p.join("Contents/MacOS").is_dir());
    let Some(fresh) = fresh.filter(|_| ok) else {
        let _ = std::fs::remove_dir_all(&staging);
        bail!("更新檔內容不完整，請到官網下載新版");
    };
    let old = parent.join(".brilliant-old.app");
    let _ = std::fs::remove_dir_all(&old);
    std::fs::rename(&bundle, &old).context("沒有權限更新「應用程式」裡的輝煌啟動器，請到官網下載新版")?;
    if let Err(e) = std::fs::rename(&fresh, &bundle) {
        let _ = std::fs::rename(&old, &bundle);
        let _ = std::fs::remove_dir_all(&staging);
        bail!("替換啟動器失敗：{e}");
    }
    relaunch_after_exit(&bundle, &[old, staging])
}

#[cfg(target_os = "linux")]
fn install_update(file: &std::path::Path) -> R<()> {
    use std::os::unix::fs::PermissionsExt;
    let target = std::path::PathBuf::from(std::env::var_os("APPIMAGE").context("只有 AppImage 版能自動更新")?);
    let staging = target.with_extension("update");
    std::fs::copy(file, &staging).context("沒有權限更新 AppImage 所在的資料夾，請到官網下載新版")?;
    std::fs::set_permissions(&staging, std::fs::Permissions::from_mode(0o755))?;
    if let Err(e) = std::fs::rename(&staging, &target) {
        let _ = std::fs::remove_file(&staging);
        bail!("替換 AppImage 失敗：{e}");
    }
    relaunch_after_exit(&target, &[])
}

#[cfg(target_os = "windows")]
fn install_update(file: &std::path::Path) -> R<()> {
    std::process::Command::new(file)
        .args(["/S", "/UPDATE", "/R"])
        .spawn()
        .with_context(|| format!("無法執行更新：{}", file.display()))?;
    Ok(())
}

#[derive(serde::Serialize, Clone)]
struct UpdateProgress {
    done: u64,
    total: u64,
}

#[tauri::command]
pub async fn download_update(app: AppHandle, url: String, file: String, sha256: String) -> R<String> {
    if update_suffix(update_kind()).is_none() {
        bail!("請到官網下載新版");
    }
    if !url.starts_with(&format!("{SITE}/api/launcher/download/")) || !update_file_ok(&file) || sha256.len() != 64 {
        bail!("更新資訊不正確，請到官網手動下載新版");
    }
    let path = update_dir().join(&file);
    if let Ok(existing) = tokio::fs::read(&path).await {
        if modinject::sha256_hex(&existing).eq_ignore_ascii_case(&sha256) {
            return Ok(path.to_string_lossy().to_string());
        }
    }
    let mut res = crate::net::client().get(&url).send().await?.error_for_status()?;
    let total = res.content_length().unwrap_or(0);
    let mut bytes: Vec<u8> = Vec::with_capacity(total as usize);
    let mut reported = 0u64;
    while let Some(chunk) = res.chunk().await? {
        bytes.extend_from_slice(&chunk);
        let done = bytes.len() as u64;
        if total == 0 || done - reported >= total / 100 || done == total {
            reported = done;
            let _ = app.emit("launcher://update-progress", UpdateProgress { done, total });
        }
    }
    let got = modinject::sha256_hex(&bytes);
    if !got.eq_ignore_ascii_case(&sha256) {
        bail!("下載的更新檔 SHA-256 不符，已取消更新");
    }
    if let Ok(mut entries) = tokio::fs::read_dir(update_dir()).await {
        while let Ok(Some(entry)) = entries.next_entry().await {
            let _ = tokio::fs::remove_file(entry.path()).await;
        }
    }
    crate::net::write_file(&path, &bytes, false).await?;
    Ok(path.to_string_lossy().to_string())
}

#[tauri::command]
pub async fn apply_update(app: AppHandle, path: String) -> R<()> {
    let path = std::path::PathBuf::from(path);
    let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    if path.parent() != Some(update_dir().as_path()) || !update_file_ok(&name) || !path.is_file() {
        bail!("找不到已下載的更新檔，請重新下載");
    }
    install_update(&path)?;
    app.exit(0);
    Ok(())
}

#[tauri::command]
pub async fn create_shortcut(id: String, join: Option<String>) -> R<String> {
    let instance = instance::get(&id).await?;
    let join = join.filter(|j| j.len() <= 260 && j.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == ':'));
    let path = tokio::task::spawn_blocking(move || crate::advanced::create_shortcut(&instance, join)).await.context("建立捷徑中斷")??;
    Ok(path.to_string_lossy().to_string())
}

#[tauri::command]
pub async fn export_diagnostics(id: String) -> R<String> {
    let instance = instance::get(&id).await?;
    let settings = config::load_settings().await;
    let java = instance.java(&settings).to_string();
    let path = tokio::task::spawn_blocking(move || crate::advanced::export_diagnostics(&instance, &settings, &java)).await.context("匯出中斷")??;
    Ok(path.to_string_lossy().to_string())
}

#[tauri::command]
pub async fn export_config() -> R<String> {
    Ok(crate::advanced::export_config().await?.to_string_lossy().to_string())
}

#[derive(Serialize)]
pub struct ImportSummary {
    added: usize,
    skipped: usize,
    settings: bool,
    notes: Vec<String>,
}

#[tauri::command]
pub async fn import_config(path: String) -> R<ImportSummary> {
    let (added, skipped, settings, notes) = crate::advanced::import_config(&path).await?;
    Ok(ImportSummary { added, skipped, settings, notes })
}

#[tauri::command]
pub async fn add_mods(id: String, files: Vec<String>) -> R<Vec<String>> {
    let mut added = Vec::new();
    for file in files {
        added.push(instance::add_mod(&id, &file).await?);
    }
    Ok(added)
}

#[tauri::command]
pub async fn launch_game(app: AppHandle, id: String, join: Option<String>) -> R<Vec<String>> {
    if let Some(host) = &join {
        let valid = host.len() <= 260 && host.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == ':');
        if !valid {
            bail!("伺服器位址不正確");
        }
    }
    launch::launch(&app, &id, join).await
}

#[tauri::command]
pub async fn stop_game(app: AppHandle, id: String) -> R<()> {
    launch::stop(&app, &id).await
}

#[tauri::command]
pub async fn ping_server(host: String) -> R<ping::ServerStatus> {
    ping::ping(&host).await
}

#[tauri::command]
pub async fn send_report(body: serde_json::Value) -> R<()> {
    let _: serde_json::Value = crate::net::post_json(&format!("{SITE}/api/launcher/report"), &body).await?;
    Ok(())
}

#[tauri::command]
pub async fn site_json(path: String) -> R<serde_json::Value> {
    if !(path.starts_with("/api/") || path.starts_with("/mcstats/api/")) || path.contains("..") || path.contains("://") {
        bail!("不允許的官網路徑：{path}");
    }
    crate::net::get_json(&format!("{SITE}{path}"), false).await
}

#[tauri::command]
pub fn open_path(app: AppHandle, path: String) -> R<()> {
    use tauri_plugin_opener::OpenerExt;
    let target = std::path::PathBuf::from(&path);
    if let Some(parent) = target.parent() {
        let _ = std::fs::create_dir_all(if target.is_dir() { &target } else { parent });
    }
    app.opener().open_path(path, None::<&str>)?;
    Ok(())
}

#[tauri::command]
pub fn open_instance_folder(id: String, sub: String) -> R<std::path::PathBuf> {
    let dir = paths::game_dir(&id);
    let target = if sub.is_empty() { dir } else { dir.join(sub) };
    std::fs::create_dir_all(&target).with_context(|| format!("建立失敗：{}", target.display()))?;
    Ok(target)
}

#[tauri::command]
pub async fn prefetch(app: AppHandle, id: String) -> R<()> {
    launch::prefetch(&app, &id).await
}

#[tauri::command]
pub async fn repair_instance(app: AppHandle, id: String) -> R<()> {
    launch::repair(&app, &id).await
}

#[tauri::command]
pub async fn hide_window(app: AppHandle) -> R<()> {
    if let Some(window) = app.get_webview_window("main") {
        window.hide()?;
    }
    Ok(())
}

#[tauri::command]
pub fn exit_launcher(app: AppHandle) {
    app.exit(0);
}

#[tauri::command]
pub async fn show_window(app: AppHandle) -> R<()> {
    if let Some(window) = app.get_webview_window("main") {
        window.show()?;
        window.set_focus()?;
    }
    Ok(())
}
