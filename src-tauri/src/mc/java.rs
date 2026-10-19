use super::rules::java_platform;
use crate::err::R;
use crate::net::{self, DlFile};
use crate::paths;
use crate::bail;
use anyhow::Context;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use tauri::AppHandle;

const ALL_RUNTIMES: &str = "https://launchermeta.mojang.com/v1/products/java-runtime/all.json";

#[derive(Deserialize, Clone)]
struct RuntimeEntry {
    manifest: FileRef,
}

#[derive(Deserialize, Clone)]
struct FileRef {
    url: String,
    #[serde(default)]
    sha1: Option<String>,
    #[serde(default)]
    size: u64,
}

#[derive(Deserialize)]
struct RuntimeManifest {
    #[serde(default)]
    files: BTreeMap<String, RuntimeFile>,
}

#[derive(Deserialize)]
struct RuntimeFile {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    executable: bool,
    #[serde(default)]
    downloads: Option<RuntimeDownloads>,
    #[serde(default)]
    target: Option<String>,
}

#[derive(Deserialize)]
struct RuntimeDownloads {
    raw: FileRef,
}

#[derive(Serialize, Clone, Debug)]
pub struct JavaInfo {
    pub path: String,
    pub version: String,
    pub major: u32,
    pub source: String,
}

fn exe_name() -> &'static str {
    if cfg!(target_os = "windows") {
        "java.exe"
    } else {
        "java"
    }
}

fn runtime_exe(dir: &Path) -> PathBuf {
    if cfg!(target_os = "macos") {
        let bundled = dir.join("jre.bundle/Contents/Home/bin/java");
        if bundled.exists() {
            return bundled;
        }
    }
    dir.join("bin").join(exe_name())
}

fn component_dir(component: &str) -> PathBuf {
    paths::java_root().join(component)
}

fn parse_major(text: &str) -> Option<u32> {
    let quoted = text.split('"').nth(1)?;
    let mut parts = quoted.split(['.', '_', '-', '+']);
    let first: u32 = parts.next()?.parse().ok()?;
    if first == 1 {
        parts.next()?.parse().ok()
    } else {
        Some(first)
    }
}

pub async fn probe(path: &Path) -> Option<JavaInfo> {
    if !path.is_file() {
        return None;
    }
    let mut cmd = tokio::process::Command::new(path);
    cmd.arg("-version");
    hide_console(&mut cmd);
    let out = cmd.output().await.ok()?;
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stderr),
        String::from_utf8_lossy(&out.stdout)
    );
    let version = text.split('"').nth(1).unwrap_or("").to_string();
    Some(JavaInfo {
        path: path.to_string_lossy().to_string(),
        major: parse_major(&text)?,
        version,
        source: "system".into(),
    })
}

#[cfg(target_os = "windows")]
pub fn hide_console(cmd: &mut tokio::process::Command) {
    cmd.creation_flags(0x0800_0000);
}

#[cfg(not(target_os = "windows"))]
pub fn hide_console(_cmd: &mut tokio::process::Command) {}

fn candidate_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(home) = std::env::var_os("JAVA_HOME") {
        roots.push(PathBuf::from(home));
    }
    for base in [
        r"C:\Program Files\Java",
        r"C:\Program Files\Eclipse Adoptium",
        r"C:\Program Files\Microsoft\jdk",
        r"C:\Program Files\Zulu",
        r"C:\Program Files (x86)\Java",
        "/usr/lib/jvm",
        "/Library/Java/JavaVirtualMachines",
    ] {
        let Ok(entries) = std::fs::read_dir(base) else {
            continue;
        };
        roots.extend(entries.flatten().map(|e| e.path()).filter(|p| p.is_dir()));
    }
    roots
}

