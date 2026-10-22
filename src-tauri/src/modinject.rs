use crate::bail;
use crate::err::R;
use crate::instance::Instance;
use crate::net::{self, DlFile};
use crate::paths;
use anyhow::Context;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use tauri::AppHandle;

pub const MOD_PREFIX: &str = "brilliant-server-mod-";
const LEGACY_PREFIX: &str = "brilliant-server-discord-rich-presence-";
const API_PREFIX: &str = "fabric-api-";
const SITE: &str = "https://www.brilliantw.net";

#[derive(Deserialize, Serialize, Clone, Debug, Default)]
#[serde(default)]
pub struct LauncherInfo {
    pub versions: Vec<String>,
    pub recommended: String,
    pub launcher: Option<serde_json::Value>,
}

#[derive(Deserialize)]
struct ModLatest {
    url: String,
    file: String,
    sha256: Option<String>,
    #[serde(default)]
    signature: Option<String>,
}

#[derive(Deserialize)]
struct MrVersion {
    version_type: String,
    files: Vec<MrFile>,
}

#[derive(Deserialize)]
struct MrFile {
    url: String,
    filename: String,
    primary: bool,
    size: u64,
    hashes: MrHashes,
}

#[derive(Deserialize)]
struct MrHashes {
    sha1: String,
}

pub fn is_managed(file: &str) -> bool {
    file.starts_with(MOD_PREFIX) || file.starts_with(LEGACY_PREFIX) || file.starts_with(API_PREFIX)
}

async fn install_id() -> Option<String> {
    if !crate::config::load_settings().await.usage_stats {
        return None;
    }
    let file = paths::root().join("install-id");
    let saved = tokio::fs::read_to_string(&file).await.unwrap_or_default();
    let saved = saved.trim();
    if saved.len() == 32 && saved.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()) {
        return Some(saved.to_string());
    }
    let id = format!("{:032x}", rand::random::<u128>());
    tokio::fs::write(&file, &id).await.ok()?;
    Some(id)
}

pub async fn info() -> R<LauncherInfo> {
    let channel = crate::config::channel_of(&crate::config::load_settings().await);
    let id = install_id().await.map(|id| format!("&id={id}")).unwrap_or_default();
    net::get_json(&format!("{SITE}/api/launcher/info?channel={channel}{id}"), false).await
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

pub fn safe_file_name(name: &str, prefix: &str, suffix: &str) -> bool {
    name.starts_with(prefix)
        && name.ends_with(suffix)
        && !name.contains(['/', '\\'])
        && !name.contains("..")
}

fn cache_dir() -> PathBuf {
    paths::shared().join("brilliant-mods")
}

async fn prune(dir: &Path, prefixes: &[&str], keep: &str) -> R<()> {
    let Ok(mut entries) = tokio::fs::read_dir(dir).await else {
        return Ok(());
    };
    while let Some(entry) = entries.next_entry().await? {
        let name = entry.file_name().to_string_lossy().to_string();
        if prefixes.iter().any(|p| name.starts_with(p)) && name != keep {
            let _ = tokio::fs::remove_file(entry.path()).await;
        }
    }
    Ok(())
}

fn stale_cache(names: &[String], mc: &str, keep: &str) -> Vec<String> {
    let suffix = format!("+{mc}.jar");
    names.iter().filter(|n| n.starts_with(MOD_PREFIX) && n.ends_with(&suffix) && n.as_str() != keep).cloned().collect()
}

async fn prune_cache(mc: &str, keep: &str) {
    let names: Vec<String> = std::fs::read_dir(cache_dir())
        .map(|d| d.flatten().map(|e| e.file_name().to_string_lossy().to_string()).collect())
        .unwrap_or_default();
    for name in stale_cache(&names, mc, keep) {
        let _ = tokio::fs::remove_file(cache_dir().join(name)).await;
    }
}

fn cached_for(mc: &str) -> Option<PathBuf> {
    let suffix = format!("+{mc}.jar");
    std::fs::read_dir(cache_dir())
        .ok()?
        .flatten()
        .filter(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            name.starts_with(MOD_PREFIX) && name.ends_with(&suffix)
        })
        .max_by_key(|e| e.metadata().and_then(|m| m.modified()).ok())
        .map(|e| e.path())
}

