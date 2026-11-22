use crate::bail;
use crate::config::Settings;
use crate::err::R;
use crate::instance::Instance;
use crate::paths;
use anyhow::Context;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Emitter};

pub const SHAREABLE: [&str; 4] = ["saves", "resourcepacks", "shaderpacks", "screenshots"];
const CONFIG_ROOT: &str = "brilliant-launcher-config";

pub fn parse_env(text: &str) -> Vec<(String, String)> {
    text.lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                return None;
            }
            let (key, value) = line.split_once('=')?;
            let key = key.trim();
            let valid = key.chars().next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
                && key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
            valid.then(|| (key.to_string(), value.trim().to_string()))
        })
        .collect()
}

pub fn drop_removed_flags(args: Vec<String>, java_major: u32) -> Vec<String> {
    let removed = crate::hotfix::data::<crate::config::JvmData>("jvm.json").removed_flags;
    args.into_iter().filter(|a| !removed.iter().any(|r| java_major >= r.since_java && a.ends_with(&r.suffix))).collect()
}

pub fn split_args(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut quoted = false;
    let mut started = false;
    for c in text.chars() {
        match c {
            '"' => {
                quoted = !quoted;
                started = true;
            }
            c if c.is_whitespace() && !quoted => {
                if started {
                    out.push(std::mem::take(&mut current));
                    started = false;
                }
            }
            c => {
                current.push(c);
                started = true;
            }
        }
    }
    if started {
        out.push(current);
    }
    out
}

pub fn java_component(major: u32) -> String {
    crate::hotfix::data::<crate::config::JvmData>("jvm.json").java_components.remove(&major.to_string()).unwrap_or_else(|| format!("java-{major}"))
}

pub fn proxy_props(proxy: &str) -> Vec<String> {
    let proxy = proxy.trim();
    if proxy.is_empty() || !crate::net::valid_proxy(proxy) {
        return Vec::new();
    }
    let rest = proxy.split_once("://").map(|(_, r)| r).unwrap_or(proxy).trim_end_matches('/');
    let rest = rest.rsplit_once('@').map(|(_, r)| r).unwrap_or(rest);
    let Some((host, port)) = rest.rsplit_once(':') else {
        return Vec::new();
    };
    ["http", "https"].iter().flat_map(|p| [format!("-D{p}.proxyHost={host}"), format!("-D{p}.proxyPort={port}")]).collect()
}

pub fn prefer_dgpu(cmd: &mut tokio::process::Command, java: &Path) {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        let _ = cmd;
        let _ = std::process::Command::new("reg")
            .args(["add", r"HKCU\Software\Microsoft\DirectX\UserGpuPreferences", "/v"])
            .arg(java.as_os_str())
            .args(["/t", "REG_SZ", "/d", "GpuPreference=2;", "/f"])
            .creation_flags(0x0800_0000)
            .status();
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = java;
        cmd.env("DRI_PRIME", "1")
            .env("__NV_PRIME_RENDER_OFFLOAD", "1")
            .env("__GLX_VENDOR_LIBRARY_NAME", "nvidia")
            .env("__VK_LAYER_NV_optimus", "NVIDIA_only");
    }
}

pub fn is_link(path: &Path) -> bool {
    std::fs::symlink_metadata(path).map(|m| m.file_type().is_symlink()).unwrap_or(false)
}

pub fn unlink_shared(game: &Path) -> std::io::Result<()> {
    for name in SHAREABLE {
        let link = game.join(name);
        if is_link(&link) {
            remove_link(&link)?;
        }
    }
    Ok(())
}

fn remove_link(path: &Path) -> std::io::Result<()> {
    if cfg!(target_os = "windows") {
        std::fs::remove_dir(path)
    } else {
        std::fs::remove_file(path)
    }
}

fn make_link(link: &Path, target: &Path) -> R<()> {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        let (l, t) = (link.to_string_lossy(), target.to_string_lossy());
        if [&l, &t].iter().any(|p| p.contains(['%', '"'])) {
            bail!("資料夾路徑含有 % 或 \" 符號，Windows 無法為它建立共用連結：{}", link.display());
        }
        let status = std::process::Command::new("cmd")
            .arg("/C")
            .raw_arg(format!("mklink /J \"{l}\" \"{t}\""))
            .creation_flags(0x0800_0000)
            .stdout(std::process::Stdio::null())
            .status()
            .context("無法建立共用資料夾連結")?;
        if !status.success() {
            bail!("建立共用資料夾連結失敗：{}", link.display());
        }
    }
    #[cfg(not(target_os = "windows"))]
    std::os::unix::fs::symlink(target, link).with_context(|| format!("建立共用資料夾連結失敗：{}", link.display()))?;
    Ok(())
}

fn copy_dir(src: &Path, dst: &Path) -> R<()> {
    std::fs::create_dir_all(dst).with_context(|| format!("建立 {} 失敗", dst.display()))?;
    for entry in std::fs::read_dir(src)?.flatten() {
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if entry.file_type().is_ok_and(|t| t.is_symlink()) {
            continue;
        }
        if from.is_dir() {
            copy_dir(&from, &to)?;
        } else {
            std::fs::copy(&from, &to).with_context(|| format!("複製 {} 失敗", from.display()))?;
        }
    }
    Ok(())
}

fn move_into(src: &Path, dst: &Path, tag: &str) -> R<()> {
    std::fs::create_dir_all(dst).with_context(|| format!("建立 {} 失敗", dst.display()))?;
    for entry in std::fs::read_dir(src)?.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        let mut target = dst.join(&name);
        if target.exists() {
            let (stem, ext) = match name.rsplit_once('.') {
                Some((s, e)) if !entry.path().is_dir() => (s.to_string(), format!(".{e}")),
                _ => (name.clone(), String::new()),
            };
            target = dst.join(format!("{stem} ({tag}){ext}"));
            if target.exists() {
                bail!("共用資料夾已有同名項目：{}", target.display());
            }
        }
        if std::fs::rename(entry.path(), &target).is_err() {
            let from = entry.path();
            if from.is_dir() {
                copy_dir(&from, &target)?;
                std::fs::remove_dir_all(&from).with_context(|| format!("把 {} 搬進共用資料夾後，無法刪除原本的資料夾", from.display()))?;
            } else {
                std::fs::copy(&from, &target).with_context(|| format!("把 {} 搬進共用資料夾失敗", from.display()))?;
                std::fs::remove_file(&from).with_context(|| format!("把 {} 搬進共用資料夾後，無法刪除原本的檔案", from.display()))?;
            }
        }
    }
    std::fs::remove_dir(src).with_context(|| format!("整理 {} 失敗", src.display()))?;
    Ok(())
}

#[tauri::command]
pub async fn shared_folder_path(key: String) -> R<String> {
    if !valid_key(&key) {
        bail!("共用資料夾不正確");
    }
    let dir = group_base(&key);
    std::fs::create_dir_all(&dir)?;
    Ok(dir.to_string_lossy().to_string())
}

