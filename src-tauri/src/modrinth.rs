use crate::bail;
use crate::err::R;
use crate::instance::DISABLED_SUFFIX;
use crate::net::{self, sha1_hex, DlFile};
use crate::paths;
use anyhow::Context;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::SystemTime;
use tauri::AppHandle;

#[derive(Clone)]
pub struct Digest {
    pub sha1: String,
    pub nested: Vec<String>,
    pub size: u64,
    pub icon: Option<String>,
}

pub fn embedded_icon(bytes: &[u8], is_mod: bool) -> Option<String> {
    use base64::Engine;
    use std::io::Read;
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).ok()?;
    let path = if is_mod {
        let mut raw = String::new();
        zip.by_name("fabric.mod.json").ok()?.read_to_string(&mut raw).ok()?;
        let clean: String = raw.chars().map(|c| if c.is_control() { ' ' } else { c }).collect();
        let meta: serde_json::Value = serde_json::from_str(&clean).ok()?;
        match meta.get("icon")? {
            serde_json::Value::String(s) => s.clone(),
            serde_json::Value::Object(sizes) => sizes
                .iter()
                .filter_map(|(k, v)| Some((k.parse::<u32>().ok()?, v.as_str()?.to_string())))
                .min_by_key(|(size, _)| size.abs_diff(64))?
                .1,
            _ => return None,
        }
    } else {
        "pack.png".to_string()
    };
    let path = path.trim_start_matches("./").trim_start_matches('/');
    let mut entry = zip.by_name(path).ok()?;
    if entry.size() > 512 << 10 {
        return None;
    }
    let mut png = Vec::new();
    entry.read_to_end(&mut png).ok()?;
    png.starts_with(b"\x89PNG").then(|| format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(&png)))
}

type DigestCache = Mutex<HashMap<PathBuf, (u64, SystemTime, Digest)>>;

fn digest_cache() -> &'static DigestCache {
    static CACHE: OnceLock<DigestCache> = OnceLock::new();
    CACHE.get_or_init(Default::default)
}

pub async fn digest(path: &Path) -> Option<Digest> {
    let meta = tokio::fs::metadata(path).await.ok()?;
    let modified = meta.modified().ok()?;
    if let Some((len, time, hit)) = digest_cache().lock().ok()?.get(path) {
        if *len == meta.len() && *time == modified {
            return Some(hit.clone());
        }
    }
    let path_buf = path.to_path_buf();
    let found = tokio::task::spawn_blocking(move || {
        let bytes = std::fs::read(&path_buf).ok()?;
        let name = path_buf.to_string_lossy().to_lowercase();
        let is_mod = name.ends_with(".jar") || name.ends_with(".jar.disabled");
        let nested = if is_mod { nested_jars(&bytes) } else { Vec::new() };
        let icon = if is_mod || name.ends_with(".zip") || name.ends_with(".zip.disabled") { embedded_icon(&bytes, is_mod) } else { None };
        Some(Digest { sha1: sha1_hex(&bytes), nested, size: bytes.len() as u64, icon })
    })
    .await
    .ok()??;
    digest_cache().lock().ok()?.insert(path.to_path_buf(), (meta.len(), modified, found.clone()));
    Some(found)
}

const API: &str = "https://api.modrinth.com/v2";
const CDN: &str = "https://cdn.modrinth.com/";
const FABRIC_API_ID: &str = "P7dR8mSH";

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Kind {
    Mod,
    ResourcePack,
    Shader,
}

impl Kind {
    pub fn parse(kind: &str) -> R<Kind> {
        Ok(match kind {
            "mod" => Kind::Mod,
            "resourcepack" => Kind::ResourcePack,
            "shader" => Kind::Shader,
            _ => bail!("未知的內容類型：{kind}"),
        })
    }

