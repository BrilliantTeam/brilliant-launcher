use crate::config::{self, Account, Settings};
use crate::err::R;
use crate::instance::Instance;
use crate::mc::{java::JavaInfo, manifest};
use crate::state::AppState;
use crate::{auth, fabric, instance, launch, modinject, modrinth, paths, ping};
use crate::bail;
use anyhow::Context;
use serde::Serialize;
use tauri::{AppHandle, Manager, State};

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
pub async fn content_detail(project: String) -> R<modrinth::Detail> {
    modrinth::detail(&project).await
}

#[tauri::command]
pub async fn content_install(app: AppHandle, id: String, kind: String, project: String) -> R<Vec<String>> {
    let settings = config::load_settings().await;
    let instance = instance::get(&id).await?;
    let kind = modrinth::Kind::parse(&kind)?;
    modrinth::install(&app, &id, kind, &project, &instance.mc_version, instance.wants_rich_presence(&settings)).await
}

#[tauri::command]
pub async fn content_list(id: String, kind: String, mc: Option<String>) -> R<Vec<modrinth::Installed>> {
    let instance = instance::get(&id).await?;
    let mc = mc.filter(|m| !m.is_empty() && m.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-')).unwrap_or_else(|| instance.mc_version.clone());
    let settings = config::load_settings().await;
    modrinth::installed(&id, modrinth::Kind::parse(&kind)?, &mc, &instance.pinned, instance.wants_rich_presence(&settings)).await
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

#[tauri::command]
pub async fn install_update(app: AppHandle, url: String, file: String, sha256: String) -> R<()> {
    if !url.starts_with(&format!("{SITE}/api/launcher/download/"))
        || !modinject::safe_file_name(&file, "BrilliantLauncher_", "_x64-setup.exe")
        || sha256.len() != 64
    {
        bail!("更新資訊不正確，請到官網手動下載新版");
    }
    let bytes = crate::net::get_bytes(&url, false).await?;
    let got = modinject::sha256_hex(&bytes);
    if !got.eq_ignore_ascii_case(&sha256) {
        bail!("下載的安裝檔 SHA-256 不符，已取消更新");
    }
    let path = std::env::temp_dir().join("BrilliantLauncher-update").join(&file);
    crate::net::write_file(&path, &bytes, false).await?;
    std::process::Command::new(&path).spawn().with_context(|| format!("無法執行安裝檔：{}", path.display()))?;
    app.exit(0);
    Ok(())
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
pub async fn launch_game(app: AppHandle, id: String) -> R<Vec<String>> {
    launch::launch(&app, &id).await
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
