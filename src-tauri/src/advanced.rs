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
    match major {
        8 => "jre-legacy".into(),
        16 => "java-runtime-alpha".into(),
        17 => "java-runtime-gamma".into(),
        21 => "java-runtime-delta".into(),
        other => format!("java-{other}"),
    }
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

fn is_link(path: &Path) -> bool {
    std::fs::symlink_metadata(path).map(|m| m.file_type().is_symlink()).unwrap_or(false)
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
        let status = std::process::Command::new("cmd")
            .arg("/C")
            .arg("mklink")
            .arg("/J")
            .arg(link.as_os_str())
            .arg(target.as_os_str())
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
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)?.flatten() {
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if from.is_dir() {
            copy_dir(&from, &to)?;
        } else {
            std::fs::copy(&from, &to).with_context(|| format!("複製 {} 失敗", from.display()))?;
        }
    }
    Ok(())
}

fn move_into(src: &Path, dst: &Path, tag: &str) -> R<()> {
    std::fs::create_dir_all(dst)?;
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
            if entry.path().is_dir() {
                copy_dir(&entry.path(), &target)?;
                std::fs::remove_dir_all(entry.path())?;
            } else {
                std::fs::copy(entry.path(), &target)?;
                std::fs::remove_file(entry.path())?;
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
    for name in SHAREABLE {
        let link = game.join(name);
        let want = group.filter(|g| g.shared.iter().any(|f| f == name)).map(|g| group_base(&g.key).join(name));
        let current = if is_link(&link) { std::fs::read_link(&link).ok() } else { None };
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

#[tauri::command]
pub async fn shared_items(key: String, folder: String) -> R<Vec<SharedItem>> {
    let dir = shared_dir(&key, &folder)?;
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
                    let title = std::fs::read(&level).ok().and_then(|b| crate::worlds::read_level(&b).0).unwrap_or_else(|| name.clone());
                    let icon = path.join("icon.png");
                    (level.is_file(), title, icon.is_file().then(|| icon.to_string_lossy().to_string()))
                }
                "screenshots" => (lower.ends_with(".png") || lower.ends_with(".jpg"), name.clone(), None),
                _ => {
                    let pack = meta.is_dir() || lower.ends_with(".zip");
                    let icon = if !meta.is_dir() && meta.len() < 64 << 20 {
                        std::fs::read(&path).ok().and_then(|b| crate::modrinth::embedded_icon(&b, false))
                    } else if meta.is_dir() && path.join("pack.png").is_file() {
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
    let mut added = 0;
    for file in files {
        let src = PathBuf::from(&file);
        let name = src.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        if !safe_item(&name) || !name.to_ascii_lowercase().ends_with(".zip") || !src.is_file() {
            continue;
        }
        std::fs::copy(&src, dir.join(&name)).with_context(|| format!("複製 {name} 失敗"))?;
        added += 1;
    }
    Ok(added)
}

#[tauri::command]
pub async fn shared_remove(key: String, folder: String, names: Vec<String>) -> R<usize> {
    let dir = shared_dir(&key, &folder)?;
    let trash = group_base(&key).join("trash").join(format!("{folder}-{}", crate::config::now_secs()));
    let mut moved = 0;
    for name in names.iter().filter(|n| safe_item(n)) {
        let src = dir.join(name);
        if !src.exists() {
            continue;
        }
        std::fs::create_dir_all(&trash)?;
        std::fs::rename(&src, trash.join(name)).with_context(|| format!("移除 {name} 失敗"))?;
        moved += 1;
    }
    Ok(moved)
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

pub fn sync_shared(instance: &Instance, settings: &Settings) -> R<Vec<String>> {
    let game = paths::game_dir(&instance.id);
    std::fs::create_dir_all(&game)?;
    let group = settings.instance_groups.iter().find(|g| !instance.group.is_empty() && g.name == instance.group && !g.key.is_empty());
    let mut notes = Vec::new();
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
                std::fs::create_dir_all(&link)?;
            }
            if want.is_none() {
                notes.push(format!("「{name}」已改回這個實例自己的資料夾（複製了一份共用內容）"));
            }
        }
        if let Some(target) = &want {
            std::fs::create_dir_all(target)?;
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
    let clean: Vec<String> = text.lines().map(redact).collect();
    add_text(zip, name, &clean.join("\n"))
}

fn safe_name(text: &str) -> String {
    let cleaned: String = text.chars().map(|c| if "\\/:*?\"<>|".contains(c) || c.is_control() { '_' } else { c }).collect();
    cleaned.trim().chars().take(40).collect()
}

fn export_dir() -> PathBuf {
    dirs::download_dir().unwrap_or_else(|| paths::root().join("exports"))
}

pub fn export_diagnostics(instance: &Instance, settings: &Settings, java: &str) -> R<PathBuf> {
    let dir = export_dir();
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(format!("輝煌啟動器診斷-{}-{}.zip", safe_name(&instance.name), crate::config::now_secs()));
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
        if instance.use_shared { settings.shared_folders.join("、") } else { "未使用".into() },
        sorted.len(),
        sorted.join("\n")
    );
    add_text(&mut zip, "摘要.txt", &summary)?;
    if let Ok(json) = serde_json::to_string_pretty(instance) {
        add_text(&mut zip, "instance.json", &json)?;
    }
    if let Ok(json) = serde_json::to_string_pretty(settings) {
        add_text(&mut zip, "launcher-settings.json", &json)?;
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

pub async fn export_config() -> R<PathBuf> {
    let settings = crate::config::load_settings().await;
    let instances = crate::instance::list().await;
    let dir = export_dir();
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(format!("輝煌啟動器設定-{}.zip", crate::config::now_secs()));
    let mut zip = zip::ZipWriter::new(std::fs::File::create(&path)?);
    add_text(&mut zip, &format!("{CONFIG_ROOT}/meta.json"), &serde_json::json!({ "os": std::env::consts::OS, "version": env!("CARGO_PKG_VERSION") }).to_string())?;
    add_text(&mut zip, &format!("{CONFIG_ROOT}/config.json"), &serde_json::to_string_pretty(&settings)?)?;
    for instance in &instances {
        add_text(&mut zip, &format!("{CONFIG_ROOT}/instances/{}/instance.json", instance.id), &serde_json::to_string_pretty(instance)?)?;
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

pub async fn import_config(path: &str) -> R<(usize, usize, bool, Vec<String>)> {
    let file = std::fs::File::open(path).with_context(|| format!("無法開啟 {path}"))?;
    let mut archive = zip::ZipArchive::new(file).context("這不是輝煌啟動器匯出的設定檔")?;
    let mut settings_json: Option<String> = None;
    let mut source_os: Option<String> = None;
    let mut instances: Vec<(String, String)> = Vec::new();
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;
        if entry.size() > 4 << 20 {
            continue;
        }
        let name = entry.name().replace('\\', "/");
        let mut text = String::new();
        if entry.read_to_string(&mut text).is_err() {
            continue;
        }
        if name == format!("{CONFIG_ROOT}/config.json") {
            settings_json = Some(text);
        } else if name == format!("{CONFIG_ROOT}/meta.json") {
            source_os = serde_json::from_str::<serde_json::Value>(&text).ok().and_then(|v| v.get("os")?.as_str().map(str::to_string));
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
        if !imported.background.is_empty() && !Path::new(&imported.background).is_file() {
            imported.background = String::new();
        }
        if !imported.music.is_empty() && !Path::new(&imported.music).is_file() {
            imported.music = String::new();
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
        instance.image = String::new();
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
    Ok((added, skipped, applied_settings, notes))
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
        let error = result.err().map(|e| format!("{:#}", e.0));
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
        let quote = |s: &str| s.replace('\'', "''");
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
    fn env_lines_are_validated() {
        let vars = parse_env("# 註解\nGAME_MODE=1\n  MANGOHUD = 1 \nbad key=1\n1BAD=2\nNO_EQUALS\nPATH_EXTRA=a=b");
        assert_eq!(vars, vec![("GAME_MODE".into(), "1".into()), ("MANGOHUD".into(), "1".into()), ("PATH_EXTRA".into(), "a=b".into())]);
    }

    #[test]
    fn wrapper_split_keeps_quotes() {
        assert_eq!(split_args(r#"gamemoderun  mangohud"#), vec!["gamemoderun", "mangohud"]);
        assert_eq!(split_args(r#""C:\Program Files\x.exe" --flag "a b""#), vec![r"C:\Program Files\x.exe", "--flag", "a b"]);
        assert!(split_args("   ").is_empty());
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
        let root = std::env::temp_dir().join(format!("brilliant-group-share-{}", std::process::id()));
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
