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

pub async fn info() -> R<LauncherInfo> {
    net::get_json(&format!("{SITE}/api/launcher/info"), false).await
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

async fn brilliant_mod(mc: &str) -> R<PathBuf> {
    let latest = match net::get_json::<ModLatest>(&format!("{SITE}/api/mod/latest?mc={mc}"), false).await {
        Ok(latest) => latest,
        Err(e) => return cached_for(mc).ok_or(e),
    };
    if !safe_file_name(&latest.file, MOD_PREFIX, ".jar") || !latest.url.starts_with(&format!("{SITE}/api/mod/download/")) {
        bail!("官網回傳的 Brilliant-Server-Mod 資訊不正確");
    }
    let path = cache_dir().join(&latest.file);
    if !path.is_file() {
        let bytes = net::get_bytes(&latest.url, false).await?;
        if let Some(want) = &latest.sha256 {
            let got = sha256_hex(&bytes);
            if !got.eq_ignore_ascii_case(want) {
                bail!("Brilliant-Server-Mod SHA-256 不符：預期 {want}，實得 {got}");
            }
        }
        net::write_file(&path, &bytes, false).await?;
    }
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

pub fn forget_sync() {
    if let Ok(mut synced) = SYNCED.lock() {
        synced.clear();
    }
}

fn has_both(mods_dir: &Path) -> bool {
    let names: Vec<String> = std::fs::read_dir(mods_dir)
        .map(|d| d.flatten().map(|e| e.file_name().to_string_lossy().to_string()).collect())
        .unwrap_or_default();
    names.iter().any(|n| n.starts_with(MOD_PREFIX)) && names.iter().any(|n| n.starts_with(API_PREFIX))
}

pub async fn inject(app: &AppHandle, instance: &Instance) -> R<Vec<String>> {
    let mods_dir = paths::game_dir(&instance.id).join("mods");
    tokio::fs::create_dir_all(&mods_dir).await?;
    let mc = instance.mc_version.as_str();
    let mut notes = Vec::new();
    let key = instance.id.clone();
    let fresh = SYNCED.lock().map(|s| s.iter().any(|(k, at)| *k == key && at.elapsed() < SYNC_TTL)).unwrap_or(false);
    if fresh && has_both(&mods_dir) {
        return Ok(notes);
    }

    let jar = match brilliant_mod(mc).await {
        Ok(jar) => jar,
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
    if size(&dest) != size(&jar) {
        tokio::fs::copy(&jar, &dest)
            .await
            .with_context(|| format!("複製 Brilliant-Server-Mod 失敗：{}", jar.display()))?;
    }

    match fabric_api(mc).await {
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
        if let Ok(mut synced) = SYNCED.lock() {
            synced.retain(|(k, _)| *k != key);
            synced.push((key, std::time::Instant::now()));
        }
    }
    Ok(notes)
}

#[cfg(test)]
mod tests {
    use super::*;

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