#[tauri::command]
pub async fn apply_shared(state: tauri::State<'_, crate::state::AppState>, ids: Vec<String>) -> R<Vec<(String, Vec<String>)>> {
    let settings = crate::config::load_settings().await;
    let mut out = Vec::new();
    for id in ids {
        let instance = crate::instance::get(&id).await?;
        if state.is_running(&id).await {
            out.push((instance.name, vec!["遊戲執行中，會在下次啟動時套用".to_string()]));
            continue;
        }
        let s = settings.clone();
        let inst = instance.clone();
        let notes = tokio::task::spawn_blocking(move || sync_shared(&inst, &s)).await.context("共用資料夾工作中斷")??;
        out.push((instance.name, notes));
    }
    Ok(out)
}

#[derive(serde::Serialize)]
pub struct ShareStatus {
    pub id: String,
    pub linked: Vec<String>,
    pub pending: Vec<String>,
}

pub fn share_status(instance: &Instance, settings: &Settings) -> ShareStatus {
    let game = paths::game_dir(&instance.id);
    let group = settings.instance_groups.iter().find(|g| !instance.group.is_empty() && g.name == instance.group && !g.key.is_empty());
    let mut linked = Vec::new();
    let mut pending = Vec::new();
    let synced = !links_supported(&game);
    let state_dir = paths::instance_dir(&instance.id).join(".shared-sync");
    for name in SHAREABLE {
        let link = game.join(name);
        let want = group.filter(|g| g.shared.iter().any(|f| f == name)).map(|g| group_base(&g.key).join(name));
        let current = if is_link(&link) {
            std::fs::read_link(&link).ok()
        } else if synced && link.is_dir() && state_dir.join(format!("{name}.json")).is_file() {
            want.clone()
        } else {
            None
        };
        match (&want, &current) {
            (Some(w), Some(c)) if same_dir(w, c) => linked.push(name.to_string()),
            (None, None) => {}
            _ => pending.push(name.to_string()),
        }
    }
    ShareStatus { id: instance.id.clone(), linked, pending }
}

#[tauri::command]
pub async fn shared_status(ids: Vec<String>) -> R<Vec<ShareStatus>> {
    let settings = crate::config::load_settings().await;
    let mut out = Vec::new();
    for id in ids {
        if let Ok(instance) = crate::instance::get(&id).await {
            let s = settings.clone();
            out.push(tokio::task::spawn_blocking(move || share_status(&instance, &s)).await.context("讀取共用狀態中斷")?);
        }
    }
    Ok(out)
}

#[derive(serde::Serialize)]
pub struct SharedItem {
    pub name: String,
    pub title: String,
    pub size: u64,
    pub modified: i64,
    pub dir: bool,
    pub icon: Option<String>,
    pub path: String,
}

fn valid_key(key: &str) -> bool {
    !key.is_empty() && key.len() <= 32 && key.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

fn shared_dir(key: &str, folder: &str) -> R<PathBuf> {
    if !valid_key(key) || !SHAREABLE.contains(&folder) {
        bail!("共用資料夾不正確");
    }
    Ok(group_base(key).join(folder))
}

fn safe_item(name: &str) -> bool {
    !name.is_empty() && !name.contains(['/', '\\']) && name != "." && name != ".."
}

pub async fn sync_group(state: &crate::state::AppState, settings: &Settings, group: &str, except: &str) {
    if group.is_empty() || links_supported(&paths::instances()) {
        return;
    }
    for other in crate::instance::list().await.into_iter().filter(|i| i.group == group && i.id != except) {
        if state.is_running(&other.id).await {
            continue;
        }
        let s = settings.clone();
        if let Ok(Err(e)) = tokio::task::spawn_blocking(move || sync_shared(&other, &s)).await {
            eprintln!("[shared] 同步分組其他實例失敗：{:#}", e.0);
        }
    }
}

#[tauri::command]
pub async fn shared_items(state: tauri::State<'_, crate::state::AppState>, key: String, folder: String) -> R<Vec<SharedItem>> {
    let dir = shared_dir(&key, &folder)?;
    let settings = crate::config::load_settings().await;
    if let Some(group) = settings.instance_groups.iter().find(|g| g.key == key) {
        sync_group(&state, &settings, &group.name, "").await;
    }
    tokio::task::spawn_blocking(move || {
        let mut out = Vec::new();
        for entry in std::fs::read_dir(&dir).map(|d| d.flatten().collect::<Vec<_>>()).unwrap_or_default() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            let Ok(meta) = entry.metadata() else { continue };
            let modified = meta.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_secs() as i64).unwrap_or(0);
            let lower = name.to_ascii_lowercase();
            let (keep, title, icon) = match folder.as_str() {
                "saves" => {
                    let level = path.join("level.dat");
                    let title = crate::worlds::read_level_file(&level).0.unwrap_or_else(|| name.clone());
                    let icon = path.join("icon.png");
                    (level.is_file(), title, icon.is_file().then(|| icon.to_string_lossy().to_string()))
                }
                "screenshots" => (lower.ends_with(".png") || lower.ends_with(".jpg"), name.clone(), None),
                _ => {
                    let pack = meta.is_dir() || lower.ends_with(".zip");
                    let icon = if !meta.is_dir() && meta.len() < 64 << 20 {
                        std::fs::read(&path).ok().and_then(|b| crate::modrinth::embedded_icon(&b, false))
                    } else if meta.is_dir() && std::fs::metadata(path.join("pack.png")).is_ok_and(|m| m.is_file() && m.len() < 2 << 20) {
                        std::fs::read(path.join("pack.png")).ok().map(|b| {
                            use base64::Engine;
                            format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(b))
                        })
                    } else {
                        None
                    };
                    (pack, name.trim_end_matches(".zip").to_string(), icon)
                }
            };
            if !keep {
                continue;
            }
            let size = if meta.is_dir() { crate::worlds::dir_size(&path) } else { meta.len() };
            out.push(SharedItem { name, title, size, modified, dir: meta.is_dir(), icon, path: path.to_string_lossy().to_string() });
        }
        out.sort_by(|a, b| b.modified.cmp(&a.modified));
        out
    })
    .await
    .context("讀取共用資料夾中斷")
    .map_err(Into::into)
}

#[tauri::command]
pub async fn shared_add(key: String, folder: String, files: Vec<String>) -> R<usize> {
    let dir = shared_dir(&key, &folder)?;
    if !matches!(folder.as_str(), "resourcepacks" | "shaderpacks") {
        bail!("這個共用資料夾不能直接加入檔案");
    }
    std::fs::create_dir_all(&dir)?;
    let picked: Vec<(PathBuf, String)> = files
        .iter()
        .map(PathBuf::from)
        .filter_map(|src| {
            let name = src.file_name()?.to_string_lossy().to_string();
            (safe_item(&name) && name.to_ascii_lowercase().ends_with(".zip") && src.is_file()).then_some((src, name))
        })
        .collect();
    let mut seen = std::collections::HashSet::new();
    let clash: Vec<&str> = picked.iter().filter(|(_, n)| dir.join(n).exists() || !seen.insert(n.as_str())).map(|(_, n)| n.as_str()).collect();
    if !clash.is_empty() {
        bail!("共用資料夾裡已經有同名的檔案，為了避免覆蓋已取消加入：{}", clash.join("、"));
    }
    for (src, name) in &picked {
        std::fs::copy(src, dir.join(name)).with_context(|| format!("複製 {name} 失敗"))?;
    }
    Ok(picked.len())
}

