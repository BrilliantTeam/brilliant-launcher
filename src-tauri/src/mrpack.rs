use crate::err::R;
use crate::instance::{self, Instance};
use crate::net::{self, DlFile};
use crate::state::AppState;
use crate::{bail, paths};
use anyhow::Context;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use tauri::{AppHandle, State};

const API: &str = "https://api.modrinth.com/v2";
const ALLOWED_HOSTS: [&str; 4] = ["cdn.modrinth.com", "github.com", "raw.githubusercontent.com", "gitlab.com"];
const MAX_FILES: usize = 600;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Index {
    #[serde(default)]
    game: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    files: Vec<IndexFile>,
    #[serde(default)]
    dependencies: HashMap<String, String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct IndexFile {
    path: String,
    hashes: HashMap<String, String>,
    #[serde(default)]
    env: Option<HashMap<String, String>>,
    #[serde(default)]
    downloads: Vec<String>,
    #[serde(default)]
    file_size: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Imported {
    pub instance_id: String,
    pub name: String,
    pub mods: Vec<String>,
    pub skipped: Vec<String>,
}

pub fn safe_relative(path: &str) -> Option<PathBuf> {
    let p = Path::new(path);
    if path.is_empty() || p.is_absolute() || path.contains(':') {
        return None;
    }
    let mut out = PathBuf::new();
    for part in p.components() {
        match part {
            Component::Normal(s) => out.push(s),
            _ => return None,
        }
    }
    Some(out)
}

pub fn allowed_url(url: &str) -> bool {
    url.strip_prefix("https://")
        .and_then(|rest| rest.split('/').next())
        .is_some_and(|host| ALLOWED_HOSTS.contains(&host))
}

fn file_name_safe(name: &str) -> String {
    let cleaned: String = name.chars().map(|c| if r#"<>:"/\|?*"#.contains(c) || c.is_control() { '_' } else { c }).collect();
    let trimmed = cleaned.trim().trim_end_matches('.').to_string();
    if trimmed.is_empty() { "模組組合".into() } else { trimmed }
}

#[tauri::command]
pub async fn profile_export(id: String, name: String, files: Vec<String>) -> R<String> {
    let inst = instance::get(&id).await?;
    let mods_dir = paths::game_dir(&id).join("mods");
    let mut locals = Vec::new();
    for file in &files {
        if file.contains(['/', '\\']) || file.contains("..") || !file.ends_with(".jar") && !file.ends_with(".jar.disabled") {
            bail!("模組檔名不正確：{file}");
        }
        let bytes = tokio::fs::read(mods_dir.join(file)).await.with_context(|| format!("讀取 {file} 失敗"))?;
        locals.push((file.trim_end_matches(".disabled").to_string(), net::sha1_hex(&bytes), bytes));
    }
    let hashes: Vec<&String> = locals.iter().map(|(_, h, _)| h).collect();
    let found: HashMap<String, Value> = if hashes.is_empty() {
        HashMap::new()
    } else {
        net::post_json(&format!("{API}/version_files"), &json!({ "hashes": hashes, "algorithm": "sha1" })).await.unwrap_or_default()
    };

    let mut entries = Vec::new();
    let mut overrides = Vec::new();
    for (file, sha1, bytes) in locals {
        let remote = found.get(&sha1).and_then(|v| v.get("files")?.as_array()?.iter().find(|f| f.pointer("/hashes/sha1").and_then(Value::as_str) == Some(sha1.as_str())).cloned());
        match remote {
            Some(f) => entries.push(json!({
                "path": format!("mods/{file}"),
                "hashes": { "sha1": sha1, "sha512": f.pointer("/hashes/sha512").and_then(Value::as_str).unwrap_or_default() },
                "env": { "client": "required", "server": "unsupported" },
                "downloads": [f.get("url").and_then(Value::as_str).unwrap_or_default()],
                "fileSize": f.get("size").and_then(Value::as_u64).unwrap_or(bytes.len() as u64)
            })),
            None => overrides.push((file, bytes)),
        }
    }

    let mut deps = serde_json::Map::new();
    deps.insert("minecraft".into(), json!(inst.mc_version));
    if inst.loader == "fabric" && !inst.loader_version.is_empty() {
        deps.insert("fabric-loader".into(), json!(inst.loader_version));
    }
    let index = json!({
        "formatVersion": 1,
        "game": "minecraft",
        "versionId": "1.0.0",
        "name": name,
        "summary": format!("由輝煌啟動器匯出的模組組合（{}）", inst.name),
        "files": entries,
        "dependencies": deps
    });

    let out_dir = dirs::download_dir().unwrap_or_else(|| paths::root().join("exports"));
    std::fs::create_dir_all(&out_dir)?;
    let dest = out_dir.join(format!("{}.mrpack", file_name_safe(&format!("{}-{}", inst.name, name))));
    let target = dest.clone();
    tokio::task::spawn_blocking(move || -> R<()> {
        let mut zip = zip::ZipWriter::new(std::fs::File::create(&target)?);
        let opts = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        zip.start_file("modrinth.index.json", opts)?;
        zip.write_all(serde_json::to_string_pretty(&index)?.as_bytes())?;
        for (file, bytes) in overrides {
            zip.start_file(format!("overrides/mods/{file}"), opts)?;
            zip.write_all(&bytes)?;
        }
        zip.finish()?;
        Ok(())
    })
    .await
    .context("匯出工作中斷")??;
    Ok(dest.to_string_lossy().to_string())
}

fn read_pack(path: &Path) -> R<(Index, Vec<(PathBuf, Vec<u8>)>)> {
    let mut zip = zip::ZipArchive::new(std::fs::File::open(path).context("打不開這個檔案")?).context("這不是有效的 .mrpack 檔")?;
    let mut raw = String::new();
    zip.by_name("modrinth.index.json").context("檔案裡找不到 modrinth.index.json")?.read_to_string(&mut raw)?;
    let index: Index = serde_json::from_str(&raw).context("modrinth.index.json 格式不正確")?;
    let mut overrides = Vec::new();
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i)?;
        if entry.is_dir() {
            continue;
        }
        let name = entry.name().replace('\\', "/");
        let rel = name.strip_prefix("overrides/").or_else(|| name.strip_prefix("client-overrides/"));
        if let Some(rel) = rel.and_then(safe_relative) {
            let mut bytes = Vec::new();
            entry.read_to_end(&mut bytes)?;
            overrides.push((rel, bytes));
        }
    }
    Ok((index, overrides))
}

#[tauri::command]
pub async fn mrpack_import(app: AppHandle, state: State<'_, AppState>, path: String, target: Option<String>) -> R<Imported> {
    let source = PathBuf::from(&path);
    let (index, overrides) = tokio::task::spawn_blocking(move || read_pack(&source)).await.context("讀取模組包中斷")??;
    if index.game != "minecraft" {
        bail!("這不是 Minecraft 的模組包");
    }
    if index.files.len() > MAX_FILES {
        bail!("模組包的檔案太多了（超過 {MAX_FILES} 個）");
    }
    for loader in ["forge", "neoforge", "quilt-loader"] {
        if index.dependencies.contains_key(loader) {
            bail!("這是 {loader} 模組包，輝煌啟動器只支援 Fabric");
        }
    }
    let mc = index.dependencies.get("minecraft").cloned().context("模組包沒有標示 Minecraft 版本")?;
    let fabric = index.dependencies.get("fabric-loader").cloned();

    let inst: Instance = match target {
        Some(id) => {
            let inst = instance::get(&id).await?;
            if state.is_running(&id).await {
                bail!("請先關閉遊戲再匯入");
            }
            if inst.mc_version != mc {
                bail!("這個模組包是給 Minecraft {mc} 的，目前實例是 {}，請改用「建立新實例」", inst.mc_version);
            }
            inst
        }
        None => {
            let draft = Instance {
                name: if index.name.trim().is_empty() { format!("模組包 {mc}") } else { index.name.chars().take(40).collect() },
                icon: "briefcase".into(),
                mc_version: mc.clone(),
                loader: if fabric.is_some() { "fabric".into() } else { "vanilla".into() },
                loader_version: fabric.clone().unwrap_or_default(),
                ..Default::default()
            };
            crate::commands::create_instance(draft).await?
        }
    };

    let game = paths::game_dir(&inst.id);
    let mut jobs = Vec::new();
    let mut mods = Vec::new();
    let mut skipped = Vec::new();
    for file in &index.files {
        if file.env.as_ref().and_then(|e| e.get("client")).is_some_and(|c| c == "unsupported") {
            continue;
        }
        let Some(rel) = safe_relative(&file.path) else {
            skipped.push(file.path.clone());
            continue;
        };
        let Some(url) = file.downloads.iter().find(|u| allowed_url(u)) else {
            skipped.push(file.path.clone());
            continue;
        };
        let Some(sha1) = file.hashes.get("sha1").filter(|h| h.len() == 40) else {
            skipped.push(file.path.clone());
            continue;
        };
        if rel.starts_with("mods") {
            if let Some(n) = rel.file_name() {
                mods.push(n.to_string_lossy().to_string());
            }
        }
        jobs.push(DlFile::new(url.clone(), game.join(&rel)).sha1(Some(sha1.clone())).size(file.file_size));
    }
    if !jobs.is_empty() {
        net::download_all(&app, "mods", jobs, false).await?;
    }
    for (rel, bytes) in overrides {
        if rel.starts_with("mods") {
            if let Some(n) = rel.file_name() {
                mods.push(n.to_string_lossy().to_string());
            }
        }
        let dest = game.join(&rel);
        if let Some(parent) = dest.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        tokio::fs::write(&dest, bytes).await.with_context(|| format!("寫入 {} 失敗", rel.display()))?;
    }
    mods.sort();
    mods.dedup();
    Ok(Imported { instance_id: inst.id, name: index.name, mods, skipped })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore]
    async fn exports_real_mods() {
        let src = std::env::var("MODS_FROM").expect("MODS_FROM");
        let home = std::env::temp_dir().join(format!("bl-export-{}", std::process::id()));
        std::env::set_var("BRILLIANT_LAUNCHER_HOME", &home);
        let inst = instance::create(Instance { name: "測試".into(), mc_version: "26.3".into(), loader: "fabric".into(), loader_version: "0.19.5".into(), ..Default::default() }).await.unwrap();
        let mods = paths::game_dir(&inst.id).join("mods");
        let mut files = Vec::new();
        for e in std::fs::read_dir(&src).unwrap().flatten() {
            let n = e.file_name().to_string_lossy().to_string();
            if n.ends_with(".jar") && !n.starts_with("brilliant-server") {
                std::fs::copy(e.path(), mods.join(&n)).unwrap();
                files.push(n);
            }
        }
        let out = profile_export(inst.id.clone(), "生存".into(), files.clone()).await.unwrap();
        let (index, overrides) = read_pack(Path::new(&out)).unwrap();
        println!("{out}
共 {} 個模組 → 索引 {} 個、內嵌 {} 個，相依 {:?}", files.len(), index.files.len(), overrides.len(), index.dependencies);
        let ok = index.files.iter().filter(|f| f.downloads.iter().all(|u| allowed_url(u)) && f.hashes.get("sha512").is_some_and(|h| h.len() == 128)).count();
        println!("網址與 sha512 都正確：{ok}");
        println!("範例：{} {}", index.files[0].path, index.files[0].downloads[0]);
        std::fs::remove_file(&out).ok();
        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn rejects_paths_and_hosts_outside_the_rules() {
        assert_eq!(safe_relative("mods/sodium.jar"), Some(PathBuf::from("mods").join("sodium.jar")));
        assert!(safe_relative("../evil.jar").is_none());
        assert!(safe_relative("mods/../../evil.jar").is_none());
        assert!(safe_relative("C:/Windows/evil.dll").is_none());
        assert!(safe_relative("/etc/passwd").is_none());
        assert!(allowed_url("https://cdn.modrinth.com/data/AANobbMI/versions/x/sodium.jar"));
        assert!(!allowed_url("http://cdn.modrinth.com/x.jar"));
        assert!(!allowed_url("https://cdn.modrinth.com.evil.example/x.jar"));
        assert_eq!(file_name_safe("輝煌/生存:建築?"), "輝煌_生存_建築_");
    }

    #[test]
    fn reads_index_and_overrides_from_pack() {
        let dir = std::env::temp_dir().join(format!("bl-mrpack-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let pack = dir.join("t.mrpack");
        let mut zip = zip::ZipWriter::new(std::fs::File::create(&pack).unwrap());
        let opts = zip::write::SimpleFileOptions::default();
        zip.start_file("modrinth.index.json", opts).unwrap();
        zip.write_all(br#"{"formatVersion":1,"game":"minecraft","name":"Test","files":[{"path":"mods/a.jar","hashes":{"sha1":"0000000000000000000000000000000000000000"},"downloads":["https://cdn.modrinth.com/a.jar"],"fileSize":1}],"dependencies":{"minecraft":"26.3","fabric-loader":"0.19.5"}}"#).unwrap();
        zip.start_file("overrides/mods/local.jar", opts).unwrap();
        zip.write_all(b"jar").unwrap();
        zip.start_file("overrides/../evil.txt", opts).unwrap();
        zip.write_all(b"x").unwrap();
        zip.finish().unwrap();
        let (index, overrides) = read_pack(&pack).unwrap();
        assert_eq!(index.dependencies.get("fabric-loader").map(String::as_str), Some("0.19.5"));
        assert_eq!(index.files.len(), 1);
        assert_eq!(overrides.len(), 1);
        assert_eq!(overrides[0].0, PathBuf::from("mods").join("local.jar"));
        let _ = std::fs::remove_dir_all(dir);
    }
}