    fn folder(self) -> &'static str {
        match self {
            Kind::Mod => "mods",
            Kind::ResourcePack => "resourcepacks",
            Kind::Shader => "shaderpacks",
        }
    }

    fn project_type(self) -> &'static str {
        match self {
            Kind::Mod => "mod",
            Kind::ResourcePack => "resourcepack",
            Kind::Shader => "shader",
        }
    }

    fn loaders(self) -> &'static str {
        match self {
            Kind::Mod => "[\"fabric\"]",
            Kind::ResourcePack => "[\"minecraft\"]",
            Kind::Shader => "[\"iris\"]",
        }
    }

    fn extension(self) -> &'static str {
        match self {
            Kind::Mod => ".jar",
            _ => ".zip",
        }
    }

    pub fn dir(self, instance_id: &str) -> PathBuf {
        paths::game_dir(instance_id).join(self.folder())
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Hit {
    pub project_id: String,
    pub slug: String,
    pub title: String,
    pub description: String,
    #[serde(default)]
    pub icon_url: Option<String>,
    pub downloads: u64,
    pub author: String,
    #[serde(default)]
    pub display_categories: Vec<String>,
    #[serde(default)]
    pub follows: u64,
    #[serde(default)]
    pub date_modified: String,
    #[serde(default)]
    pub client_side: String,
    #[serde(default)]
    pub server_side: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SearchResult {
    pub hits: Vec<Hit>,
    pub total_hits: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct GalleryImage {
    pub url: String,
    #[serde(default)]
    pub title: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Detail {
    pub id: String,
    pub slug: String,
    pub title: String,
    pub description: String,
    #[serde(default)]
    pub body: String,
    #[serde(default)]
    pub icon_url: Option<String>,
    pub downloads: u64,
    #[serde(default)]
    pub categories: Vec<String>,
    #[serde(default)]
    pub gallery: Vec<GalleryImage>,
    pub project_type: String,
    #[serde(default)]
    pub followers: u64,
    #[serde(default)]
    pub published: String,
    #[serde(default)]
    pub updated: String,
    #[serde(default)]
    pub license: Option<License>,
    #[serde(default)]
    pub client_side: String,
    #[serde(default)]
    pub server_side: String,
    #[serde(default)]
    pub source_url: Option<String>,
    #[serde(default)]
    pub issues_url: Option<String>,
    #[serde(default)]
    pub wiki_url: Option<String>,
    #[serde(default)]
    pub discord_url: Option<String>,
    #[serde(default)]
    pub donation_urls: Vec<Donation>,
    #[serde(default)]
    pub authors: Vec<String>,
    #[serde(default)]
    pub compat: Option<Compat>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct License {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Donation {
    #[serde(default)]
    pub platform: String,
    pub url: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Compat {
    pub mc: String,
    pub version_number: String,
    pub version_type: String,
    pub date_published: String,
    pub changelog: String,
    pub deps: Vec<DepInfo>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct DepInfo {
    pub project_id: String,
    pub title: String,
    pub kind: String,
}

#[derive(Deserialize)]
struct Member {
    user: MemberUser,
    #[serde(default)]
    role: String,
}

#[derive(Deserialize)]
struct MemberUser {
    username: String,
}

#[derive(Deserialize, Clone)]
struct Version {
    id: String,
    project_id: String,
    version_number: String,
    version_type: String,
    files: Vec<VersionFile>,
    #[serde(default)]
    dependencies: Vec<Dependency>,
    #[serde(default)]
    game_versions: Vec<String>,
    #[serde(default)]
    loaders: Vec<String>,
    #[serde(default)]
    date_published: String,
    #[serde(default)]
    changelog: Option<String>,
}

#[derive(Serialize, Clone, Debug)]
pub struct VersionInfo {
    pub id: String,
    pub version_number: String,
    pub version_type: String,
    pub date_published: String,
    pub game_versions: Vec<String>,
    pub loaders: Vec<String>,
    pub file: Option<Update>,
}

#[derive(Serialize, Clone, Debug)]
pub struct DepRef {
    pub project_id: String,
    pub title: String,
    pub disabled: bool,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct VersionFile {
    pub url: String,
    pub filename: String,
    pub primary: bool,
    pub size: u64,
    pub hashes: Hashes,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct Hashes {
    pub sha1: String,
}

#[derive(Deserialize, Clone)]
struct Dependency {
    project_id: Option<String>,
    version_id: Option<String>,
    dependency_type: String,
}

#[derive(Deserialize)]
struct ProjectInfo {
    id: String,
    title: String,
    #[serde(default)]
    slug: String,
    #[serde(default)]
    icon_url: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Update {
    pub version: String,
    pub url: String,
    pub filename: String,
    pub sha1: String,
    pub size: u64,
}

#[derive(Serialize, Clone, Debug)]
pub struct Installed {
    pub file: String,
    pub name: String,
    pub size: u64,
    pub enabled: bool,
    pub managed: bool,
    pub project_id: Option<String>,
    pub icon_url: Option<String>,
    pub version: Option<String>,
    pub update: Option<Update>,
    pub compatible: Option<bool>,
    pub version_id: Option<String>,
    pub pinned: bool,
    pub missing: Vec<DepRef>,
    pub conflicts: Vec<String>,
    pub dependents: Vec<String>,
}

pub(crate) fn encode(text: &str) -> String {
    text.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

pub fn valid_id(id: &str) -> bool {
    (2..=64).contains(&id.len()) && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

pub fn valid_mc(mc: &str) -> bool {
    !mc.is_empty() && mc.len() <= 24 && mc.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-')
}

fn valid_word(text: &str) -> bool {
    text.len() <= 32 && text.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

fn safe_file(name: &str, kind: Kind) -> bool {
    let base = name.trim_end_matches(DISABLED_SUFFIX);
    base.ends_with(kind.extension()) && !name.contains(['/', '\\']) && !name.contains("..") && !name.is_empty()
}

fn side_facets(side: &str) -> R<&'static [&'static str]> {
    Ok(match side {
        "" => &[],
        "client" => &["[\"client_side:required\",\"client_side:optional\"]", "[\"server_side:optional\",\"server_side:unsupported\"]"],
        "server" => &["[\"server_side:required\",\"server_side:optional\"]", "[\"client_side:optional\",\"client_side:unsupported\"]"],
        "both" => &["[\"client_side:required\"]", "[\"server_side:required\"]"],
        _ => bail!("搜尋參數不正確"),
    })
}

pub async fn search(kind: Kind, query: &str, mc: &str, category: &str, sort: &str, offset: u32, side: &str) -> R<SearchResult> {
    if !valid_mc(mc) || !valid_word(category) || !valid_word(sort) {
        bail!("搜尋參數不正確");
    }
    let mut facets = vec![format!("[\"project_type:{}\"]", kind.project_type()), format!("[\"versions:{mc}\"]")];
    if kind == Kind::Mod {
        facets.push("[\"categories:fabric\"]".into());
        facets.extend(side_facets(side)?.iter().map(|f| f.to_string()));
    }
    if !category.is_empty() {
        facets.push(format!("[\"categories:{category}\"]"));
    }
    let index = match sort {
        "downloads" | "follows" | "newest" | "updated" => sort,
        _ if query.trim().is_empty() => "downloads",
        _ => "relevance",
    };
    let url = format!(
        "{API}/search?query={}&facets={}&index={index}&limit=24&offset={offset}",
        encode(query.trim()),
        encode(&format!("[{}]", facets.join(",")))
    );
    net::get_json(&url, false).await
}

fn clip(text: &str, max: usize) -> String {
    if text.chars().count() > max {
        text.chars().take(max).collect::<String>() + "\n⋯"
    } else {
        text.to_string()
    }
}

pub async fn detail(project: &str, kind: Option<Kind>, mc: Option<&str>) -> R<Detail> {
    if !valid_id(project) || mc.is_some_and(|m| !valid_mc(m)) {
        bail!("模組代號不正確");
    }
    let compat_version = async {
        match (kind, mc) {
            (Some(kind), Some(mc)) => version_for(kind, project, mc, "release").await.ok(),
            _ => None,
        }
    };
    let (project_url, members_url) = (format!("{API}/project/{project}"), format!("{API}/project/{project}/members"));
    let (detail, members, version) = tokio::join!(
        net::get_json::<Detail>(&project_url, false),
        net::get_json::<Vec<Member>>(&members_url, false),
        compat_version
    );
    let mut detail = detail?;
    detail.body = clip(&detail.body, 20000);
    detail.gallery.truncate(12);
    let mut members = members.unwrap_or_default();
    members.sort_by_key(|m| !(m.role.to_lowercase().contains("owner") || m.role.to_lowercase().contains("lead")));
    detail.authors = members.into_iter().take(4).map(|m| m.user.username).collect();
    if let (Some(version), Some(mc)) = (version, mc) {
        let ids: Vec<String> = version.dependencies.iter().filter_map(|d| d.project_id.clone()).filter(|id| valid_id(id)).collect();
        let titles: Vec<ProjectInfo> = if ids.is_empty() {
            Vec::new()
        } else {
            net::get_json(&format!("{API}/projects?ids={}", encode(&serde_json::to_string(&ids)?)), false).await.unwrap_or_default()
        };
        let deps = version
            .dependencies
            .iter()
            .filter_map(|d| {
                let id = d.project_id.clone()?;
                let title = titles.iter().find(|p| p.id == id).map(|p| p.title.clone()).unwrap_or_else(|| id.clone());
                Some(DepInfo { project_id: id, title, kind: d.dependency_type.clone() })
            })
            .collect();
        detail.compat = Some(Compat {
            mc: mc.to_string(),
            version_number: version.version_number,
            version_type: version.version_type,
            date_published: version.date_published,
            changelog: clip(version.changelog.as_deref().unwrap_or_default().trim(), 6000),
            deps,
        });
    }
    Ok(detail)
}

fn channel_rank(version_type: &str) -> u8 {
    match version_type {
        "release" => 0,
        "beta" => 1,
        _ => 2,
    }
}

pub fn channel_allows(channel: &str, version_type: &str) -> bool {
    channel_rank(version_type) <= channel_rank(channel)
}

async fn version_for(kind: Kind, project: &str, mc: &str, channel: &str) -> R<Version> {
    let url = format!(
        "{API}/project/{project}/version?game_versions={}&loaders={}",
        encode(&format!("[\"{mc}\"]")),
        encode(kind.loaders())
    );
    let mut versions: Vec<Version> = net::get_json(&url, false).await?;
    if versions.is_empty() {
        bail!("這個項目沒有支援 Minecraft {mc} 的版本");
    }
    let index = versions.iter().position(|v| channel_allows(channel, &v.version_type)).unwrap_or(0);
    Ok(versions.swap_remove(index))
}

fn primary(files: Vec<VersionFile>) -> Option<VersionFile> {
    let index = files.iter().position(|f| f.primary).unwrap_or(0);
    files.into_iter().nth(index)
}

pub async fn install(app: &AppHandle, instance_id: &str, kind: Kind, project: &str, mc: &str, skip_fabric_api: bool, channel: &str) -> R<Vec<String>> {
    if !valid_id(project) || !valid_mc(mc) {
        bail!("項目或版本參數不正確");
    }
    let dir = kind.dir(instance_id);
    let mut queue: Vec<(String, Option<String>)> = vec![(project.to_string(), None)];
    let mut seen: HashSet<String> = installed_projects(&dir, kind).await;
    let mut files = Vec::new();
    let mut names = Vec::new();
    while let Some((project_id, version_id)) = queue.pop() {
        if !seen.insert(project_id.clone()) || names.len() > 24 {
            continue;
        }
        let version: Version = match version_id {
            Some(id) if valid_id(&id) => net::get_json(&format!("{API}/version/{id}"), false).await?,
            _ => version_for(kind, &project_id, mc, channel).await?,
        };
        if skip_fabric_api && version.project_id == FABRIC_API_ID {
            continue;
        }
        if kind == Kind::Mod {
            for dep in &version.dependencies {
                if dep.dependency_type == "required" {
                    if let Some(id) = dep.project_id.clone().filter(|id| valid_id(id)) {
                        queue.push((id, dep.version_id.clone()));
                    }
                }
            }
        }
        let Some(file) = primary(version.files) else {
            bail!("版本 {} 沒有可下載的檔案", version.version_number);
        };
        if !safe_file(&file.filename, kind) {
            bail!("檔名不正確：{}", file.filename);
        }
        if dir.join(&file.filename).is_file() || dir.join(format!("{}{DISABLED_SUFFIX}", file.filename)).is_file() {
            continue;
        }
        names.push(file.filename.clone());
        let mut dl = DlFile::new(file.url, dir.join(&file.filename)).sha1(Some(file.hashes.sha1));
        dl.size = file.size;
        files.push(dl);
    }
    if !files.is_empty() {
        net::download_all(app, "mods", files, false).await?;
    }
    Ok(names)
}

fn managed_name(file: &str) -> Option<&'static str> {
    if file.starts_with(crate::modinject::MOD_PREFIX) {
        Some("Brilliant-Server-Mod")
    } else if file.starts_with("fabric-api-") {
        Some("Fabric API")
    } else {
        None
    }
}

pub async fn installed(instance_id: &str, kind: Kind, mc: &str, pinned: &[String], auto_api: bool, channel: &str, frozen: bool) -> R<Vec<Installed>> {
    let dir = kind.dir(instance_id);
    let mut out = Vec::new();
    let mut hashes = Vec::new();
    let mut embedded: Vec<Vec<String>> = Vec::new();
    let mut local_icons: Vec<Option<String>> = Vec::new();
    if let Ok(mut entries) = tokio::fs::read_dir(&dir).await {
        while let Ok(Some(entry)) = entries.next_entry().await {
            let file = entry.file_name().to_string_lossy().to_string();
            if !safe_file(&file, kind) || !entry.path().is_file() {
                continue;
            }
            let Some(found) = digest(&entry.path()).await else { continue };
            hashes.push(found.sha1);
            embedded.push(if kind == Kind::Mod { found.nested } else { Vec::new() });
            local_icons.push(found.icon);
            let enabled = !file.ends_with(DISABLED_SUFFIX);
            let stem = file.trim_end_matches(DISABLED_SUFFIX).trim_end_matches(kind.extension()).to_string();
            out.push(Installed {
                name: managed_name(&file).map(str::to_string).unwrap_or(stem),
                managed: kind == Kind::Mod && crate::modinject::is_managed(&file),
                size: found.size,
                enabled,
                file,
                project_id: None,
                icon_url: None,
                version: None,
                update: None,
                compatible: None,
                version_id: None,
                pinned: false,
                missing: Vec::new(),
                conflicts: Vec::new(),
                dependents: Vec::new(),
            });
        }
    }
    if !hashes.is_empty() {
        let current: HashMap<String, Version> = net::post_json(&format!("{API}/version_files"), &json!({ "hashes": hashes, "algorithm": "sha1" }))
            .await
            .unwrap_or_default();
        let loaders: Vec<String> = serde_json::from_str(kind.loaders()).unwrap_or_default();
        let latest: HashMap<String, Version> = net::post_json(
            &format!("{API}/version_files/update"),
            &json!({ "hashes": hashes, "algorithm": "sha1", "loaders": loaders, "game_versions": [mc] }),
        )
        .await
        .unwrap_or_default();
        let present: HashMap<String, bool> = out
            .iter()
            .zip(&hashes)
            .filter_map(|(entry, hash)| current.get(hash).map(|v| (v.project_id.clone(), entry.enabled)))
            .collect();
        let mut wanted: HashSet<String> = current.values().map(|v| v.project_id.clone()).collect();
        for (entry, hash) in out.iter().zip(&hashes) {
            if let Some(version) = current.get(hash).filter(|_| entry.enabled) {
                for dep in version.dependencies.iter().filter(|d| d.dependency_type == "required") {
                    if let Some(id) = dep.project_id.clone() {
                        wanted.insert(id);
                    }
                }
            }
        }
        let ids: Vec<String> = wanted.into_iter().filter(|id| valid_id(id)).collect();
        let projects: Vec<ProjectInfo> = if ids.is_empty() {
            Vec::new()
        } else {
            net::get_json(&format!("{API}/projects?ids={}", encode(&serde_json::to_string(&ids)?)), false).await.unwrap_or_default()
        };
        let bundled = |index: usize, id: &str| {
            let slug = projects.iter().find(|p| p.id == id).map(|p| p.slug.to_lowercase()).unwrap_or_default();
            !slug.is_empty() && embedded[index].iter().any(|n| n.starts_with(&slug) || n.replace('_', "-").starts_with(&slug))
        };
        let title_of = |id: &str| projects.iter().find(|p| p.id == id).map(|p| p.title.clone()).unwrap_or_else(|| id.to_string());
        let mut needed_by: HashMap<String, Vec<String>> = HashMap::new();
        let mut fallback: Vec<(usize, String, String, String)> = Vec::new();
        for (index, (entry, hash)) in out.iter_mut().zip(&hashes).enumerate() {
            let Some(version) = current.get(hash) else { continue };
            entry.project_id = Some(version.project_id.clone());
            entry.version = Some(version.version_number.clone());
            entry.version_id = Some(version.id.clone());
            entry.pinned = pinned.contains(&version.project_id);
            if entry.enabled {
                let display = projects.iter().find(|p| p.id == version.project_id).map(|p| p.title.clone()).unwrap_or_else(|| entry.name.clone());
                for dep in &version.dependencies {
                    let Some(id) = dep.project_id.clone() else { continue };
                    match (dep.dependency_type.as_str(), present.get(&id)) {
                        ("required", Some(true)) => needed_by.entry(id).or_default().push(display.clone()),
                        ("required", Some(false)) => entry.missing.push(DepRef { title: title_of(&id), project_id: id, disabled: true }),
                        ("required", None) if !(auto_api && id == FABRIC_API_ID) && !bundled(index, &id) => entry.missing.push(DepRef { title: title_of(&id), project_id: id, disabled: false }),
                        ("incompatible", Some(true)) => entry.conflicts.push(title_of(&id)),
                        _ => {}
                    }
                }
            }
            if let Some(project) = projects.iter().find(|p| p.id == version.project_id) {
                if !entry.managed {
                    entry.name = project.title.clone();
                }
                entry.icon_url = project.icon_url.clone();
            }
            if entry.managed {
                continue;
            }
            entry.compatible = Some(latest.contains_key(hash));
            if frozen {
                continue;
            }
            if let Some(newer) = latest.get(hash).filter(|v| v.id != version.id && !channel_allows(channel, &v.version_type)) {
                let _ = newer;
                fallback.push((index, version.project_id.clone(), version.id.clone(), version.date_published.clone()));
                continue;
            }
            if let Some(newer) = latest.get(hash).filter(|v| v.id != version.id) {
                if let Some(file) = primary(newer.files.clone()) {
                    entry.update = Some(Update {
                        version: newer.version_number.clone(),
                        url: file.url,
                        filename: file.filename,
                        sha1: file.hashes.sha1,
                        size: file.size,
                    });
                }
            }
        }
        for (index, project_id, current_id, current_date) in fallback {
            let Ok(list) = versions(kind, &project_id, Some(mc)).await else { continue };
            let Some(pick) = list.into_iter().find(|v| channel_allows(channel, &v.version_type)) else { continue };
            if pick.id != current_id && pick.date_published > current_date {
                out[index].update = pick.file;
            }
        }
        for entry in out.iter_mut() {
            if let Some(names) = entry.project_id.as_ref().and_then(|id| needed_by.get(id)) {
                entry.dependents = names.clone();
            }
        }
    }
    for (entry, icon) in out.iter_mut().zip(local_icons) {
        if entry.icon_url.is_none() {
            entry.icon_url = icon;
        }
    }
    out.sort_by(|a, b| b.managed.cmp(&a.managed).then(a.name.to_lowercase().cmp(&b.name.to_lowercase())));
    Ok(out)
}

pub fn nested_jars(bytes: &[u8]) -> Vec<String> {
    zip::ZipArchive::new(std::io::Cursor::new(bytes))
        .map(|zip| {
            zip.file_names()
                .filter_map(|n| n.strip_prefix("META-INF/jars/"))
                .filter(|n| n.ends_with(".jar"))
                .map(|n| n.rsplit('/').next().unwrap_or(n).to_lowercase())
                .collect()
        })
        .unwrap_or_default()
}

async fn installed_projects(dir: &std::path::Path, kind: Kind) -> HashSet<String> {
    let mut hashes = Vec::new();
    if let Ok(mut entries) = tokio::fs::read_dir(dir).await {
        while let Ok(Some(entry)) = entries.next_entry().await {
            let file = entry.file_name().to_string_lossy().to_string();
            if safe_file(&file, kind) && entry.path().is_file() {
                if let Some(found) = digest(&entry.path()).await {
                    hashes.push(found.sha1);
                }
            }
        }
    }
    if hashes.is_empty() {
        return HashSet::new();
    }
    let current: HashMap<String, Version> = net::post_json(&format!("{API}/version_files"), &json!({ "hashes": hashes, "algorithm": "sha1" }))
        .await
        .unwrap_or_default();
    current.into_values().map(|v| v.project_id).collect()
}

pub async fn versions(kind: Kind, project: &str, mc: Option<&str>) -> R<Vec<VersionInfo>> {
    if !valid_id(project) || mc.is_some_and(|m| !valid_mc(m)) {
        bail!("項目或版本參數不正確");
    }
    let mut url = format!("{API}/project/{project}/version?loaders={}", encode(kind.loaders()));
    if let Some(mc) = mc {
        url.push_str(&format!("&game_versions={}", encode(&format!("[\"{mc}\"]"))));
    }
    let list: Vec<Version> = net::get_json(&url, false).await?;
    Ok(list
        .into_iter()
        .take(60)
        .map(|v| VersionInfo {
            file: primary(v.files).filter(|f| safe_file(&f.filename, kind)).map(|f| Update {
                version: v.version_number.clone(),
                url: f.url,
                filename: f.filename,
                sha1: f.hashes.sha1,
                size: f.size,
            }),
            id: v.id,
            version_number: v.version_number,
            version_type: v.version_type,
            date_published: v.date_published,
            game_versions: v.game_versions,
            loaders: v.loaders,
        })
        .collect())
}

pub async fn replace(app: &AppHandle, instance_id: &str, kind: Kind, old_file: &str, update: Update) -> R<String> {
    if !safe_file(old_file, kind) || !safe_file(&update.filename, kind) || !update.url.starts_with(CDN) || update.sha1.len() != 40 {
        bail!("更新資訊不正確");
    }
    let dir = kind.dir(instance_id);
    let disabled = old_file.ends_with(DISABLED_SUFFIX);
    let target = if disabled { format!("{}{DISABLED_SUFFIX}", update.filename) } else { update.filename.clone() };
    let mut dl = DlFile::new(update.url, dir.join(&target)).sha1(Some(update.sha1));
    dl.size = update.size;
    net::download_all(app, "mods", vec![dl], false).await?;
    if old_file != target {
        crate::paths::discard(&format!("舊版 {old_file}"), &[(dir.join(old_file), old_file)]).with_context(|| format!("移除舊版失敗：{old_file}"))?;
    }
    Ok(target)
}

pub async fn remove(instance_id: &str, kind: Kind, file: &str) -> R<()> {
    if !safe_file(file, kind) {
        bail!("無效的檔名：{file}");
    }
    crate::paths::discard(&format!("移除 {file}"), &[(kind.dir(instance_id).join(file), file)]).context("移到回收資料夾失敗")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    #[ignore]
    async fn real_detail_has_compat_and_authors() {
        let d = detail("AANobbMI", Some(Kind::Mod), Some("1.21.1")).await.unwrap();
        assert!(!d.authors.is_empty() || d.license.is_some());
        let c = d.compat.expect("Sodium 要有 1.21.1 版本");
        assert_eq!(c.mc, "1.21.1");
        assert!(!c.version_number.is_empty());
        let d = detail("mOgUt4GM", Some(Kind::Mod), Some("1.21.1")).await.unwrap();
        println!("{:?} {:?} {:?}", d.authors, d.license, d.compat.as_ref().map(|c| &c.deps));
    }

    use super::*;

    #[test]
    fn query_encoding_and_validation() {
        assert_eq!(encode("sodium extra"), "sodium%20extra");
        assert_eq!(encode("[\"26.3\"]"), "%5B%2226.3%22%5D");
        assert!(valid_id("AANobbMI") && valid_id("ferrite-core"));
        assert!(!valid_id("../x") && !valid_id("a/b"));
        assert!(valid_mc("26.3") && valid_mc("1.21.11") && !valid_mc("26.3\"]"));
        assert!(valid_word("optimization") && valid_word("") && !valid_word("x\"]"));
    }

    #[tokio::test]
    #[ignore]
    async fn analyse_real_mod_folder() {
        let source = std::env::var("MODS_FROM").expect("MODS_FROM");
        let mc = std::env::var("MODS_MC").unwrap_or_else(|_| "1.21.1".into());
        let home = std::env::temp_dir().join(format!("bl-deps-{}", std::process::id()));
        std::env::set_var("BRILLIANT_LAUNCHER_HOME", &home);
        let dir = Kind::Mod.dir("real");
        std::fs::create_dir_all(&dir).unwrap();
        for entry in std::fs::read_dir(&source).unwrap().flatten() {
            if entry.path().is_file() {
                std::fs::copy(entry.path(), dir.join(entry.file_name())).unwrap();
            }
        }
        let list = installed("real", Kind::Mod, &mc, &[], false, "release", false).await.unwrap();
        let known = list.iter().filter(|e| e.project_id.is_some()).count();
        println!("共 {} 個，Modrinth 認得 {} 個", list.len(), known);
        for e in &list {
            if !e.missing.is_empty() || !e.conflicts.is_empty() || !e.dependents.is_empty() {
                println!("{:<32} 缺:{:?} 衝突:{:?} 被需要:{:?}", e.name, e.missing.iter().map(|m| format!("{}{}", m.title, if m.disabled { "(停用)" } else { "" })).collect::<Vec<_>>(), e.conflicts, e.dependents);
            }
        }
        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn finds_jars_bundled_inside_a_mod() {
        use std::io::Write;
        let mut buf = std::io::Cursor::new(Vec::new());
        let mut zip = zip::ZipWriter::new(&mut buf);
        let opts = zip::write::SimpleFileOptions::default();
        zip.start_file("fabric.mod.json", opts).unwrap();
        zip.write_all(b"{}").unwrap();
        zip.start_file("META-INF/jars/Placeholder-API-2.4.0+1.21.jar", opts).unwrap();
        zip.finish().unwrap();
        assert_eq!(nested_jars(buf.get_ref()), vec!["placeholder-api-2.4.0+1.21.jar".to_string()]);
        assert!(nested_jars(b"not a zip").is_empty());
    }

    #[test]
    fn reads_embedded_icons() {
        use std::io::Write;
        let png = b"\x89PNG\r\n\x1a\nfake".to_vec();
        let build = |files: &[(&str, &[u8])]| {
            let mut buf = std::io::Cursor::new(Vec::new());
            let mut zip = zip::ZipWriter::new(&mut buf);
            for (name, data) in files {
                zip.start_file(*name, zip::write::SimpleFileOptions::default()).unwrap();
                zip.write_all(data).unwrap();
            }
            zip.finish().unwrap();
            buf.into_inner()
        };
        let jar = build(&[("fabric.mod.json", br#"{"id":"x","icon":{"16":"a.png","64":"assets/x/icon.png"}}"#), ("assets/x/icon.png", &png)]);
        assert!(embedded_icon(&jar, true).unwrap().starts_with("data:image/png;base64,"));
        let pack = build(&[("pack.png", &png), ("pack.mcmeta", b"{}")]);
        assert!(embedded_icon(&pack, false).is_some());
        assert!(embedded_icon(&build(&[("fabric.mod.json", br#"{"id":"x"}"#)]), true).is_none());
        if let Some(real) = std::env::var_os("BRILLIANT_MOD_JAR") {
            assert!(embedded_icon(&std::fs::read(real).unwrap(), true).is_some(), "輝煌模組要讀得到圖示");
        }
    }

    #[tokio::test]
    async fn digest_cache_follows_changes() {
        let path = std::env::temp_dir().join("brilliant-digest.jar");
        std::fs::write(&path, b"first").unwrap();
        let a = digest(&path).await.unwrap();
        assert_eq!(a.sha1, sha1_hex(b"first"));
        assert_eq!(digest(&path).await.unwrap().sha1, a.sha1);
        std::fs::write(&path, b"second!").unwrap();
        assert_eq!(digest(&path).await.unwrap().sha1, sha1_hex(b"second!"));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn file_rules_follow_kind() {
        assert!(safe_file("sodium-fabric-0.6.13+mc26.3.jar", Kind::Mod));
        assert!(safe_file("sodium.jar.disabled", Kind::Mod));
        assert!(!safe_file("pack.zip", Kind::Mod));
        assert!(safe_file("ComplementaryReimagined_r5.5.zip", Kind::Shader));
        assert!(!safe_file("../evil.zip", Kind::ResourcePack));
        assert!(!safe_file("a.jar", Kind::ResourcePack));
        assert_eq!(managed_name("brilliant-server-mod-1.1.40+26.3.jar"), Some("Brilliant-Server-Mod"));
        assert_eq!(managed_name("sodium.jar"), None);
    }
}