#[tauri::command]
pub async fn shared_remove(state: tauri::State<'_, crate::state::AppState>, key: String, folder: String, names: Vec<String>) -> R<usize> {
    let dir = shared_dir(&key, &folder)?;
    let settings = crate::config::load_settings().await;
    if let Some(group) = settings.instance_groups.iter().find(|g| g.key == key) {
        for instance in crate::instance::list().await.into_iter().filter(|i| i.group == group.name) {
            if state.ensure_idle(&instance.id).await.is_err() {
                bail!("「{}」正在執行，共用的檔案被遊戲使用著，請先關閉遊戲再移除", instance.name);
            }
        }
    }
    let items: Vec<(PathBuf, &str)> = names.iter().filter(|n| safe_item(n) && dir.join(n).exists()).map(|n| (dir.join(n), n.as_str())).collect();
    if !items.is_empty() {
        paths::discard(&format!("共用{folder}"), &items).context("移到回收資料夾失敗")?;
    }
    Ok(items.len())
}

#[tauri::command]
pub async fn shared_discard(key: String, name: String) -> R<&'static str> {
    if !valid_key(&key) {
        bail!("共用資料夾不正確");
    }
    let settings = crate::config::load_settings().await;
    if settings.instance_groups.iter().any(|g| g.key == key) {
        bail!("這個分組還在使用共用資料夾");
    }
    let base = group_base(&key);
    if !base.exists() {
        return Ok("none");
    }
    for instance in crate::instance::list().await {
        let game = paths::game_dir(&instance.id);
        for folder in SHAREABLE {
            let link = game.join(folder);
            if is_link(&link) && std::fs::read_link(&link).is_ok_and(|t| t.starts_with(&base) || same_dir(&t, &base.join(folder))) {
                return Ok("busy");
            }
        }
    }
    tokio::task::spawn_blocking(move || paths::discard(&format!("分組「{name}」的共用資料夾"), &[(&base, "shared")]))
        .await
        .context("整理共用資料夾中斷")?
        .context("把共用資料夾移到回收資料夾失敗")?;
    Ok("moved")
}

pub fn group_base(key: &str) -> PathBuf {
    if key == "legacy" {
        paths::shared().join("folders")
    } else {
        paths::shared().join("groups").join(key)
    }
}

fn same_dir(a: &Path, b: &Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(x), Ok(y)) => x == y,
        _ => a == b,
    }
}

fn links_supported(dir: &Path) -> bool {
    if cfg!(target_os = "windows") {
        return true;
    }
    let probe = dir.join(".brilliant-link-test");
    let _ = std::fs::remove_file(&probe);
    #[cfg(not(target_os = "windows"))]
    let ok = std::os::unix::fs::symlink(dir, &probe).is_ok();
    #[cfg(target_os = "windows")]
    let ok = true;
    let _ = std::fs::remove_file(&probe);
    ok
}

type Snapshot = std::collections::BTreeMap<String, (u64, i64)>;

fn snapshot(root: &Path) -> Snapshot {
    fn walk(root: &Path, dir: &Path, out: &mut Snapshot) {
        let Ok(entries) = std::fs::read_dir(dir) else { return };
        for entry in entries.flatten() {
            let Ok(kind) = entry.file_type() else { continue };
            let path = entry.path();
            if kind.is_dir() {
                walk(root, &path, out);
            } else if kind.is_file() {
                let Ok(meta) = entry.metadata() else { continue };
                let modified = meta.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map_or(0, |d| d.as_secs() as i64);
                let rel = path.strip_prefix(root).map(|p| p.to_string_lossy().replace('\\', "/")).unwrap_or_default();
                out.insert(rel, (meta.len(), modified));
            }
        }
    }
    let mut out = Snapshot::new();
    walk(root, root, &mut out);
    out
}

fn copy_stamped(from: &Path, to: &Path) -> R<()> {
    if let Some(parent) = to.parent() {
        std::fs::create_dir_all(parent).with_context(|| format!("建立 {} 失敗", parent.display()))?;
    }
    std::fs::copy(from, to).with_context(|| format!("同步 {} 失敗", from.display()))?;
    if let Ok(modified) = std::fs::metadata(from).and_then(|m| m.modified()) {
        let _ = std::fs::File::options().write(true).open(to).and_then(|f| f.set_modified(modified));
    }
    Ok(())
}

fn prune_empty(dir: &Path, root: bool) -> bool {
    let Ok(entries) = std::fs::read_dir(dir) else { return false };
    let mut empty = true;
    for entry in entries.flatten() {
        let path = entry.path();
        if entry.file_type().is_ok_and(|t| t.is_dir()) && prune_empty(&path, false) {
            continue;
        }
        empty = false;
    }
    empty && !root && std::fs::remove_dir(dir).is_ok()
}

fn conflict_name(rel: &str, tag: &str) -> String {
    let (dir, file) = rel.rsplit_once('/').map_or(("", rel), |(d, f)| (d, f));
    let renamed = match file.rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() => format!("{stem} ({tag}).{ext}"),
        _ => format!("{file} ({tag})"),
    };
    if dir.is_empty() { renamed } else { format!("{dir}/{renamed}") }
}

fn sync_folder(local: &Path, shared: &Path, state: &Path, tag: &str) -> R<()> {
    let previous: Option<Snapshot> = std::fs::read(state).ok().and_then(|b| serde_json::from_slice(&b).ok());
    let local_now = snapshot(local);
    let shared_now = snapshot(shared);
    for (rel, stamp) in &local_now {
        if previous.as_ref().and_then(|p| p.get(rel)) == Some(stamp) {
            continue;
        }
        match shared_now.get(rel) {
            None => copy_stamped(&local.join(rel), &shared.join(rel))?,
            Some(theirs) if theirs == stamp => {}
            Some(theirs) => {
                let untouched = previous.as_ref().and_then(|p| p.get(rel)) == Some(theirs);
                if previous.is_none() {
                    copy_stamped(&local.join(rel), &shared.join(conflict_name(rel, tag)))?;
                } else if untouched || stamp.1 >= theirs.1 {
                    copy_stamped(&local.join(rel), &shared.join(rel))?;
                }
            }
        }
    }
    if let Some(previous) = &previous {
        for (rel, stamp) in previous {
            if !local_now.contains_key(rel) && shared_now.get(rel) == Some(stamp) {
                let _ = std::fs::remove_file(shared.join(rel));
            }
        }
    }
    prune_empty(shared, true);
    let shared_now = snapshot(shared);
    let local_now = snapshot(local);
    for (rel, stamp) in &shared_now {
        if local_now.get(rel) != Some(stamp) {
            copy_stamped(&shared.join(rel), &local.join(rel))?;
        }
    }
    for rel in local_now.keys() {
        if !shared_now.contains_key(rel) {
            let _ = std::fs::remove_file(local.join(rel));
        }
    }
    prune_empty(local, true);
    if let Some(parent) = state.parent() {
        std::fs::create_dir_all(parent).with_context(|| format!("建立 {} 失敗", parent.display()))?;
    }
    std::fs::write(state, serde_json::to_vec(&snapshot(local))?).with_context(|| format!("寫入 {} 失敗", state.display()))?;
    Ok(())
}