pub(crate) async fn brilliant_mod(mc: &str, keep: bool) -> R<PathBuf> {
    if keep && !net::DEEP_VERIFY.load(std::sync::atomic::Ordering::Relaxed) {
        if let Some(cached) = cached_for(mc) {
            return Ok(cached);
        }
    }
    let latest = match net::get_json::<ModLatest>(&format!("{SITE}/api/mod/latest?mc={mc}"), false).await {
        Ok(latest) => latest,
        Err(e) => return cached_for(mc).ok_or(e),
    };
    if !safe_file_name(&latest.file, MOD_PREFIX, ".jar") || !latest.url.starts_with(&format!("{SITE}/api/mod/download/")) {
        bail!("官網回傳的 Brilliant-Server-Mod 資訊不正確");
    }
    let signature = latest.signature.clone().unwrap_or_default();
    let path = cache_dir().join(&latest.file);
    let trusted = |bytes: &[u8]| crate::trust::verify(&latest.file, bytes, &signature);
    if path.is_file() && !tokio::fs::read(&path).await.map(|b| trusted(&b)).unwrap_or(false) {
        let _ = tokio::fs::remove_file(&path).await;
    }
    if !path.is_file() {
        let bytes = net::get_bytes(&latest.url, false).await?;
        if let Some(want) = &latest.sha256 {
            let got = sha256_hex(&bytes);
            if !got.eq_ignore_ascii_case(want) {
                bail!("Brilliant-Server-Mod SHA-256 不符：預期 {want}，實得 {got}");
            }
        }
        if !trusted(&bytes) {
            if let Some(cached) = cached_for(mc) {
                return Ok(cached);
            }
            bail!("Brilliant-Server-Mod 沒有通過輝煌官方數位簽章驗證，為了安全已拒絕安裝");
        }
        net::write_file(&path, &bytes, false).await?;
    }
    prune_cache(mc, &latest.file).await;
    Ok(path)
}

async fn fabric_api(mc: &str) -> R<MrFile> {
    let url = format!(
        "https://api.modrinth.com/v2/project/fabric-api/version?game_versions=%5B%22{mc}%22%5D&loaders=%5B%22fabric%22%5D"
    );
    let versions: Vec<MrVersion> = net::get_json(&url, false).await?;
    let Some(index) = versions.iter().position(|v| v.version_type == "release").or(if versions.is_empty() { None } else { Some(0) }) else {
        bail!("Modrinth 上找不到 Minecraft {mc} 的 Fabric API");
    };
    let mut files = versions.into_iter().nth(index).map(|v| v.files).unwrap_or_default();
    if files.is_empty() {
        bail!("Fabric API 沒有可下載的檔案");
    }
    let primary = files.iter().position(|f| f.primary).unwrap_or(0);
    let file = files.swap_remove(primary);
    if !safe_file_name(&file.filename, API_PREFIX, ".jar") {
        bail!("Fabric API 檔名不正確：{}", file.filename);
    }
    Ok(file)
}

static SYNCED: std::sync::Mutex<Vec<(String, std::time::Instant)>> = std::sync::Mutex::new(Vec::new());
const SYNC_TTL: std::time::Duration = std::time::Duration::from_secs(15 * 60);

pub fn synced_recently(key: &str) -> bool {
    SYNCED.lock().map(|s| s.iter().any(|(k, at)| k == key && at.elapsed() < SYNC_TTL)).unwrap_or(false)
}

pub fn mark_synced(key: String) {
    if let Ok(mut synced) = SYNCED.lock() {
        synced.retain(|(k, _)| *k != key);
        synced.push((key, std::time::Instant::now()));
    }
}

pub fn forget_sync() {
    if let Ok(mut synced) = SYNCED.lock() {
        synced.clear();
    }
}

fn exactly_one_each(names: &[String]) -> bool {
    names.iter().filter(|n| n.starts_with(MOD_PREFIX) || n.starts_with(LEGACY_PREFIX)).count() == 1
        && names.iter().filter(|n| n.starts_with(API_PREFIX)).count() == 1
}

fn has_both(mods_dir: &Path) -> bool {
    let names: Vec<String> = std::fs::read_dir(mods_dir)
        .map(|d| d.flatten().map(|e| e.file_name().to_string_lossy().to_string()).collect())
        .unwrap_or_default();
    exactly_one_each(&names)
}

