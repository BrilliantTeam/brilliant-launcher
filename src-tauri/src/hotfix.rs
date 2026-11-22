use crate::bail;
use crate::err::R;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::collections::HashMap;
use std::io::Read;
use std::path::PathBuf;
use std::sync::RwLock;
use tauri::utils::assets::{AssetKey, AssetsIter, CspHash};
use tauri::{Assets, Runtime};

const VERSION: &str = env!("CARGO_PKG_VERSION");
const SITE: &str = "https://www.brilliantw.net";
const MAX_BYTES: usize = 32 * 1024 * 1024;

static ACTIVE: RwLock<Option<(u32, HashMap<String, Vec<u8>>)>> = RwLock::new(None);

#[derive(Serialize, Deserialize)]
struct Saved {
    app: String,
    build: u32,
    signature: String,
}

fn dir() -> PathBuf {
    crate::paths::root().join("ui-hotfix")
}

pub fn file_name(build: u32) -> String {
    format!("BrilliantLauncher_UI_{VERSION}_{build}.zip")
}

fn unpack(build: u32, bytes: &[u8], signature: &str) -> anyhow::Result<HashMap<String, Vec<u8>>> {
    if !crate::trust::verify(&file_name(build), bytes, signature) {
        anyhow::bail!("介面修正檔沒有通過輝煌官方數位簽章驗證");
    }
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes))?;
    let mut files = HashMap::new();
    let mut total = 0usize;
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i)?;
        if entry.is_dir() {
            continue;
        }
        let Some(name) = entry.enclosed_name() else { continue };
        let mut data = Vec::new();
        entry.read_to_end(&mut data)?;
        total += data.len();
        if total > MAX_BYTES {
            anyhow::bail!("介面修正檔過大");
        }
        files.insert(format!("/{}", name.to_string_lossy().replace('\\', "/")), data);
    }
    Ok(files)
}

pub fn load() {
    let dir = dir();
    let saved: Option<Saved> = std::fs::read(dir.join("hotfix.json")).ok().and_then(|b| serde_json::from_slice(&b).ok());
    let Some(saved) = saved else { return };
    if saved.app != VERSION {
        let _ = std::fs::remove_dir_all(&dir);
        return;
    }
    let pending = dir.join("pending");
    if pending.exists() {
        for file in ["hotfix.zip", "hotfix.json", "pending"] {
            let _ = std::fs::remove_file(dir.join(file));
        }
        let _ = std::fs::write(dir.join("rejected"), format!("{VERSION} {}", saved.build));
        return;
    }
    let Ok(bytes) = std::fs::read(dir.join("hotfix.zip")) else { return };
    match unpack(saved.build, &bytes, &saved.signature) {
        Ok(files) => {
            let _ = std::fs::write(&pending, "");
            *ACTIVE.write().unwrap() = Some((saved.build, files));
        }
        Err(_) => {
            let _ = std::fs::remove_dir_all(&dir);
        }
    }
}

fn rejected() -> u32 {
    std::fs::read_to_string(dir().join("rejected"))
        .ok()
        .and_then(|s| s.split_once(' ').filter(|(app, _)| *app == VERSION).and_then(|(_, b)| b.trim().parse().ok()))
        .unwrap_or(0)
}

fn current() -> u32 {
    ACTIVE.read().unwrap().as_ref().map(|(build, _)| *build).unwrap_or(0)
}

#[tauri::command]
pub fn hotfix_status() -> u32 {
    current()
}

#[tauri::command]
pub fn hotfix_ok() {
    let _ = std::fs::remove_file(dir().join("pending"));
}

#[tauri::command]
pub async fn apply_hotfix(url: String, build: u32, sha256: String, signature: String) -> R<u32> {
    if build <= current() || build <= rejected() {
        return Ok(current());
    }
    let file = file_name(build);
    if !url.starts_with(&format!("{SITE}/api/launcher/download/")) || !url.ends_with(&file) || sha256.len() != 64 {
        bail!("介面修正資訊不正確");
    }
    let bytes = crate::net::client().get(&url).send().await?.error_for_status()?.bytes().await?;
    if bytes.len() > MAX_BYTES || !crate::modinject::sha256_hex(&bytes).eq_ignore_ascii_case(&sha256) {
        bail!("介面修正檔下載不完整，已取消");
    }
    let files = unpack(build, &bytes, &signature)?;
    let dir = dir();
    tokio::fs::create_dir_all(&dir).await?;
    tokio::fs::write(dir.join("hotfix.zip"), &bytes).await?;
    tokio::fs::write(dir.join("hotfix.json"), serde_json::to_vec(&Saved { app: VERSION.into(), build, signature })?).await?;
    tokio::fs::write(dir.join("pending"), "").await?;
    *ACTIVE.write().unwrap() = Some((build, files));
    Ok(build)
}

struct Empty;

impl<R: Runtime> Assets<R> for Empty {
    fn get(&self, _: &AssetKey) -> Option<Cow<'_, [u8]>> {
        None
    }
    fn iter(&self) -> Box<AssetsIter<'_>> {
        Box::new(std::iter::empty())
    }
    fn csp_hashes(&self, _: &AssetKey) -> Box<dyn Iterator<Item = CspHash<'_>> + '_> {
        Box::new(std::iter::empty())
    }
}

struct Overlay<R: Runtime>(Box<dyn Assets<R>>);

impl<R: Runtime> Assets<R> for Overlay<R> {
    fn setup(&self, app: &tauri::App<R>) {
        self.0.setup(app)
    }
    fn get(&self, key: &AssetKey) -> Option<Cow<'_, [u8]>> {
        if let Some(data) = ACTIVE.read().unwrap().as_ref().and_then(|(_, files)| files.get(key.as_ref()).cloned()) {
            return Some(Cow::Owned(data));
        }
        self.0.get(key)
    }
    fn iter(&self) -> Box<AssetsIter<'_>> {
        self.0.iter()
    }
    fn csp_hashes(&self, key: &AssetKey) -> Box<dyn Iterator<Item = CspHash<'_>> + '_> {
        self.0.csp_hashes(key)
    }
}

pub fn wrap<R: Runtime>(context: &mut tauri::Context<R>) {
    load();
    let embedded = context.set_assets(Box::new(Empty));
    context.set_assets(Box::new(Overlay(embedded)));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_unsigned_or_renamed_bundles() {
        let mut buf = std::io::Cursor::new(Vec::new());
        {
            let mut zip = zip::ZipWriter::new(&mut buf);
            zip.start_file("js/main.js", zip::write::SimpleFileOptions::default()).unwrap();
            std::io::Write::write_all(&mut zip, b"console.log(1)").unwrap();
            zip.finish().unwrap();
        }
        let bytes = buf.into_inner();
        assert!(unpack(1, &bytes, "").is_err(), "沒有簽章要拒絕");
        assert!(unpack(1, &bytes, "7w50zp16kSKorY4aE6TR6E0MSnEG1MWHblvMoE1SKd4SdPuvVdYHrENDjCatgMxruSwFyAP9PgIc+m82xxznCg==").is_err(), "別的檔案的簽章要拒絕");
        assert_eq!(file_name(3), format!("BrilliantLauncher_UI_{VERSION}_3.zip"));
    }
}