pub fn sync_shared(instance: &Instance, settings: &Settings) -> R<Vec<String>> {
    let game = paths::game_dir(&instance.id);
    std::fs::create_dir_all(&game).with_context(|| format!("建立 {} 失敗", game.display()))?;
    let group = settings.instance_groups.iter().find(|g| !instance.group.is_empty() && g.name == instance.group && !g.key.is_empty());
    let mut notes = Vec::new();
    if !links_supported(&game) {
        let state_dir = paths::instance_dir(&instance.id).join(".shared-sync");
        for name in SHAREABLE {
            let local = game.join(name);
            let state = state_dir.join(format!("{name}.json"));
            let want = group.filter(|g| g.shared.iter().any(|f| f == name)).map(|g| group_base(&g.key).join(name));
            if is_link(&local) {
                remove_link(&local).with_context(|| format!("移除共用連結失敗：{}", local.display()))?;
            }
            match want {
                Some(shared) => {
                    std::fs::create_dir_all(&local).with_context(|| format!("建立 {} 失敗", local.display()))?;
                    std::fs::create_dir_all(&shared).with_context(|| format!("建立共用資料夾 {} 失敗", shared.display()))?;
                    sync_folder(&local, &shared, &state, &instance.name)?;
                }
                None => {
                    let _ = std::fs::remove_file(&state);
                }
            }
        }
        return Ok(notes);
    }
    for name in SHAREABLE {
        let link = game.join(name);
        let want = group.filter(|g| g.shared.iter().any(|f| f == name)).map(|g| group_base(&g.key).join(name));
        let current = if is_link(&link) { std::fs::read_link(&link).ok().or_else(|| Some(PathBuf::new())) } else { None };
        if let (Some(w), Some(c)) = (&want, &current) {
            if same_dir(w, c) {
                continue;
            }
        }
        if let Some(old) = &current {
            remove_link(&link).with_context(|| format!("移除共用連結失敗：{}", link.display()))?;
            if old.is_dir() {
                copy_dir(old, &link)?;
            } else {
                std::fs::create_dir_all(&link).with_context(|| format!("建立 {} 失敗", link.display()))?;
            }
            if want.is_none() {
                notes.push(format!("「{name}」已改回這個實例自己的資料夾（複製了一份共用內容）"));
            }
        }
        if let Some(target) = &want {
            std::fs::create_dir_all(target).with_context(|| format!("建立共用資料夾 {} 失敗", target.display()))?;
            let state = paths::instance_dir(&instance.id).join(".shared-sync").join(format!("{name}.json"));
            if state.is_file() && link.is_dir() && !is_link(&link) {
                sync_folder(&link, target, &state, &instance.name)?;
                std::fs::remove_dir_all(&link).with_context(|| format!("改回共用連結時無法移除 {}", link.display()))?;
                let _ = std::fs::remove_file(&state);
            }
            if link.exists() {
                move_into(&link, target, &instance.name)?;
            }
            make_link(&link, target)?;
            notes.push(format!("「{name}」已改用分組「{}」共用的資料夾", instance.group));
        }
    }
    Ok(notes)
}

pub fn redact(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut hide_next = false;
    for word in text.split_inclusive(char::is_whitespace) {
        let bare = word.trim_end();
        let tail = &word[bare.len()..];
        if bare.is_empty() {
            out.push_str(word);
            continue;
        }
        if hide_next || bare.starts_with("token:") || (bare.starts_with("eyJ") && bare.len() > 40) {
            out.push_str("<已隱藏>");
            out.push_str(tail);
            hide_next = false;
            continue;
        }
        hide_next = matches!(bare, "--accessToken" | "--session" | "--xuid");
        out.push_str(word);
    }
    out
}

fn newest(dir: &Path, filter: impl Fn(&str) -> bool, count: usize) -> Vec<PathBuf> {
    let mut files: Vec<(std::time::SystemTime, PathBuf)> = std::fs::read_dir(dir)
        .map(|d| {
            d.flatten()
                .filter(|e| filter(&e.file_name().to_string_lossy()))
                .filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.path())))
                .collect()
        })
        .unwrap_or_default();
    files.sort_by(|a, b| b.0.cmp(&a.0));
    files.into_iter().take(count).map(|(_, p)| p).collect()
}

fn add_text(zip: &mut zip::ZipWriter<std::fs::File>, name: &str, text: &str) -> R<()> {
    let options = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    zip.start_file(name, options)?;
    zip.write_all(text.as_bytes())?;
    Ok(())
}

fn add_file(zip: &mut zip::ZipWriter<std::fs::File>, name: &str, path: &Path, limit: u64) -> R<()> {
    let mut text = String::new();
    let file = std::fs::File::open(path)?;
    let len = file.metadata().map(|m| m.len()).unwrap_or(0);
    let mut reader = std::io::BufReader::new(file);
    if len > limit {
        std::io::Seek::seek(&mut reader, std::io::SeekFrom::Start(len - limit))?;
    }
    let mut bytes = Vec::new();
    reader.read_to_end(&mut bytes)?;
    text.push_str(&String::from_utf8_lossy(&bytes));
    add_text(zip, name, &crate::insight::scrub_report(&text))
}

fn safe_name(text: &str) -> String {
    let cleaned: String = text.chars().map(|c| if "\\/:*?\"<>|".contains(c) || c.is_control() { '_' } else { c }).collect();
    cleaned.trim().chars().take(40).collect()
}

fn export_dir() -> PathBuf {
    paths::exports()
}

pub fn export_diagnostics(instance: &Instance, settings: &Settings, java: &str) -> R<PathBuf> {
    let dir = export_dir();
    std::fs::create_dir_all(&dir)?;
    let path = paths::unused(dir.join(format!("輝煌啟動器診斷-{}-{}.zip", safe_name(&instance.name), crate::config::now_secs())));
    let mut zip = zip::ZipWriter::new(std::fs::File::create(&path)?);
    let game = paths::game_dir(&instance.id);
    let mods: Vec<String> = std::fs::read_dir(game.join("mods"))
        .map(|d| d.flatten().map(|e| e.file_name().to_string_lossy().to_string()).collect())
        .unwrap_or_default();
    let mut sorted = mods;
    sorted.sort();
    let summary = format!(
        "輝煌啟動器 {}\n作業系統：{} {}\n實例：{}（{} {} {}）\nJava：{}\n記憶體上限：{} MB\nJVM 參數：{}\n包裝指令：{}\n模組更新管道：{}\n共用資料夾：{}\n\n模組（{} 個）：\n{}\n",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        std::env::consts::ARCH,
        instance.name,
        instance.mc_version,
        instance.loader,
        instance.loader_version,
        if java.is_empty() { "自動" } else { java },
        instance.memory(settings),
        instance.jvm(settings),
        instance.wrapper_cmd(settings).join(" "),
        instance.channel(settings),
        settings.instance_groups.iter().find(|g| !instance.group.is_empty() && g.name == instance.group && !g.shared.is_empty()).map(|g| format!("分組「{}」共用 {}", g.name, g.shared.join("、"))).unwrap_or_else(|| "未使用".into()),
        sorted.len(),
        sorted.join("\n")
    );
    add_text(&mut zip, "摘要.txt", &crate::insight::scrub_report(&summary))?;
    for (name, value) in [("instance.json", serde_json::to_value(instance)), ("launcher-settings.json", serde_json::to_value(settings))] {
        if let Ok(mut value) = value {
            crate::insight::strip_secrets(&mut value);
            add_text(&mut zip, name, &crate::insight::scrub_report(&serde_json::to_string_pretty(&value).unwrap_or_default()))?;
        }
    }
    let latest = game.join("logs").join("latest.log");
    if latest.is_file() {
        add_file(&mut zip, "game/latest.log", &latest, 4 << 20)?;
    }
    for crash in newest(&game.join("crash-reports"), |n| n.ends_with(".txt"), 3) {
        let name = crash.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        add_file(&mut zip, &format!("crash-reports/{name}"), &crash, 2 << 20)?;
    }
    let prefix = format!("{}-", instance.id);
    for log in newest(&paths::logs(), |n| n.starts_with(&prefix), 3) {
        let name = log.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        add_file(&mut zip, &format!("launcher-logs/{name}"), &log, 4 << 20)?;
    }
    zip.finish()?;
    Ok(path)
}