pub async fn inject(app: &AppHandle, instance: &Instance) -> R<Vec<String>> {
    let mods_dir = paths::game_dir(&instance.id).join("mods");
    tokio::fs::create_dir_all(&mods_dir).await?;
    let mc = instance.mc_version.as_str();
    let mut notes = Vec::new();
    let key = instance.id.clone();
    if synced_recently(&key) && has_both(&mods_dir) {
        return Ok(notes);
    }

    let keep = crate::config::load_settings().await.mod_update == "keep";
    let (jar, api) = tokio::join!(brilliant_mod(mc, keep), fabric_api(mc));
    let jar = match jar {
        Ok(jar) => jar,
        Err(e) if net::is_offline(&e.0) => {
            notes.push("目前連不上官網，沿用已安裝的 Brilliant-Server-Mod，網路恢復後會自動更新。".into());
            return Ok(notes);
        }
        Err(e) => {
            prune(&mods_dir, &[MOD_PREFIX, LEGACY_PREFIX], "").await?;
            notes.push(format!("Brilliant-Server-Mod 目前不支援 Minecraft {mc}，或暫時連不上官網（{:#}），本次啟動不安裝。", e.0));
            return Ok(notes);
        }
    };
    let name = jar.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    prune(&mods_dir, &[MOD_PREFIX, LEGACY_PREFIX], &name).await?;
    let dest = mods_dir.join(&name);
    let size = |p: &Path| std::fs::metadata(p).map(|m| m.len()).ok();
    if net::DEEP_VERIFY.load(std::sync::atomic::Ordering::Relaxed) || size(&dest) != size(&jar) {
        let bytes = tokio::fs::read(&jar).await.with_context(|| format!("讀取 Brilliant-Server-Mod 失敗：{}", jar.display()))?;
        net::write_file(&dest, &bytes, false).await?;
    }

    match api {
        Ok(file) => {
            prune(&mods_dir, &[API_PREFIX], &file.filename).await?;
            let mut dl = DlFile::new(file.url, mods_dir.join(&file.filename)).sha1(Some(file.hashes.sha1));
            dl.size = file.size;
            net::download_all(app, "mods", vec![dl], false).await?;
        }
        Err(e) => {
            let has_api = std::fs::read_dir(&mods_dir)
                .map(|d| d.flatten().any(|e| e.file_name().to_string_lossy().starts_with(API_PREFIX)))
                .unwrap_or(false);
            if !has_api {
                notes.push(format!("無法下載 Fabric API（{:#}），Brilliant-Server-Mod 可能無法載入。", e.0));
            }
        }
    }
    if notes.is_empty() {
        mark_synced(key);
    }
    Ok(notes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stale_versions_are_detected() {
        let names: Vec<String> = ["brilliant-server-mod-1.1.46+26.3.jar", "brilliant-server-mod-1.1.47+26.3.jar", "brilliant-server-mod-1.1.40+26.2.jar", "fabric-api-0.164.0+26.3.jar"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        assert_eq!(stale_cache(&names, "26.3", "brilliant-server-mod-1.1.47+26.3.jar"), vec!["brilliant-server-mod-1.1.46+26.3.jar".to_string()]);
        assert!(stale_cache(&names, "26.2", "brilliant-server-mod-1.1.40+26.2.jar").is_empty());
        assert!(!exactly_one_each(&names));
        let clean: Vec<String> = ["brilliant-server-mod-1.1.47+26.3.jar", "fabric-api-0.164.0+26.3.jar", "sodium.jar"].iter().map(|s| s.to_string()).collect();
        assert!(exactly_one_each(&clean));
        let old_and_new: Vec<String> = ["brilliant-server-mod-1.1.47+26.3.jar", "brilliant-server-discord-rich-presence-1.0.jar", "fabric-api-0.164.0+26.3.jar"].iter().map(|s| s.to_string()).collect();
        assert!(!exactly_one_each(&old_and_new));
    }

    #[test]
    fn file_names_are_checked() {
        assert!(safe_file_name("brilliant-server-mod-1.1.40+26.3.jar", MOD_PREFIX, ".jar"));
        assert!(!safe_file_name("brilliant-server-mod-../x.jar", MOD_PREFIX, ".jar"));
        assert!(!safe_file_name("brilliant-server-mod-a\\b.jar", MOD_PREFIX, ".jar"));
        assert!(!safe_file_name("other.jar", MOD_PREFIX, ".jar"));
        assert!(is_managed("brilliant-server-discord-rich-presence-1.0+26.2.jar"));
        assert!(is_managed("fabric-api-0.162.0+26.3.jar"));
        assert!(!is_managed("sodium-fabric-0.6.jar"));
    }

    #[test]
    fn sha256_matches_known_vector() {
        assert_eq!(sha256_hex(b"abc"), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
    }
}
