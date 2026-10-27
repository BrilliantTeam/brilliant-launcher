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
const ALLOWED_HOSTS: [&str; 6] = ["cdn.modrinth.com", "github.com", "raw.githubusercontent.com", "gitlab.com", "edge.forgecdn.net", "mediafilez.forgecdn.net"];
const MAX_FILES: usize = 600;
const MAX_INDEX: u64 = 8 << 20;
const MAX_OVERRIDES: u64 = 1 << 30;

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

#[derive(Deserialize)]
struct CfManifest {
    minecraft: CfMinecraft,
    #[serde(default)]
    name: String,
    #[serde(default)]
    files: Vec<CfFile>,
    #[serde(default)]
    overrides: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CfMinecraft {
    version: String,
    #[serde(default)]
    mod_loaders: Vec<CfLoader>,
}

#[derive(Deserialize)]
struct CfLoader {
    id: String,
}

#[derive(Deserialize)]
struct CfFile {
    #[serde(rename = "projectID")]
    project_id: u64,
    #[serde(rename = "fileID")]
    file_id: u64,
    #[serde(default = "yes")]
    required: bool,
}

fn yes() -> bool {
    true
}

impl CfManifest {
    fn into_index(self) -> (Index, Vec<(u64, u64)>) {
        let mut dependencies = HashMap::from([("minecraft".to_string(), self.minecraft.version)]);
        for loader in self.minecraft.mod_loaders {
            let (name, version) = loader.id.split_once('-').unwrap_or((loader.id.as_str(), ""));
            let key = match name {
                "fabric" => "fabric-loader",
                "quilt" => "quilt-loader",
                other => other,
            };
            dependencies.insert(key.to_string(), version.to_string());
        }
        let refs = self.files.iter().filter(|f| f.required).map(|f| (f.project_id, f.file_id)).collect();
        (Index { game: "minecraft".into(), name: self.name, files: Vec::new(), dependencies }, refs)
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Imported {
    pub instance_id: String,
    pub name: String,
    pub mods: Vec<String>,
    pub skipped: Vec<String>,
    pub blocked: Vec<crate::curseforge::Blocked>,
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
    if inst.loader != "vanilla" && !inst.loader_version.is_empty() {
        let key = if inst.loader == "fabric" { "fabric-loader" } else { inst.loader.as_str() };
        deps.insert(key.into(), json!(inst.loader_version));
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
    let stem = file_name_safe(&format!("{}-{}", inst.name, name));
    let dest = paths::unused(out_dir.join(format!("{stem}.mrpack")));
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

type Pack = (Index, Vec<(u64, u64)>, Vec<(PathBuf, Vec<u8>)>);

fn read_pack(path: &Path) -> R<Pack> {
    let mut zip = zip::ZipArchive::new(std::fs::File::open(path).context("打不開這個檔案")?).context("這不是有效的模組包")?;
    let mut raw = String::new();
    let (index, refs, roots) = if zip.by_name("modrinth.index.json").is_ok() {
        zip.by_name("modrinth.index.json")?.take(MAX_INDEX).read_to_string(&mut raw).context("modrinth.index.json 讀取失敗")?;
        let index: Index = serde_json::from_str(&raw).context("modrinth.index.json 格式不正確")?;
        (index, Vec::new(), vec!["overrides/".to_string(), "client-overrides/".to_string()])
    } else {
        zip.by_name("manifest.json").context("這不是模組包：找不到 modrinth.index.json 或 CurseForge 的 manifest.json")?.take(MAX_INDEX).read_to_string(&mut raw).context("manifest.json 讀取失敗")?;
        let manifest: CfManifest = serde_json::from_str(raw.trim_start_matches('\u{feff}')).context("manifest.json 格式不正確")?;
        let root = manifest.overrides.clone().filter(|o| safe_relative(o).is_some()).unwrap_or_else(|| "overrides".into());
        let (index, refs) = manifest.into_index();
        (index, refs, vec![format!("{}/", root.trim_end_matches('/'))])
    };
    let mut overrides = Vec::new();
    let mut total = 0u64;
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i)?;
        if entry.is_dir() {
            continue;
        }
        let name = entry.name().replace('\\', "/");
        let rel = roots.iter().find_map(|root| name.strip_prefix(root.as_str()));
        if let Some(rel) = rel.and_then(safe_relative) {
            total += entry.size();
            if total > MAX_OVERRIDES {
                bail!("模組包附帶的檔案超過 1 GB，為了安全不會匯入");
            }
            let mut bytes = Vec::new();
            (&mut entry).take(MAX_OVERRIDES).read_to_end(&mut bytes).with_context(|| format!("讀取 {name} 失敗"))?;
            overrides.push((rel, bytes));
        }
    }
    Ok((index, refs, overrides))
}

#[tauri::command]
pub async fn mrpack_import(app: AppHandle, state: State<'_, AppState>, path: String, target: Option<String>) -> R<Imported> {
    let source = PathBuf::from(&path);
    let (mut index, refs, overrides) = tokio::task::spawn_blocking(move || read_pack(&source)).await.context("讀取模組包中斷")??;
    if refs.len() > MAX_FILES {
        bail!("模組包的檔案太多了（超過 {MAX_FILES} 個）");
    }
    let mut blocked = Vec::new();
    let mut skipped = Vec::new();
    if !refs.is_empty() {
        if index.dependencies.contains_key("quilt-loader") {
            bail!("這是 Quilt 模組包，輝煌啟動器只支援 Fabric、Forge 與 NeoForge");
        }
        let (files, stopped, missing) = crate::curseforge::resolve_pack(&refs).await.map_err(|e| e.0.context("向 CurseForge 查詢模組包內容失敗"))?;
        blocked = stopped;
        skipped = missing;
        index.files = files
            .into_iter()
            .map(|f| IndexFile { path: f.path, hashes: HashMap::from([("sha1".to_string(), f.sha1)]), env: None, downloads: vec![f.url], file_size: f.size })
            .collect();
    }
    if index.game != "minecraft" {
        bail!("這不是 Minecraft 的模組包");
    }
    if index.files.len() > MAX_FILES {
        bail!("模組包的檔案太多了（超過 {MAX_FILES} 個）");
    }
    if index.dependencies.contains_key("quilt-loader") {
        bail!("這是 Quilt 模組包，輝煌啟動器只支援 Fabric、Forge 與 NeoForge");
    }
    let mc = index.dependencies.get("minecraft").cloned().context("模組包沒有標示 Minecraft 版本")?;
    let (loader, loader_version) = [("fabric-loader", "fabric"), ("neoforge", "neoforge"), ("forge", "forge")]
        .iter()
        .find_map(|(key, name)| Some((name.to_string(), index.dependencies.get(*key)?.clone())))
        .unwrap_or_else(|| ("vanilla".into(), String::new()));

    let target_was_new = target.is_none();
    let inst: Instance = match target {
        Some(id) => {
            let inst = instance::get(&id).await?;
            state.ensure_idle(&id).await?;
            if inst.mc_version != mc {
                bail!("這個模組包是給 Minecraft {mc} 的，目前實例是 {}，請改用「建立新實例」", inst.mc_version);
            }
            if inst.loader != loader && loader != "vanilla" {
                bail!("這個模組包要用 {}，目前實例是 {}，請改用「建立新實例」", crate::forge::display(&loader), crate::forge::display(&inst.loader));
            }
            inst
        }
        None => {
            let base: String = if index.name.trim().is_empty() { format!("模組包 {mc}") } else { index.name.chars().take(36).collect() };
            let draft = Instance {
                name: crate::instance::unique_name(&base).await,
                icon: "briefcase".into(),
                mc_version: mc.clone(),
                loader: loader.clone(),
                loader_version: loader_version.clone(),
                ..Default::default()
            };
            crate::commands::create_instance(draft).await?
        }
    };

    let created = target_was_new;
    let game = paths::game_dir(&inst.id);
    let mut jobs = Vec::new();
    let mut mods = Vec::new();
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
    let written = write_pack(&app, &game, jobs, overrides, &mut mods, if created { None } else { Some(index.name.as_str()) }).await;
    if let Err(error) = written {
        if created {
            let _ = tokio::fs::remove_dir_all(paths::instance_dir(&inst.id)).await;
            return Err(error.0.context("匯入模組包失敗，已收回建立到一半的實例").into());
        }
        return Err(error);
    }
    mods.sort();
    mods.dedup();
    Ok(Imported { instance_id: inst.id, name: index.name, mods, skipped, blocked })
}

async fn write_pack(app: &AppHandle, game: &Path, jobs: Vec<DlFile>, overrides: Vec<(PathBuf, Vec<u8>)>, mods: &mut Vec<String>, existing: Option<&str>) -> R<()> {
    if let Some(pack) = existing {
        let mut replaced: Vec<(PathBuf, String)> = Vec::new();
        for job in &jobs {
            let differs = std::fs::read(&job.path).is_ok_and(|b| job.sha1.as_deref().is_some_and(|want| !net::sha1_hex(&b).eq_ignore_ascii_case(want)));
            if differs {
                replaced.push((job.path.clone(), job.path.strip_prefix(game).unwrap_or(&job.path).to_string_lossy().to_string()));
            }
        }
        for (rel, bytes) in &overrides {
            let dest = game.join(rel);
            if std::fs::read(&dest).is_ok_and(|old| &old != bytes) {
                replaced.push((dest, rel.to_string_lossy().to_string()));
            }
        }
        if !replaced.is_empty() {
            let items: Vec<(&PathBuf, &str)> = replaced.iter().map(|(p, r)| (p, r.as_str())).collect();
            paths::discard(&format!("匯入模組包「{pack}」前被取代的檔案"), &items).context("把會被取代的檔案移到回收資料夾失敗")?;
        }
    }
    if !jobs.is_empty() {
        net::download_all(app, "mods", jobs, false).await?;
    }
    for (rel, bytes) in overrides {
        if rel.starts_with("mods") {
            if let Some(n) = rel.file_name() {
                mods.push(n.to_string_lossy().to_string());
            }
        }
        let dest = game.join(&rel);
        if let Some(parent) = dest.parent() {
            tokio::fs::create_dir_all(parent).await.with_context(|| format!("建立 {} 失敗", parent.display()))?;
        }
        tokio::fs::write(&dest, bytes).await.with_context(|| format!("寫入 {} 失敗", rel.display()))?;
    }
    Ok(())
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
        let (index, _, overrides) = read_pack(Path::new(&out)).unwrap();
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
        let (index, refs, overrides) = read_pack(&pack).unwrap();
        assert_eq!(index.dependencies.get("fabric-loader").map(String::as_str), Some("0.19.5"));
        assert_eq!(index.files.len(), 1);
        assert!(refs.is_empty());
        assert_eq!(overrides.len(), 1);
        assert_eq!(overrides[0].0, PathBuf::from("mods").join("local.jar"));

        let cf = dir.join("t.zip");
        let mut zip = zip::ZipWriter::new(std::fs::File::create(&cf).unwrap());
        zip.start_file("manifest.json", opts).unwrap();
        zip.write_all("\u{feff}{\"minecraft\":{\"version\":\"1.21.1\",\"modLoaders\":[{\"id\":\"fabric-0.16.5\",\"primary\":true}]},\"manifestType\":\"minecraftModpack\",\"name\":\"CF\",\"files\":[{\"projectID\":306612,\"fileID\":5,\"required\":true},{\"projectID\":2,\"fileID\":6,\"required\":false}],\"overrides\":\"extra\"}".as_bytes()).unwrap();
        zip.start_file("extra/config/a.txt", opts).unwrap();
        zip.write_all(b"a").unwrap();
        zip.start_file("overrides/config/b.txt", opts).unwrap();
        zip.write_all(b"b").unwrap();
        zip.finish().unwrap();
        let (index, refs, overrides) = read_pack(&cf).unwrap();
        assert_eq!((index.name.as_str(), index.dependencies.get("fabric-loader").map(String::as_str), index.dependencies.get("minecraft").map(String::as_str)), ("CF", Some("0.16.5"), Some("1.21.1")));
        assert_eq!(refs, vec![(306612, 5)]);
        assert_eq!(overrides.iter().map(|o| o.0.clone()).collect::<Vec<_>>(), vec![PathBuf::from("config").join("a.txt")]);
        let _ = std::fs::remove_dir_all(dir);
    }
}