const MEDIA_FILE_MAX: u64 = 500 << 20;
const PORTABLE_DIRS: [&str; 2] = ["media", "skins"];

fn platform() -> &'static str {
    if cfg!(target_os = "android") { "android" } else { "desktop" }
}

fn portable_rel(root: &Path, path: &str) -> Option<String> {
    let rel = Path::new(path).strip_prefix(root).ok()?.to_string_lossy().replace('\\', "/");
    let safe = !rel.split('/').any(|c| c.is_empty() || c == "." || c == "..");
    (safe && PORTABLE_DIRS.iter().any(|d| rel.starts_with(&format!("{d}/")))).then_some(rel)
}

fn rel_from(path: &str, source_root: &str) -> Option<String> {
    let normalized = path.replace('\\', "/");
    let root = source_root.replace('\\', "/").trim_end_matches('/').to_string() + "/";
    let rest = normalized.get(root.len()..).filter(|_| normalized.get(..root.len()).is_some_and(|head| head.eq_ignore_ascii_case(&root)))?;
    (!rest.is_empty() && !rest.split('/').any(|c| c.is_empty() || c == "." || c == "..")).then(|| rest.to_string())
}

fn relocate(path: &str, source_root: Option<&str>) -> String {
    if path.is_empty() {
        return String::new();
    }
    let local = source_root.and_then(|r| rel_from(path, r)).map(|rel| paths::root().join(rel)).unwrap_or_else(|| PathBuf::from(path));
    if local.is_file() { local.to_string_lossy().to_string() } else { String::new() }
}

pub struct ImportOutcome {
    pub added: usize,
    pub skipped: usize,
    pub settings: bool,
    pub notes: Vec<String>,
    pub web: Option<serde_json::Value>,
    pub game: Option<String>,
    pub source_platform: String,
}

pub async fn export_config(web: serde_json::Value, game: Option<String>) -> R<PathBuf> {
    let settings = crate::config::load_settings().await;
    let instances = crate::instance::list().await;
    let root = paths::root();
    let dir = export_dir();
    std::fs::create_dir_all(&dir)?;
    let path = paths::unused(dir.join(format!("輝煌啟動器設定-{}.zip", crate::config::now_secs())));
    let mut zip = zip::ZipWriter::new(std::fs::File::create(&path)?);
    let meta = serde_json::json!({ "os": std::env::consts::OS, "platform": platform(), "version": env!("CARGO_PKG_VERSION"), "root": root.to_string_lossy() });
    add_text(&mut zip, &format!("{CONFIG_ROOT}/meta.json"), &meta.to_string())?;
    add_text(&mut zip, &format!("{CONFIG_ROOT}/config.json"), &serde_json::to_string_pretty(&settings)?)?;
    for instance in &instances {
        add_text(&mut zip, &format!("{CONFIG_ROOT}/instances/{}/instance.json", instance.id), &serde_json::to_string_pretty(instance)?)?;
    }
    let skins = crate::skins::load_library();
    let mut media: Vec<String> = vec![settings.background.clone(), settings.music.clone(), settings.custom_font.clone()];
    media.extend(settings.fonts.iter().chain(&settings.playlist).map(|m| m.path.clone()));
    media.extend(instances.iter().map(|i| i.image.clone()));
    media.extend(skins.iter().map(|s| root.join("skins").join(&s.file).to_string_lossy().to_string()));
    let files: std::collections::BTreeSet<String> = media.iter().filter_map(|p| portable_rel(&root, p)).collect();
    let options = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated).large_file(true);
    for rel in files {
        let src = root.join(&rel);
        if !src.is_file() || std::fs::metadata(&src)?.len() > MEDIA_FILE_MAX {
            continue;
        }
        zip.start_file(format!("{CONFIG_ROOT}/files/{rel}"), options)?;
        std::io::copy(&mut std::fs::File::open(&src)?, &mut zip)?;
    }
    add_text(&mut zip, &format!("{CONFIG_ROOT}/skins.json"), &serde_json::to_string(&skins)?)?;
    if web.is_object() {
        add_text(&mut zip, &format!("{CONFIG_ROOT}/web.json"), &web.to_string())?;
    }
    if let Some(game) = game.filter(|g| !g.is_empty()) {
        add_text(&mut zip, &format!("{CONFIG_ROOT}/game.json"), &game)?;
    }
    zip.finish()?;
    Ok(path)
}

pub fn fit_memory(mb: u32, total_mb: u64) -> u32 {
    if total_mb == 0 || u64::from(mb) * 10 <= total_mb * 7 {
        mb
    } else {
        crate::boost::recommended_memory(total_mb)
    }
}