pub async fn scan() -> Vec<JavaInfo> {
    let mut found: Vec<JavaInfo> = Vec::new();
    let mut seen = std::collections::BTreeSet::new();

    if let Ok(entries) = std::fs::read_dir(paths::java_root()) {
        for dir in entries.flatten().map(|e| e.path()).filter(|p| p.is_dir()) {
            if let Some(mut info) = probe(&runtime_exe(&dir)).await {
                info.source = "mojang".into();
                if seen.insert(info.path.clone()) {
                    found.push(info);
                }
            }
        }
    }

    for root in candidate_roots() {
        for exe in [root.join("bin").join(exe_name()), root.join(exe_name())] {
            if let Some(info) = probe(&exe).await {
                if seen.insert(info.path.clone()) {
                    found.push(info);
                }
            }
        }
    }

    if let Some(paths_var) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&paths_var) {
            if let Some(info) = probe(&dir.join(exe_name())).await {
                if seen.insert(info.path.clone()) {
                    found.push(info);
                }
            }
        }
    }

    found.sort_by_key(|j| std::cmp::Reverse(j.major));
    found
}

async fn install_mojang_runtime(app: &AppHandle, component: &str, mirror: bool) -> R<PathBuf> {
    net::emit_stage(app, "java", &format!("取得 {component} 執行環境資訊"));
    let all: BTreeMap<String, BTreeMap<String, Vec<RuntimeEntry>>> =
        net::get_json(ALL_RUNTIMES, mirror).await?;

    let platform = java_platform();
    let entry = all
        .get(platform)
        .and_then(|c| c.get(component))
        .and_then(|list| list.first())
        .with_context(|| format!("Mojang 沒有提供 {platform} 的 {component}"))?;

    let manifest: RuntimeManifest = net::get_json(&entry.manifest.url, mirror).await?;
    let dir = component_dir(component);

    let mut jobs = Vec::new();
    let mut links: Vec<(PathBuf, String)> = Vec::new();
    for (rel, file) in &manifest.files {
        let target = dir.join(rel);
        match file.kind.as_str() {
            "directory" => tokio::fs::create_dir_all(&target).await?,
            "file" => {
                if let Some(d) = &file.downloads {
                    jobs.push(
                        DlFile::new(d.raw.url.clone(), target)
                            .sha1(d.raw.sha1.clone())
                            .size(d.raw.size)
                            .exec(file.executable),
                    );
                }
            }
            "link" => {
                if let Some(t) = &file.target {
                    links.push((target, t.clone()));
                }
            }
            _ => {}
        }
    }

    net::download_all(app, "java", jobs, mirror).await?;

    for (target, rel) in links {
        if target.exists() {
            continue;
        }
        let Some(parent) = target.parent() else { continue };
        tokio::fs::create_dir_all(parent).await?;
        let source = parent.join(&rel);
        if source.is_file() {
            let _ = tokio::fs::copy(&source, &target).await;
        }
    }

    let exe = runtime_exe(&dir);
    if !exe.is_file() {
        bail!("{component} 安裝後找不到 {}", exe.display());
    }
    Ok(exe)
}

pub async fn ensure(
    app: &AppHandle,
    component: &str,
    major: u32,
    override_path: &str,
    mirror: bool,
) -> R<PathBuf> {
    if !override_path.trim().is_empty() {
        let path = PathBuf::from(override_path.trim());
        if !path.is_file() {
            bail!("設定的 Java 路徑不存在：{}", path.display());
        }
        return Ok(path);
    }

    let installed = runtime_exe(&component_dir(component));
    if installed.is_file() {
        return Ok(installed);
    }

    if major > 0 {
        if let Some(info) = scan().await.into_iter().find(|j| j.major == major) {
            return Ok(PathBuf::from(info.path));
        }
    }

    match install_mojang_runtime(app, component, mirror).await {
        Ok(exe) => Ok(exe),
        Err(e) => {
            if let Some(info) = scan().await.into_iter().find(|j| j.major >= major) {
                eprintln!("[java] {component} 下載失敗（{:#}），改用系統 Java {}", e.0, info.version);
                return Ok(PathBuf::from(info.path));
            }
            bail!(
                "找不到 Java {major}（{component}），且自動下載失敗：{:#}\n\
                 請自行安裝 Java {major} 後在「設定」頁指定 java 執行檔路徑。",
                e.0
            )
        }
    }
}
