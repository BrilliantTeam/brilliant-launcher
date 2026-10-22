use crate::err::R;
use crate::bail;
use anyhow::{anyhow, Context};
use futures::StreamExt;
use serde::{de::DeserializeOwned, Serialize};
use sha1::{Digest, Sha1};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Emitter};

const RETRIES: usize = 3;
static CONCURRENCY: AtomicUsize = AtomicUsize::new(12);

pub fn set_concurrency(n: u32) {
    CONCURRENCY.store(n.clamp(1, 64) as usize, Ordering::Relaxed);
}

static CLIENT: std::sync::RwLock<Option<reqwest::Client>> = std::sync::RwLock::new(None);

fn build_client(proxy: &str) -> crate::err::R<reqwest::Client> {
    let mut builder = reqwest::Client::builder()
        .user_agent(format!("BrilliantLauncher/{} ({})", env!("CARGO_PKG_VERSION"), std::env::consts::OS))
        .connect_timeout(std::time::Duration::from_secs(10))
        .read_timeout(std::time::Duration::from_secs(30))
        .timeout(std::time::Duration::from_secs(600));
    let proxy = proxy.trim();
    if !proxy.is_empty() {
        if !valid_proxy(proxy) {
            crate::bail!("代理伺服器格式不正確，請填寫 http://主機:連接埠");
        }
        builder = builder.proxy(reqwest::Proxy::all(proxy)?);
    }
    Ok(builder.build()?)
}

pub fn valid_proxy(proxy: &str) -> bool {
    let rest = proxy.strip_prefix("http://").or_else(|| proxy.strip_prefix("https://"));
    rest.is_some_and(|r| {
        let host = r.trim_end_matches('/');
        host.rsplit_once(':').is_some_and(|(h, p)| !h.is_empty() && p.parse::<u16>().is_ok())
            && host.chars().all(|c| c.is_ascii_alphanumeric() || ".-:@_[]".contains(c))
    })
}

pub fn set_proxy(proxy: &str) -> crate::err::R<()> {
    let client = build_client(proxy)?;
    *CLIENT.write().unwrap_or_else(|e| e.into_inner()) = Some(client);
    Ok(())
}

pub fn is_offline(error: &anyhow::Error) -> bool {
    error.chain().any(|cause| {
        cause.downcast_ref::<reqwest::Error>().is_some_and(|e| e.is_connect() || e.is_timeout())
            || cause.downcast_ref::<std::io::Error>().is_some_and(|e| {
                matches!(e.kind(), std::io::ErrorKind::ConnectionRefused | std::io::ErrorKind::ConnectionReset | std::io::ErrorKind::TimedOut | std::io::ErrorKind::NotConnected | std::io::ErrorKind::HostUnreachable | std::io::ErrorKind::NetworkUnreachable | std::io::ErrorKind::NetworkDown)
            })
    })
}

pub async fn reachable() -> bool {
    let attempts = ["www.brilliantw.net:443", "piston-meta.mojang.com:443", "1.1.1.1:443"].map(|host| {
        Box::pin(async move {
            match tokio::time::timeout(std::time::Duration::from_secs(4), tokio::net::TcpStream::connect(host)).await {
                Ok(Ok(_)) => Ok(()),
                _ => Err(()),
            }
        })
    });
    futures::future::select_ok(attempts).await.is_ok()
}