pub async fn import_config(path: &str) -> R<ImportOutcome> {
    let file = std::fs::File::open(path).with_context(|| format!("無法開啟 {path}"))?;
    let mut archive = zip::ZipArchive::new(file).context("這不是輝煌啟動器匯出的設定檔")?;
    let root = paths::root();
    let files_prefix = format!("{CONFIG_ROOT}/files/");
    let mut settings_json: Option<String> = None;
    let mut meta = serde_json::Value::Null;
    let mut instances: Vec<(String, String)> = Vec::new();
    let (mut skins_json, mut web, mut game) = (None, None, None);
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;
        let name = entry.name().replace('\\', "/");
        if let Some(rel) = name.strip_prefix(&files_prefix) {
            let safe = !rel.split('/').any(|c| c.is_empty() || c == "." || c == "..") && PORTABLE_DIRS.iter().any(|d| rel.starts_with(&format!("{d}/")));
            let dest = root.join(rel);
            if !safe || dest.exists() || entry.size() > MEDIA_FILE_MAX {
                continue;
            }
            std::fs::create_dir_all(dest.parent().unwrap_or(&root))?;
            let copied = std::fs::File::create(&dest).map_err(anyhow::Error::from).and_then(|mut out| Ok(std::io::copy(&mut (&mut entry).take(MEDIA_FILE_MAX), &mut out)?));
            if copied.is_err() {
                let _ = std::fs::remove_file(&dest);
            }
            continue;
        }
        if entry.size() > 4 << 20 {
            continue;
        }
        let mut text = String::new();
        if (&mut entry).take(4 << 20).read_to_string(&mut text).is_err() {
            continue;
        }
        if name == format!("{CONFIG_ROOT}/config.json") {
            settings_json = Some(text);
        } else if name == format!("{CONFIG_ROOT}/meta.json") {
            meta = serde_json::from_str(&text).unwrap_or_default();
        } else if name == format!("{CONFIG_ROOT}/skins.json") {
            skins_json = Some(text);
        } else if name == format!("{CONFIG_ROOT}/web.json") {
            web = serde_json::from_str::<serde_json::Value>(&text).ok().filter(|v| v.is_object());
        } else if name == format!("{CONFIG_ROOT}/game.json") {
            game = Some(text);
        } else if let Some(rest) = name.strip_prefix(&format!("{CONFIG_ROOT}/instances/")) {
            if let Some((id, "instance.json")) = rest.split_once('/') {
                if paths::valid_instance_id(id) {
                    instances.push((id.to_string(), text));
                }
            }
        }
    }
    if settings_json.is_none() && instances.is_empty() {
        bail!("這不是輝煌啟動器匯出的設定檔");
    }
    let source_os = meta.get("os").and_then(|v| v.as_str()).map(str::to_string);
    let source_root = meta.get("root").and_then(|v| v.as_str()).map(str::to_string);
    let source_platform = meta.get("platform").and_then(|v| v.as_str()).unwrap_or(if source_os.as_deref() == Some("android") { "android" } else { "desktop" }).to_string();
    let same_os = source_os.as_deref() == Some(std::env::consts::OS);
    let total_mb = crate::boost::system_info().memory_mb;
    let mut notes = Vec::new();
    let mut applied_settings = false;
    if let Some(json) = settings_json {
        let mut imported: Settings = serde_json::from_str(&json).context("設定內容損毀")?;
        let current = crate::config::load_settings().await;
        imported.selected_account = current.selected_account;
        imported.autostart = current.autostart;
        imported.hardware_tuned = current.hardware_tuned;
        let fitted = fit_memory(imported.memory_mb, total_mb);
        if fitted != imported.memory_mb {
            notes.push(format!("記憶體上限 {} MB 超過這台電腦能負荷的量，已改為 {fitted} MB", imported.memory_mb));
            imported.memory_mb = fitted;
        }
        if !same_os {
            let cleared: Vec<&str> = [("啟動包裝指令", &mut imported.wrapper), ("啟動前指令", &mut imported.pre_launch_cmd), ("結束後指令", &mut imported.post_exit_cmd)]
                .into_iter()
                .filter(|(_, v)| !v.trim().is_empty())
                .map(|(label, v)| {
                    v.clear();
                    label
                })
                .collect();
            if !cleared.is_empty() {
                notes.push(format!("設定來自其他作業系統，已清除不適用的{}", cleared.join("、")));
            }
        } else {
            let kept: Vec<&str> = [("啟動包裝指令", &imported.wrapper), ("啟動前指令", &imported.pre_launch_cmd), ("結束後指令", &imported.post_exit_cmd)]
                .into_iter()
                .filter(|(_, v)| !v.trim().is_empty())
                .map(|(label, _)| label)
                .collect();
            if !kept.is_empty() {
                notes.push(format!("設定裡有{}，會在開始遊戲時執行；如果不是你自己設定的，請到「設定」的「啟動流程」與「進階」清除", kept.join("、")));
            }
        }
        imported.background = relocate(&imported.background, source_root.as_deref());
        imported.music = relocate(&imported.music, source_root.as_deref());
        imported.custom_font = relocate(&imported.custom_font, source_root.as_deref());
        for list in [&mut imported.fonts, &mut imported.playlist] {
            for item in list.iter_mut() {
                item.path = relocate(&item.path, source_root.as_deref());
            }
            list.retain(|item| !item.path.is_empty());
        }
        if !imported.java_path.is_empty() && !Path::new(&imported.java_path).is_file() {
            imported.java_path = String::new();
        }
        crate::net::set_proxy(&imported.proxy)?;
        crate::config::save_settings(&imported).await?;
        applied_settings = true;
    }
    let existing: Vec<String> = crate::instance::list().await.into_iter().map(|i| i.id).collect();
    let (mut added, mut skipped) = (0, 0);
    for (id, json) in instances {
        if existing.contains(&id) {
            skipped += 1;
            continue;
        }
        let Ok(mut instance) = serde_json::from_str::<Instance>(&json) else {
            skipped += 1;
            continue;
        };
        instance.id = id;
        instance.name = crate::instance::unique_name(&instance.name).await;
        instance.image = relocate(&instance.image, source_root.as_deref());
        instance.memory_mb = if instance.memory_mb > 0 { fit_memory(instance.memory_mb, total_mb) } else { 0 };
        if !same_os && !instance.wrapper.trim().is_empty() {
            instance.wrapper.clear();
            notes.push(format!("「{}」的啟動包裝指令來自其他作業系統，已清除", instance.name));
        } else if !instance.wrapper.trim().is_empty() {
            notes.push(format!("「{}」設有啟動包裝指令，會在開始遊戲時執行；如果不是你自己設定的，請到實例設定清除", instance.name));
        }
        if !instance.java_path.is_empty() && !Path::new(&instance.java_path).is_file() {
            instance.java_path = String::new();
        }
        std::fs::create_dir_all(paths::game_dir(&instance.id))?;
        crate::instance::save(&instance).await?;
        added += 1;
    }
    if let Some(list) = skins_json.and_then(|t| serde_json::from_str::<Vec<crate::skins::LibrarySkin>>(&t).ok()) {
        let mut library = crate::skins::load_library();
        let before = library.len();
        for mut skin in list {
            let file = root.join("skins").join(&skin.file);
            if skin.file.contains(['/', '\\']) || !file.is_file() || library.iter().any(|s| s.file == skin.file) {
                continue;
            }
            skin.path = file.to_string_lossy().to_string();
            library.push(skin);
        }
        if library.len() > before {
            crate::skins::save_library(&library)?;
        }
    }
    Ok(ImportOutcome { added, skipped, settings: applied_settings, notes, web, game, source_platform })
}

pub fn parse_cli(args: &[String]) -> Option<(String, Option<String>)> {
    let mut target = None;
    let mut join = None;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--launch" => target = iter.next().cloned(),
            "--join" => join = iter.next().cloned(),
            _ => {}
        }
    }
    let join = join.map(|j| match j.as_str() {
        "s1" | "s2" | "s3" | "s4" | "s0" => format!("{j}.brilliantw.net"),
        _ => j,
    });
    let join = join.filter(|j| j.len() <= 260 && j.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == ':'));
    target.filter(|t| !t.trim().is_empty()).map(|t| (t, join))
}

#[derive(serde::Serialize, Clone)]
struct CliLaunch {
    name: String,
    error: Option<String>,
}

pub fn handle_cli(app: &AppHandle, args: &[String]) {
    let Some((target, join)) = parse_cli(args) else {
        return;
    };
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let instances = crate::instance::list().await;
        let Some(instance) = instances.iter().find(|i| i.id == target).or_else(|| instances.iter().find(|i| i.name == target)).cloned() else {
            crate::boost::notify(&app, "找不到實例", &format!("捷徑指定的實例「{target}」不存在，可能已被刪除或改名。"));
            return;
        };
        let result = crate::commands::launch_game(app.clone(), instance.id.clone(), join).await;
        let error = result.err().map(|e| crate::err::message(&e.0));
        if let Some(message) = &error {
            crate::boost::notify(&app, "無法啟動遊戲", message);
        }
        let _ = app.emit("launcher://cli-launch", CliLaunch { name: instance.name, error });
    });
}

