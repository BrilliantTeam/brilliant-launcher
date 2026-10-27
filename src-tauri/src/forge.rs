use crate::err::R;
use crate::fabric::LoaderInfo;
use crate::mc::{java, version};
use crate::net::{self, DlFile};
use crate::{bail, paths};
use anyhow::Context;
use serde::Deserialize;
use std::io::Read;
use std::path::PathBuf;
use tauri::AppHandle;

const FORGE_MAVEN: &str = "https://maven.minecraftforge.net/net/minecraftforge/forge";
const FORGE_PROMOS: &str = "https://files.minecraftforge.net/net/minecraftforge/forge/promotions_slim.json";
const NEO_MAVEN: &str = "https://maven.neoforged.net/releases/net/neoforged/neoforge";
const NEO_LIST: &str = "https://maven.neoforged.net/api/maven/versions/releases/net/neoforged/neoforge";

pub fn is_forge_like(loader: &str) -> bool {
    matches!(loader, "forge" | "neoforge")
}

pub fn display(loader: &str) -> &'static str {
    match loader {
        "forge" => "Forge",
        "neoforge" => "NeoForge",
        "fabric" => "Fabric",
        _ => "原版",
    }
}

pub fn neo_prefix(mc: &str) -> Option<String> {
    let nums: Vec<u32> = mc.split('.').map(|p| p.parse().ok()).collect::<Option<_>>()?;
    match nums.as_slice() {
        [1, minor] => Some(format!("{minor}.0.")),
        [1, minor, patch] => Some(format!("{minor}.{patch}.")),
        [year, drop] if *year >= 26 => Some(format!("{year}.{drop}.0.")),
        [year, drop, hotfix] if *year >= 26 => Some(format!("{year}.{drop}.{hotfix}.")),
        _ => None,
    }
}

fn version_key(v: &str) -> Vec<u64> {
    v.split(|c: char| !c.is_ascii_digit()).filter(|s| !s.is_empty()).map(|s| s.parse().unwrap_or(0)).collect()
}

pub fn forge_versions(metadata: &str, mc: &str, recommended: Option<&str>) -> Vec<LoaderInfo> {
    let prefix = format!("{mc}-");
    let mut out: Vec<LoaderInfo> = metadata
        .split("<version>")
        .skip(1)
        .filter_map(|chunk| chunk.split("</version>").next())
        .filter_map(|full| full.strip_prefix(&prefix))
        .filter(|v| paths::valid_version_id(v))
        .map(|v| LoaderInfo { version: v.to_string(), stable: Some(v) == recommended })
        .collect();
    out.sort_by_key(|l| std::cmp::Reverse(version_key(&l.version)));
    if recommended.is_none() {
        if let Some(first) = out.first_mut() {
            first.stable = true;
        }
    }
    out
}

pub fn neo_versions(all: &[String], mc: &str) -> Vec<LoaderInfo> {
    let Some(prefix) = neo_prefix(mc) else { return Vec::new() };
    let mut out: Vec<LoaderInfo> = all
        .iter()
        .filter(|v| v.starts_with(&prefix) && paths::valid_version_id(v))
        .map(|v| LoaderInfo { version: v.clone(), stable: !v.contains("beta") && !v.contains("alpha") })
        .collect();
    out.sort_by_key(|l| std::cmp::Reverse(version_key(&l.version)));
    out
}

#[derive(Deserialize)]
struct Promos {
    promos: std::collections::HashMap<String, String>,
}

#[derive(Deserialize)]
struct NeoList {
    versions: Vec<String>,
}

pub async fn loaders(loader: &str, mc: &str, mirror: bool) -> R<Vec<LoaderInfo>> {
    match loader {
        "forge" => {
            let url = format!("{FORGE_MAVEN}/maven-metadata.xml");
            let (metadata, promos) = tokio::join!(net::get_bytes(&url, mirror), net::get_json::<Promos>(FORGE_PROMOS, mirror));
            let metadata = String::from_utf8_lossy(&metadata?).to_string();
            let recommended = promos.ok().and_then(|p| ["recommended", "latest"].iter().find_map(|tag| p.promos.get(&format!("{mc}-{tag}")).cloned()));
            Ok(forge_versions(&metadata, mc, recommended.as_deref()))
        }
        "neoforge" => Ok(neo_versions(&net::get_json::<NeoList>(NEO_LIST, mirror).await?.versions, mc)),
        _ => bail!("不支援的載入器：{loader}"),
    }
}

pub fn version_id(loader: &str, mc: &str, version: &str) -> String {
    if loader == "neoforge" {
        format!("neoforge-{version}")
    } else {
        format!("{mc}-forge-{version}")
    }
}

fn read_installer(jar: &PathBuf) -> R<(String, Vec<PathBuf>)> {
    let mut zip = zip::ZipArchive::new(std::fs::File::open(jar)?).context("安裝器檔案損毀")?;
    let mut raw = String::new();
    zip.by_name("version.json").context("安裝器裡找不到 version.json")?.take(8 << 20).read_to_string(&mut raw)?;
    let parsed: version::VersionJson = serde_json::from_str(&raw).context("安裝器的 version.json 格式不正確")?;
    if !paths::valid_version_id(&parsed.id) {
        bail!("安裝器的版本名稱不正確：{}", parsed.id);
    }
    let generated = parsed
        .libraries
        .iter()
        .filter_map(|lib| lib.downloads.as_ref()?.artifact.as_ref())
        .filter(|a| a.url.is_empty())
        .filter_map(|a| crate::mrpack::safe_relative(a.path.as_deref()?))
        .map(|rel| paths::libraries().join(rel))
        .collect();
    Ok((parsed.id, generated))
}