pub fn client() -> reqwest::Client {
    if let Some(client) = CLIENT.read().unwrap_or_else(|e| e.into_inner()).clone() {
        return client;
    }
    let client = build_client("").expect("建立 HTTP client 失敗");
    *CLIENT.write().unwrap_or_else(|e| e.into_inner()) = Some(client.clone());
    client
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

const PROBE: &str = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";
static PREFER_MIRROR: tokio::sync::OnceCell<bool> = tokio::sync::OnceCell::const_new();

async fn timed(url: &str) -> Option<std::time::Duration> {
    let started = std::time::Instant::now();
    let res = client().get(url).timeout(std::time::Duration::from_secs(8)).send().await.ok()?;
    if !res.status().is_success() {
        return None;
    }
    res.bytes().await.ok()?;
    Some(started.elapsed())
}

async fn prefer_mirror() -> bool {
    *PREFER_MIRROR
        .get_or_init(|| async {
            let mirror = mirrored(PROBE, true);
            let (official, fast) = tokio::join!(timed(PROBE), timed(&mirror));
            let choice = match (official, fast) {
                (Some(o), Some(m)) => m < o,
                (Some(_), None) => false,
                _ => true,
            };
            eprintln!("[net] 下載來源測速：官方 {official:?}，鏡像 {fast:?}，使用{}", if choice { "鏡像" } else { "官方" });
            choice
        })
        .await
}

async fn sources(url: &str, mirror: bool) -> Vec<String> {
    let alternate = mirrored(url, true);
    if !mirror || alternate == url {
        return vec![url.to_string()];
    }
    if prefer_mirror().await {
        vec![alternate, url.to_string()]
    } else {
        vec![url.to_string(), alternate]
    }
}

pub async fn get_bytes(url: &str, mirror: bool) -> R<Vec<u8>> {
    let mut last = None;
    for source in sources(url, mirror).await {
        match get_bytes_raw(&source).await {
            Ok(b) => return Ok(b),
            Err(e) => {
                eprintln!("[net] 下載來源失敗，換下一個：{source}（{:#}）", e.0);
                last = Some(e);
            }
        }
    }
    Err(last.unwrap_or_else(|| anyhow!("沒有可用的下載來源：{url}").into()))
}

fn part_path(path: &Path) -> PathBuf {
    let mut name = path.file_name().map(|n| n.to_os_string()).unwrap_or_default();
    name.push(".part");
    path.with_file_name(name)
}

async fn stream_to(url: &str, part: &Path) -> R<()> {
    use tokio::io::AsyncWriteExt;
    if let Some(parent) = part.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let have = tokio::fs::metadata(part).await.map(|m| m.len()).unwrap_or(0);
    let mut request = client().get(url);
    if have > 0 {
        request = request.header(reqwest::header::RANGE, format!("bytes={have}-"));
    }
    let mut res = request.send().await?;
    let append = match res.status() {
        reqwest::StatusCode::PARTIAL_CONTENT => true,
        reqwest::StatusCode::RANGE_NOT_SATISFIABLE if have > 0 => return Ok(()),
        status if status.is_success() => false,
        status => bail!("HTTP {status}"),
    };
    let mut file = tokio::fs::OpenOptions::new().create(true).write(true).append(append).truncate(!append).open(part).await?;
    while let Some(chunk) = res.chunk().await? {
        file.write_all(&chunk).await?;
    }
    file.flush().await?;
    Ok(())
}

async fn fetch_file(f: &DlFile, mirror: bool) -> R<()> {
    let part = part_path(&f.path);
    let mut last = anyhow!("未嘗試");
    for source in sources(&f.url, mirror).await {
        for attempt in 0..RETRIES {
            match stream_to(&source, &part).await {
                Ok(()) => {
                    let bytes = tokio::fs::read(&part).await.unwrap_or_default();
                    let sha_ok = f.sha1.as_deref().is_none_or(|want| sha1_hex(&bytes).eq_ignore_ascii_case(want));
                    let size_ok = f.size == 0 || bytes.len() as u64 == f.size;
                    if sha_ok && size_ok {
                        let _ = tokio::fs::remove_file(&f.path).await;
                        tokio::fs::rename(&part, &f.path).await.with_context(|| format!("搬移失敗：{}", f.path.display()))?;
                        #[cfg(unix)]
                        if f.executable {
                            use std::os::unix::fs::PermissionsExt;
                            let _ = tokio::fs::set_permissions(&f.path, std::fs::Permissions::from_mode(0o755)).await;
                        }
                        return Ok(());
                    }
                    let _ = tokio::fs::remove_file(&part).await;
                    last = anyhow!("檔案校驗不符：{source}");
                }
                Err(e) => last = e.0,
            }
            tokio::time::sleep(std::time::Duration::from_millis(300 * (attempt as u64 + 1))).await;
        }
    }
    Err(last.context(format!("下載失敗：{}", f.url)).into())
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
    let checks = futures::stream::iter(files.into_iter().map(|f| async move { (!already_good(&f.path, f.sha1.as_deref(), f.size).await).then_some(f) }))
        .buffer_unordered(64)
        .collect::<Vec<_>>()
        .await;
    let files: Vec<DlFile> = checks.into_iter().flatten().collect();
    if files.is_empty() {
        return Ok(());
    }
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

    let last_emit = Arc::new(AtomicU64::new(0));
    let started = std::time::Instant::now();
    let mut stream = futures::stream::iter(files.into_iter().map(|f| {
        let app = app.clone();
        let stage = stage.to_string();
        let (df, db, last) = (done_files.clone(), done_bytes.clone(), last_emit.clone());
        async move {
            fetch_file(&f, mirror).await?;
            let n = df.fetch_add(1, Ordering::Relaxed) + 1;
            let b = db.fetch_add(f.size, Ordering::Relaxed) + f.size;
            let now = started.elapsed().as_millis() as u64;
            let previous = last.load(Ordering::Relaxed);
            if n != total_files && (now < previous + 80 || last.compare_exchange(previous, now, Ordering::Relaxed, Ordering::Relaxed).is_err()) {
                return Ok(());
            }
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
mod resume_tests {
    use super::*;
    use std::io::{Read, Write};

    #[tokio::test]
    async fn resumes_partial_download() {
        let body: Vec<u8> = (0..200_000u32).map(|i| (i % 251) as u8).collect();
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let served = body.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming().take(1) {
                let mut stream = stream.unwrap();
                let mut buf = [0u8; 4096];
                let n = stream.read(&mut buf).unwrap();
                let request = String::from_utf8_lossy(&buf[..n]).to_lowercase();
                let start = request.split("range: bytes=").nth(1).and_then(|r| r.split('-').next()).and_then(|v| v.trim().parse::<usize>().ok());
                let (status, slice) = match start {
                    Some(s) => ("206 Partial Content", &served[s..]),
                    None => ("200 OK", &served[..]),
                };
                let _ = write!(stream, "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", slice.len());
                let _ = stream.write_all(slice);
            }
        });
        let dir = std::env::temp_dir().join(format!("brilliant-resume-{port}"));
        let path = dir.join("client.jar");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(part_path(&path), &body[..80_000]).unwrap();
        let file = DlFile::new(format!("http://127.0.0.1:{port}/client.jar"), path.clone()).sha1(Some(sha1_hex(&body))).size(body.len() as u64);
        fetch_file(&file, false).await.unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), body, "續傳後要跟完整檔案一樣");
        assert!(!part_path(&path).exists(), "完成後不留暫存檔");
        let _ = std::fs::remove_dir_all(dir);
    }
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

#[cfg(test)]
mod offline_tests {
    use super::*;

    #[tokio::test]
    async fn detects_connection_failures() {
        let err = client().get("http://127.0.0.1:1/").send().await.unwrap_err();
        assert!(is_offline(&anyhow::Error::from(err).context("下載失敗")));
        assert!(!is_offline(&anyhow!("HTTP 404")));
    }
}