fn launch_command() -> R<(String, Vec<String>)> {
    if let Ok(id) = std::env::var("FLATPAK_ID") {
        return Ok(("flatpak".into(), vec!["run".into(), id]));
    }
    if let Ok(image) = std::env::var("APPIMAGE") {
        return Ok((image, Vec::new()));
    }
    let exe = std::env::current_exe().context("找不到啟動器執行檔")?;
    Ok((exe.to_string_lossy().to_string(), Vec::new()))
}

pub fn ps_quote(text: &str) -> String {
    text.chars().flat_map(|c| if "'\u{2018}\u{2019}\u{201A}\u{201B}".contains(c) { vec![c, c] } else { vec![c] }).collect()
}

pub fn create_shortcut(instance: &Instance, join: Option<String>) -> R<PathBuf> {
    if cfg!(target_os = "macos") {
        bail!("macOS 版暫不支援建立桌面捷徑");
    }
    let (program, mut base) = launch_command()?;
    base.push("--launch".into());
    base.push(instance.id.clone());
    if let Some(join) = join.filter(|j| !j.is_empty()) {
        base.push("--join".into());
        base.push(join);
    }
    let title = safe_name(&format!("輝煌 {}", instance.name));
    let desktop = dirs::desktop_dir().or_else(dirs::home_dir).context("找不到桌面資料夾")?;
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        let path = desktop.join(format!("{title}.lnk"));
        let quote = ps_quote;
        let script = format!(
            "$s=(New-Object -ComObject WScript.Shell).CreateShortcut('{}');$s.TargetPath='{}';$s.Arguments='{}';$s.IconLocation='{},0';$s.Description='{}';$s.Save()",
            quote(&path.to_string_lossy()),
            quote(&program),
            quote(&base.join(" ")),
            quote(&program),
            quote(&format!("用輝煌啟動器直接開啟「{}」", instance.name))
        );
        let status = std::process::Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", &script])
            .creation_flags(0x0800_0000)
            .status()
            .context("無法建立捷徑")?;
        if !status.success() || !path.is_file() {
            bail!("建立捷徑失敗");
        }
        Ok(path)
    }
    #[cfg(not(target_os = "windows"))]
    {
        let quote = |s: &str| format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""));
        let exec = std::iter::once(quote(&program)).chain(base.iter().map(|a| quote(a))).collect::<Vec<_>>().join(" ");
        let path = desktop.join(format!("{title}.desktop"));
        let body = format!("[Desktop Entry]\nType=Application\nName={title}\nComment=用輝煌啟動器直接開啟「{}」\nExec={exec}\nIcon=net.brilliantw.launcher\nTerminal=false\nCategories=Game;\n", instance.name);
        std::fs::write(&path, body)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755));
        }
        Ok(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_paths_move_between_devices() {
        assert_eq!(rel_from(r"C:\Users\米\AppData\Roaming\BrilliantLauncher\media\bg.png", r"C:\Users\米\AppData\Roaming\BrilliantLauncher").as_deref(), Some("media/bg.png"));
        assert_eq!(rel_from("/storage/emulated/0/Brilliant/media/images/a.png", "/storage/emulated/0/Brilliant/").as_deref(), Some("media/images/a.png"));
        assert_eq!(rel_from(r"c:\users\a\brilliantlauncher\skins\x.png", r"C:\Users\A\BrilliantLauncher").as_deref(), Some("skins/x.png"));
        assert!(rel_from(r"C:\Users\米\media\bg.png", r"C:\Users\米米").is_none());
        assert!(rel_from(r"C:\Root\..\secret.txt", r"C:\Root").is_none());
        let root = Path::new("/data/root");
        assert_eq!(portable_rel(root, "/data/root/media/bg.png").as_deref(), Some("media/bg.png"));
        assert!(portable_rel(root, "/data/root/accounts.json").is_none());
        assert!(portable_rel(root, "/elsewhere/media/bg.png").is_none());
    }

    #[test]
    fn powershell_quotes_cannot_be_escaped() {
        assert_eq!(ps_quote("a'b"), "a''b");
        assert_eq!(ps_quote("x\u{2019};calc;\u{2018}"), "x\u{2019}\u{2019};calc;\u{2018}\u{2018}");
        assert_eq!(ps_quote("輝煌 26.3"), "輝煌 26.3");
    }

    #[test]
    fn env_lines_are_validated() {
        let vars = parse_env("# 註解\nGAME_MODE=1\n  MANGOHUD = 1 \nbad key=1\n1BAD=2\nNO_EQUALS\nPATH_EXTRA=a=b");
        assert_eq!(vars, vec![("GAME_MODE".into(), "1".into()), ("MANGOHUD".into(), "1".into()), ("PATH_EXTRA".into(), "a=b".into())]);
    }

    #[test]
    fn wrapper_split_keeps_quotes() {
        assert_eq!(split_args(r#"gamemoderun  mangohud"#), vec!["gamemoderun", "mangohud"]);
        assert_eq!(split_args(r#""C:\Program Files\x.exe" --flag "a b""#), vec![r"C:\Program Files\x.exe", "--flag", "a b"]);
        assert!(split_args("   ").is_empty());
        let zgc = || split_args("-XX:+UseZGC -XX:+ZGenerational -Xss4m");
        assert_eq!(drop_removed_flags(zgc(), 21), zgc());
        assert_eq!(drop_removed_flags(zgc(), 25), vec!["-XX:+UseZGC", "-Xss4m"]);
    }

    #[test]
    fn tokens_are_redacted() {
        let line = "$ java -Xmx4G --username Rice --accessToken eyJhbGciOiJIUzI1NiJ9.verylongpayloadhere.signature --uuid abc";
        let out = redact(line);
        assert!(!out.contains("eyJhbGci"));
        assert!(out.contains("--accessToken <已隱藏>"));
        assert!(out.contains("--uuid abc"));
        assert_eq!(redact("token:abc:def"), "<已隱藏>");
        assert_eq!(redact("[main/INFO]: ok\n--accessToken secret\nnext line"), "[main/INFO]: ok\n--accessToken <已隱藏>\nnext line");
    }

    #[test]
    fn cli_parses_launch_and_join() {
        let args: Vec<String> = ["app.exe", "--launch", "main-1", "--join", "s1"].iter().map(|s| s.to_string()).collect();
        assert_eq!(parse_cli(&args), Some(("main-1".into(), Some("s1.brilliantw.net".into()))));
        let bad: Vec<String> = ["app.exe", "--launch", "x", "--join", "evil host;rm"].iter().map(|s| s.to_string()).collect();
        assert_eq!(parse_cli(&bad), Some(("x".into(), None)));
        assert_eq!(parse_cli(&["app.exe".to_string()]), None);
    }

    #[test]
    fn proxy_props_and_validation() {
        assert!(crate::net::valid_proxy("http://127.0.0.1:7890"));
        assert!(!crate::net::valid_proxy("socks5://127.0.0.1:1080"));
        assert!(!crate::net::valid_proxy("http://host"));
        assert_eq!(proxy_props("http://proxy.school.tw:3128"), vec!["-Dhttp.proxyHost=proxy.school.tw", "-Dhttp.proxyPort=3128", "-Dhttps.proxyHost=proxy.school.tw", "-Dhttps.proxyPort=3128"]);
    }

    #[test]
    fn mod_channels() {
        use crate::modrinth::channel_allows;
        assert!(channel_allows("release", "release") && !channel_allows("release", "beta"));
        assert!(channel_allows("beta", "beta") && !channel_allows("beta", "alpha"));
        assert!(channel_allows("alpha", "alpha") && channel_allows("alpha", "release"));
    }

    #[test]
    fn components_map_known_majors() {
        assert_eq!(java_component(21), "java-runtime-delta");
        assert_eq!(java_component(25), "java-25");
    }

    #[test]
    fn fits_memory_to_machine() {
        assert_eq!(fit_memory(8192, 16384), 8192);
        assert_eq!(fit_memory(12288, 16384), 4096);
        assert_eq!(fit_memory(6144, 0), 6144);
        assert_eq!(crate::mc::java::emulated_platform("mac-os-arm64"), Some("mac-os"));
        assert_eq!(crate::mc::java::emulated_platform("windows-arm64"), Some("windows-x64"));
        assert_eq!(crate::mc::java::emulated_platform("linux"), None);
    }

    #[test]
    fn shared_folder_round_trip() {
        let root = std::env::temp_dir().join(format!("brilliant-share-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let instance_saves = root.join("inst").join("saves");
        let shared = root.join("shared").join("saves");
        std::fs::create_dir_all(instance_saves.join("World A")).unwrap();
        std::fs::write(instance_saves.join("World A").join("level.dat"), b"a").unwrap();
        std::fs::create_dir_all(shared.join("World A")).unwrap();
        move_into(&instance_saves, &shared, "測試").unwrap();
        assert!(shared.join("World A (測試)").join("level.dat").is_file());
        make_link(&instance_saves, &shared).unwrap();
        assert!(is_link(&instance_saves));
        assert!(instance_saves.join("World A (測試)").join("level.dat").is_file());
        assert!(same_dir(&std::fs::read_link(&instance_saves).unwrap(), &shared));
        remove_link(&instance_saves).unwrap();
        assert!(shared.join("World A (測試)").join("level.dat").is_file());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    #[ignore = "改動 BRILLIANT_LAUNCHER_HOME，需單獨執行：cargo test --lib group_share_moves_existing_files -- --ignored"]
    fn group_share_moves_existing_files() {
        let root = std::env::temp_dir().join(format!("brilliant-group-share-{} A&B", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::env::set_var("BRILLIANT_LAUNCHER_HOME", &root);
        let packs = paths::game_dir("a").join("resourcepacks");
        std::fs::create_dir_all(&packs).unwrap();
        std::fs::write(packs.join("Faithful.zip"), b"pack").unwrap();
        let other = paths::game_dir("b").join("resourcepacks");
        std::fs::create_dir_all(&other).unwrap();
        std::fs::write(other.join("Faithful.zip"), b"other").unwrap();
        let mut settings = Settings::default();
        settings.instance_groups = vec![crate::config::InstanceGroup { name: "建築".into(), key: "k1".into(), shared: vec!["resourcepacks".into()], ..Default::default() }];
        let a = Instance { id: "a".into(), name: "A".into(), group: "建築".into(), ..Default::default() };
        let b = Instance { id: "b".into(), name: "B".into(), group: "建築".into(), ..Default::default() };
        assert_eq!(share_status(&a, &settings).pending, vec!["resourcepacks".to_string()]);
        sync_shared(&a, &settings).unwrap();
        sync_shared(&b, &settings).unwrap();
        let shared = group_base("k1").join("resourcepacks");
        assert_eq!(std::fs::read(shared.join("Faithful.zip")).unwrap(), b"pack");
        assert_eq!(std::fs::read(shared.join("Faithful (B).zip")).unwrap(), b"other");
        assert!(is_link(&packs) && is_link(&other));
        assert_eq!(std::fs::read(packs.join("Faithful (B).zip")).unwrap(), b"other");
        assert_eq!(share_status(&a, &settings).linked, vec!["resourcepacks".to_string()]);
        let left = Instance { group: String::new(), ..a.clone() };
        sync_shared(&left, &settings).unwrap();
        assert!(!is_link(&packs));
        assert!(packs.join("Faithful.zip").is_file() && shared.join("Faithful.zip").is_file());
        std::env::remove_var("BRILLIANT_LAUNCHER_HOME");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn sync_folder_shares_like_a_link() {
        let root = std::env::temp_dir().join(format!("brilliant-sync-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let (shared, a, b) = (root.join("shared"), root.join("a"), root.join("b"));
        let (sa, sb) = (root.join("a.json"), root.join("b.json"));
        for d in [&shared, &a, &b] {
            std::fs::create_dir_all(d).unwrap();
        }
        std::fs::write(a.join("pack.zip"), "A").unwrap();
        std::fs::create_dir_all(a.join("World/region")).unwrap();
        std::fs::write(a.join("World/region/r.0.0.mca"), "world").unwrap();
        sync_folder(&a, &shared, &sa, "a").unwrap();
        sync_folder(&b, &shared, &sb, "b").unwrap();
        assert_eq!(std::fs::read_to_string(b.join("pack.zip")).unwrap(), "A");
        assert!(b.join("World/region/r.0.0.mca").is_file());

        std::fs::remove_dir_all(b.join("World")).unwrap();
        std::fs::write(b.join("new.zip"), "B").unwrap();
        sync_folder(&b, &shared, &sb, "b").unwrap();
        sync_folder(&a, &shared, &sa, "a").unwrap();
        assert!(!a.join("World").exists(), "deleted world should disappear everywhere");
        assert!(!shared.join("World").exists());
        assert_eq!(std::fs::read_to_string(a.join("new.zip")).unwrap(), "B");

        std::fs::write(a.join("pending.txt"), "unsynced").unwrap();
        sync_folder(&a, &shared, &sa, "a").unwrap();
        assert!(shared.join("pending.txt").is_file(), "changes left from a crash are pushed before pulling");

        let c = root.join("c");
        std::fs::create_dir_all(&c).unwrap();
        std::fs::write(c.join("pack.zip"), "CC").unwrap();
        sync_folder(&c, &shared, &root.join("c.json"), "c").unwrap();
        assert_eq!(std::fs::read_to_string(shared.join("pack.zip")).unwrap(), "A");
        assert_eq!(std::fs::read_to_string(shared.join("pack (c).zip")).unwrap(), "CC", "first join keeps both versions like move_into");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn group_bases_are_separate() {
        assert!(group_base("legacy").ends_with("folders"));
        assert_ne!(group_base("a1"), group_base("b2"));
        let mut s = Settings::default();
        s.instance_groups = vec![crate::config::InstanceGroup { name: "建築".into(), shared: vec!["saves".into(), "../x".into()], ..Default::default() }];
        s.clean();
        assert_eq!(s.instance_groups[0].shared, vec!["saves".to_string()]);
        assert_eq!(s.instance_groups[0].key.len(), 16);
    }
}
