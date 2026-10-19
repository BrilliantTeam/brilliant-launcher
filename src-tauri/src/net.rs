use crate::err::R;
use crate::bail;
use anyhow::{anyhow, Context};
use futures::StreamExt;
use serde::{de::DeserializeOwned, Serialize};
use sha1::{Digest, Sha1};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, OnceLock};
use tauri::{AppHandle, Emitter};

const RETRIES: usize = 3;
static CONCURRENCY: AtomicUsize = AtomicUsize::new(12);

pub fn set_concurrency(n: u32) {
    CONCURRENCY.store(n.clamp(1, 64) as usize, Ordering::Relaxed);
}

pub fn client() -> &'static reqwest::Client {
    static C: OnceLock<reqwest::Client> = OnceLock::new();
    C.get_or_init(|| {
        reqwest::Client::builder()
            .user_agent(format!("BrilliantLauncher/{} ({})", env!("CARGO_PKG_VERSION"), std::env::consts::OS))
            .connect_timeout(std::time::Duration::from_secs(15))
            .timeout(std::time::Duration::from_secs(300))
            .build()
            .expect("建立 HTTP client 失敗")
    })
}

const MIRROR: &[(&str, &str)] = &[
    ("https://piston-meta.mojang.com", "https://bmclapi2.bangbang93.com"),
    ("https://piston-data.mojang.com", "https://bmclapi2.bangbang93.com"),
    ("https://launchermeta.mojang.com", "https://bmclapi2.bangbang93.com"),
    ("https://launcher.mojang.com", "https://bmclapi2.bangbang93.com"),
    ("https://resources.download.minecraft.net", "https://bmclapi2.bangbang93.com/assets"),
    ("https://libraries.minecraft.net", "https://bmclapi2.bangbang93.com/maven"),
    ("https://maven.fabricmc.net", "https://bmclapi2.bangbang93.com/maven"),
    ("https://meta.fabricmc.net", "https://bmclapi2.bangbang93.com/fabric-meta"),
];

pub fn mirrored(url: &str, on: bool) -> String {
    if !on {
        return url.to_string();
    }
    for (from, to) in MIRROR {
        if let Some(rest) = url.strip_prefix(from) {
            return format!("{to}{rest}");
        }
    }
    url.to_string()
}

async fn get_bytes_raw(url: &str) -> R<Vec<u8>> {
    let mut last = anyhow!("未嘗試");
    for attempt in 0..RETRIES {
        match client().get(url).send().await {
            Ok(res) if res.status().is_success() => match res.bytes().await {
                Ok(b) => return Ok(b.to_vec()),
                Err(e) => last = e.into(),
            },
            Ok(res) => last = anyhow!("HTTP {}", res.status()),
            Err(e) => last = e.into(),
        }
        tokio::time::sleep(std::time::Duration::from_millis(300 * (attempt as u64 + 1))).await;
    }
    Err(last.context(format!("下載失敗：{url}")).into())
}

pub async fn get_bytes(url: &str, mirror: bool) -> R<Vec<u8>> {
    let m = mirrored(url, mirror);
    match get_bytes_raw(&m).await {
        Ok(b) => Ok(b),
        Err(e) if m != url => {
            eprintln!("[net] 鏡像失敗，改用官方來源：{url}（{:#}）", e.0);
            get_bytes_raw(url).await
        }
        Err(e) => Err(e),
    }
}

pub async fn get_json<T: DeserializeOwned>(url: &str, mirror: bool) -> R<T> {
    let body = get_bytes(url, mirror).await?;
    Ok(serde_json::from_slice(&body).with_context(|| format!("解析 JSON 失敗：{url}"))?)
}

pub async fn post_json<B: Serialize, T: DeserializeOwned>(url: &str, body: &B) -> R<T> {
    let res = client().post(url).json(body).send().await?;
    let status = res.status();
    let text = res.text().await?;
    if !status.is_success() {
        bail!("HTTP {status}：{text}");
    }
    Ok(serde_json::from_str(&text).with_context(|| format!("解析 JSON 失敗：{url}"))?)
}

pub fn sha1_hex(bytes: &[u8]) -> String {
    hex::encode(Sha1::digest(bytes))
}

pub static DEEP_VERIFY: AtomicBool = AtomicBool::new(false);

async fn already_good(path: &Path, sha1: Option<&str>, size: u64) -> bool {
    let Ok(meta) = tokio::fs::metadata(path).await else {
        return false;
    };
    if !meta.is_file() {
        return false;
    }
    if size > 0 && (meta.len() != size || !DEEP_VERIFY.load(Ordering::Relaxed)) {
        return meta.len() == size;
    }
    match sha1 {
        Some(want) => tokio::fs::read(path)
            .await
            .map(|b| sha1_hex(&b).eq_ignore_ascii_case(want))
            .unwrap_or(false),
        None => true,
    }
}