pub async fn install(app: &AppHandle, loader: &str, mc: &str, version: &str, mirror: bool) -> R<String> {
    if !is_forge_like(loader) || !paths::valid_version_id(mc) || !paths::valid_version_id(version) {
        bail!("載入器版本不正確：{loader} {version}");
    }
    let full = if loader == "forge" { format!("{mc}-{version}") } else { version.to_string() };
    let url = if loader == "forge" { format!("{FORGE_MAVEN}/{full}/forge-{full}-installer.jar") } else { format!("{NEO_MAVEN}/{full}/neoforge-{full}-installer.jar") };
    let jar = paths::shared().join("installers").join(format!("{loader}-{full}-installer.jar"));
    if !jar.is_file() {
        net::download_all(app, "loader", vec![DlFile::new(url, jar.clone())], mirror).await?;
    }
    let source = jar.clone();
    let read = tokio::task::spawn_blocking(move || read_installer(&source)).await.context("讀取安裝器中斷")?;
    let (id, generated) = match read {
        Ok(found) => found,
        Err(error) => {
            let _ = std::fs::remove_file(&jar);
            return Err(error);
        }
    };
    if paths::version_json(&id).is_file() && generated.iter().all(|p| p.is_file()) {
        return Ok(id);
    }
    let base = version::resolve(mc, mirror).await?;
    let java_exe = java::ensure(app, &base.java_version.component, base.java_version.major_version, "", mirror, true).await?;
    let root = paths::shared();
    let profiles = root.join("launcher_profiles.json");
    if !profiles.is_file() {
        net::write_file(&profiles, br#"{"profiles":{}}"#, false).await?;
    }
    net::emit_stage(app, "loader", &format!("安裝 {} {version}（第一次需要幾分鐘）", display(loader)));
    let mut cmd = tokio::process::Command::new(&java_exe);
    cmd.arg("-Djava.awt.headless=true").arg("-jar").arg(&jar).arg("--installClient").arg(&root).current_dir(&root).kill_on_drop(true);
    java::hide_console(&mut cmd);
    let output = cmd.output().await.with_context(|| format!("無法執行 Java：{}", java_exe.display()))?;
    let log = [output.stdout, output.stderr].concat();
    let log_file = paths::logs().join(format!("{loader}-installer.log"));
    let _ = net::write_file(&log_file, &log, false).await;
    if !output.status.success() || !paths::version_json(&id).is_file() || !generated.iter().all(|p| p.is_file()) {
        let text = String::from_utf8_lossy(&log);
        let tail: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
        bail!("{} 安裝失敗，詳細紀錄在 {}\n{}", display(loader), log_file.display(), tail[tail.len().saturating_sub(6)..].join("\n"));
    }
    Ok(id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_minecraft_to_neoforge_versions() {
        assert_eq!(neo_prefix("1.21.1").as_deref(), Some("21.1."));
        assert_eq!(neo_prefix("1.21").as_deref(), Some("21.0."));
        assert_eq!(neo_prefix("26.3").as_deref(), Some("26.3.0."));
        assert_eq!(neo_prefix("26.1.2").as_deref(), Some("26.1.2."));
        assert_eq!(neo_prefix("25w14a"), None);
        let all: Vec<String> = ["21.1.9", "21.1.77", "21.10.1", "26.3.0.99", "26.3.0.100-beta", "0.25w14craftmine.3-beta"].iter().map(|s| s.to_string()).collect();
        let list = neo_versions(&all, "1.21.1");
        assert_eq!(list.iter().map(|l| l.version.as_str()).collect::<Vec<_>>(), vec!["21.1.77", "21.1.9"]);
        let list = neo_versions(&all, "26.3");
        assert_eq!((list[0].version.as_str(), list[0].stable, list[1].stable), ("26.3.0.100-beta", false, true));
    }

    #[test]
    fn reads_forge_metadata() {
        let xml = "<versions><version>26.3-66.0.13</version><version>1.21.1-52.1.20</version><version>26.3-66.1.0</version><version>26.3-66.0.9</version></versions>";
        let list = forge_versions(xml, "26.3", Some("66.0.13"));
        assert_eq!(list.iter().map(|l| (l.version.as_str(), l.stable)).collect::<Vec<_>>(), vec![("66.1.0", false), ("66.0.13", true), ("66.0.9", false)]);
        assert!(forge_versions(xml, "26.3", None)[0].stable);
        assert_eq!(version_id("forge", "26.3", "66.1.0"), "26.3-forge-66.1.0");
        assert_eq!(version_id("neoforge", "26.3", "26.3.0.99"), "neoforge-26.3.0.99");
    }

    #[test]
    fn reads_real_installers() {
        let Some(dir) = std::env::var_os("FORGE_INSTALLERS") else { return };
        for entry in std::fs::read_dir(dir).unwrap().flatten() {
            let (id, generated) = read_installer(&entry.path()).unwrap();
            println!("{} → {id}，需產生 {} 個檔案", entry.path().display(), generated.len());
        }
    }
}