#[derive(Clone)]
pub struct DlFile {
    pub url: String,
    pub path: PathBuf,
    pub sha1: Option<String>,
    pub size: u64,
    pub executable: bool,
}

impl DlFile {
    pub fn new(url: impl Into<String>, path: PathBuf) -> Self {
        Self { url: url.into(), path, sha1: None, size: 0, executable: false }
    }
    pub fn sha1(mut self, v: Option<String>) -> Self {
        self.sha1 = v;
        self
    }
    pub fn size(mut self, v: u64) -> Self {
        self.size = v;
        self
    }
    pub fn exec(mut self, v: bool) -> Self {
        self.executable = v;
        self
    }
}

#[derive(Serialize, Clone)]
pub struct Progress {
    pub stage: String,
    pub label: String,
    pub done_files: u64,
    pub total_files: u64,
    pub done_bytes: u64,
    pub total_bytes: u64,
}

pub fn emit_stage(app: &AppHandle, stage: &str, label: &str) {
    let _ = app.emit(
        "launcher://progress",
        Progress {
            stage: stage.into(),
            label: label.into(),
            done_files: 0,
            total_files: 0,
            done_bytes: 0,
            total_bytes: 0,
        },
    );
}

pub async fn write_file(path: &Path, bytes: &[u8], executable: bool) -> R<()> {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let tmp = path.with_extension(format!("part{}", rand::random::<u32>()));
    tokio::fs::write(&tmp, bytes)
        .await
        .with_context(|| format!("寫入失敗：{}", tmp.display()))?;
    let _ = tokio::fs::remove_file(path).await;
    tokio::fs::rename(&tmp, path)
        .await
        .with_context(|| format!("搬移失敗：{}", path.display()))?;
    let _ = executable;
    #[cfg(unix)]
    if executable {
        use std::os::unix::fs::PermissionsExt;
        let _ = tokio::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).await;
    }
    Ok(())
}

pub async fn download_all(app: &AppHandle, stage: &str, files: Vec<DlFile>, mirror: bool) -> R<()> {
    let total_files = files.len() as u64;
    let total_bytes: u64 = files.iter().map(|f| f.size).sum();
    let done_files = Arc::new(AtomicU64::new(0));
    let done_bytes = Arc::new(AtomicU64::new(0));

    let _ = app.emit(
        "launcher://progress",
        Progress {
            stage: stage.into(),
            label: format!("共 {total_files} 個檔案"),
            done_files: 0,
            total_files,
            done_bytes: 0,
            total_bytes,
        },
    );

    let mut stream = futures::stream::iter(files.into_iter().map(|f| {
        let app = app.clone();
        let stage = stage.to_string();
        let (df, db) = (done_files.clone(), done_bytes.clone());
        async move {
            if !already_good(&f.path, f.sha1.as_deref(), f.size).await {
                let bytes = get_bytes(&f.url, mirror).await?;
                if let Some(want) = &f.sha1 {
                    let got = sha1_hex(&bytes);
                    if !got.eq_ignore_ascii_case(want) {
                        bail!("SHA1 不符（{}）：預期 {want}，實得 {got}", f.url);
                    }
                }
                write_file(&f.path, &bytes, f.executable).await?;
            }
            let n = df.fetch_add(1, Ordering::Relaxed) + 1;
            let b = db.fetch_add(f.size, Ordering::Relaxed) + f.size;
            let name = f
                .path
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default();
            let _ = app.emit(
                "launcher://progress",
                Progress { stage, label: name, done_files: n, total_files, done_bytes: b, total_bytes },
            );
            Ok::<(), crate::err::Error>(())
        }
    }))
    .buffer_unordered(CONCURRENCY.load(Ordering::Relaxed));

    while let Some(r) = stream.next().await {
        r?;
    }
    Ok(())
}

#[cfg(test)]
mod deep_verify_tests {
    use super::*;

    #[tokio::test]
    async fn deep_mode_catches_same_size_corruption() {
        let path = std::env::temp_dir().join("brilliant-deep-verify.bin");
        std::fs::write(&path, b"broken").unwrap();
        let want = sha1_hex(b"intact");
        assert!(already_good(&path, Some(&want), 6).await);
        DEEP_VERIFY.store(true, Ordering::Relaxed);
        let deep = already_good(&path, Some(&want), 6).await;
        DEEP_VERIFY.store(false, Ordering::Relaxed);
        assert!(!deep);
        std::fs::write(&path, b"intact").unwrap();
        DEEP_VERIFY.store(true, Ordering::Relaxed);
        let fixed = already_good(&path, Some(&want), 6).await;
        DEEP_VERIFY.store(false, Ordering::Relaxed);
        assert!(fixed);
        let _ = std::fs::remove_file(path);
    }
}
