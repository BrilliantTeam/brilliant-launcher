use crate::bail;
use crate::err::R;
use crate::instance::DISABLED_SUFFIX;
use crate::modrinth::{channel_allows, clip, encode, safe_file, valid_mc, Compat, DepInfo, DepRef, Detail, GalleryImage, Hit, Installed, Kind, Scope, SearchResult, Update, VersionInfo};
use crate::net::{self, DlFile};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{HashMap, HashSet};
use tauri::AppHandle;

const PROXY: &str = "https://www.brilliantw.net/api/curseforge/v1";
const PREFIX: &str = "cf-";
pub const FABRIC_API: u64 = 306612;
const CDN: [&str; 2] = ["https://edge.forgecdn.net/", "https://mediafilez.forgecdn.net/"];

pub fn murmur2(data: &[u8], seed: u32) -> u32 {
    const M: u32 = 0x5bd1_e995;
    let mut h = seed ^ data.len() as u32;
    let mut chunks = data.chunks_exact(4);
    for c in &mut chunks {
        let mut k = u32::from_le_bytes([c[0], c[1], c[2], c[3]]).wrapping_mul(M);
        k ^= k >> 24;
        h = h.wrapping_mul(M) ^ k.wrapping_mul(M);
    }
    let rest = chunks.remainder();
    if !rest.is_empty() {
        for (i, b) in rest.iter().enumerate() {
            h ^= (*b as u32) << (8 * i);
        }
        h = h.wrapping_mul(M);
    }
    h ^= h >> 13;
    h = h.wrapping_mul(M);
    h ^ (h >> 15)
}

pub fn fingerprint(bytes: &[u8]) -> u32 {
    let data: Vec<u8> = bytes.iter().copied().filter(|b| !matches!(b, 9 | 10 | 13 | 32)).collect();
    murmur2(&data, 1)
}

pub fn is_cf(project: &str) -> bool {
    project.starts_with(PREFIX)
}

pub fn cdn(url: &str) -> bool {
    CDN.iter().any(|prefix| url.starts_with(prefix))
}

fn parse_id(project: &str) -> R<u64> {
    match project.strip_prefix(PREFIX).and_then(|n| n.parse::<u64>().ok()) {
        Some(id) if id > 0 && id <= u32::MAX as u64 => Ok(id),
        _ => bail!("CurseForge 項目代號不正確"),
    }
}

fn tag(id: u64) -> String {
    format!("{PREFIX}{id}")
}

fn loader_type(loader: &str) -> u8 {
    match loader {
        "forge" => 1,
        "neoforge" => 6,
        _ => 4,
    }
}

fn class_id(kind: Kind) -> u32 {
    match kind {
        Kind::Mod => 6,
        Kind::ResourcePack => 12,
        Kind::Shader => 6552,
    }
}

fn channel_name(release_type: u8) -> &'static str {
    match release_type {
        1 => "release",
        2 => "beta",
        _ => "alpha",
    }
}

#[derive(Deserialize)]
struct Envelope<T> {
    data: T,
}

#[derive(Deserialize)]
struct Page<T> {
    data: Vec<T>,
    #[serde(default)]
    pagination: Option<Pagination>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Pagination {
    #[serde(default)]
    total_count: u64,
}

#[derive(Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
struct Mod {
    id: u64,
    #[serde(default)]
    name: String,
    #[serde(default)]
    slug: String,
    #[serde(default)]
    summary: Option<String>,
    #[serde(default)]
    download_count: f64,
    #[serde(default)]
    thumbs_up_count: u64,
    #[serde(default)]
    links: Option<Links>,
    #[serde(default)]
    categories: Vec<Category>,
    #[serde(default)]
    class_id: Option<u32>,
    #[serde(default)]
    authors: Vec<Author>,
    #[serde(default)]
    logo: Option<Asset>,
    #[serde(default)]
    screenshots: Vec<Asset>,
    #[serde(default)]
    latest_files_indexes: Vec<FileIndex>,
    #[serde(default)]
    date_created: Option<String>,
    #[serde(default)]
    date_modified: Option<String>,
}

#[derive(Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
struct Links {
    #[serde(default)]
    website_url: Option<String>,
    #[serde(default)]
    wiki_url: Option<String>,
    #[serde(default)]
    issues_url: Option<String>,
    #[serde(default)]
    source_url: Option<String>,
}

#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Category {
    id: u64,
    #[serde(default)]
    name: String,
    #[serde(default)]
    slug: String,
    #[serde(default)]
    parent_category_id: Option<u64>,
}

#[derive(Deserialize, Clone)]
struct Author {
    #[serde(default)]
    name: String,
}

#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Asset {
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    thumbnail_url: Option<String>,
    #[serde(default)]
    url: Option<String>,
}

#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct FileIndex {
    #[serde(default)]
    game_version: String,
    file_id: u64,
    #[serde(default)]
    release_type: u8,
    #[serde(default)]
    mod_loader: Option<u8>,
}

#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct File {
    id: u64,
    mod_id: u64,
    #[serde(default)]
    display_name: String,
    #[serde(default)]
    file_name: String,
    #[serde(default)]
    release_type: u8,
    #[serde(default)]
    hashes: Vec<Hash>,
    #[serde(default)]
    file_date: String,
    #[serde(default)]
    file_length: u64,
    #[serde(default)]
    download_url: Option<String>,
    #[serde(default)]
    game_versions: Vec<String>,
    #[serde(default)]
    dependencies: Vec<Dep>,
    #[serde(default)]
    is_available: Option<bool>,
    #[serde(default)]
    file_fingerprint: u64,
}

#[derive(Deserialize, Clone)]
struct Hash {
    value: String,
    algo: u8,
}

#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Dep {
    mod_id: u64,
    relation_type: u8,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Matches {
    #[serde(default)]
    exact_matches: Vec<Match>,
}

#[derive(Deserialize)]
struct Match {
    file: File,
}

#[derive(Serialize, Clone, Debug)]
pub struct CategoryInfo {
    pub id: String,
    pub slug: String,
    pub name: String,
}

#[derive(Serialize, Clone, Debug)]
pub struct Blocked {
    pub name: String,
    pub url: String,
}

pub struct PackFile {
    pub path: String,
    pub url: String,
    pub sha1: String,
    pub size: u64,
}

impl File {
    fn sha1(&self) -> Option<String> {
        self.hashes.iter().find(|h| h.algo == 1 && h.value.len() == 40).map(|h| h.value.to_lowercase())
    }

    fn update(&self, kind: Kind) -> Option<Update> {
        let url = self.download_url.clone().filter(|u| cdn(u))?;
        safe_file(&self.file_name, kind).then_some(())?;
        Some(Update { version: self.display_name.clone(), url, filename: self.file_name.clone(), sha1: self.sha1()?, size: self.file_length })
    }
}

impl Mod {
    fn web_url(&self) -> String {
        self.links.as_ref().and_then(|l| l.website_url.clone()).unwrap_or_else(|| format!("https://www.curseforge.com/minecraft/search?search={}", encode(&self.name)))
    }
}

async fn mods_by_id(ids: &[u64]) -> HashMap<u64, Mod> {
    if ids.is_empty() {
        return HashMap::new();
    }
    let found: R<Envelope<Vec<Mod>>> = net::post_json(&format!("{PROXY}/mods"), &json!({ "modIds": ids })).await;
    found.map(|e| e.data.into_iter().map(|m| (m.id, m)).collect()).unwrap_or_default()
}

async fn files_by_id(ids: &[u64]) -> R<Vec<File>> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let found: Envelope<Vec<File>> = net::post_json(&format!("{PROXY}/mods/files"), &json!({ "fileIds": ids })).await?;
    Ok(found.data)
}

async fn identify(prints: &[u32]) -> HashMap<u32, File> {
    if prints.is_empty() {
        return HashMap::new();
    }
    let found: R<Envelope<Matches>> = net::post_json(&format!("{PROXY}/fingerprints/432"), &json!({ "fingerprints": prints })).await;
    found
        .map(|e| e.data.exact_matches.into_iter().filter_map(|m| Some((u32::try_from(m.file.file_fingerprint).ok()?, m.file))).collect())
        .unwrap_or_default()
}

pub async fn match_files(prints: &[u32]) -> HashMap<u32, (u64, u64)> {
    identify(prints).await.into_iter().map(|(print, f)| (print, (f.mod_id, f.id))).collect()
}

fn hit(m: Mod) -> Hit {
    Hit {
        project_id: tag(m.id),
        slug: m.slug,
        title: m.name,
        description: m.summary.unwrap_or_default(),
        icon_url: m.logo.and_then(|l| l.thumbnail_url.or(l.url)),
        downloads: m.download_count as u64,
        author: m.authors.first().map(|a| a.name.clone()).unwrap_or_default(),
        display_categories: m.categories.into_iter().map(|c| c.slug).collect(),
        follows: m.thumbs_up_count,
        date_modified: m.date_modified.unwrap_or_default(),
        client_side: String::new(),
        server_side: String::new(),
    }
}

#[allow(clippy::too_many_arguments)]
pub async fn search(kind: Kind, loader: &str, query: &str, mc: &str, category: &str, sort: &str, offset: u32) -> R<SearchResult> {
    if !valid_mc(mc) || category.len() > 10 || !category.chars().all(|c| c.is_ascii_digit()) {
        bail!("搜尋參數不正確");
    }
    if offset >= 10_000 - 24 {
        return Ok(SearchResult { hits: Vec::new(), total_hits: offset as u64 });
    }
    let sort_field = match sort {
        "downloads" => 6,
        "follows" => 12,
        "updated" => 3,
        "newest" => 11,
        _ => 2,
    };
    let mut url = format!("{PROXY}/mods/search?gameId=432&classId={}&gameVersion={mc}&sortField={sort_field}&sortOrder=desc&pageSize=24&index={offset}", class_id(kind));
    if kind == Kind::Mod {
        url.push_str(&format!("&modLoaderType={}", loader_type(loader)));
    }
    if !category.is_empty() {
        url.push_str(&format!("&categoryId={category}"));
    }
    let query: String = query.trim().chars().take(80).collect();
    if !query.is_empty() {
        url.push_str(&format!("&searchFilter={}", encode(&query)));
    }
    let page: Page<Mod> = net::get_json(&url, false).await?;
    let total = page.pagination.map(|p| p.total_count).unwrap_or(0).min(10_000);
    Ok(SearchResult { hits: page.data.into_iter().map(hit).collect(), total_hits: total })
}

pub async fn categories(kind: Kind) -> R<Vec<CategoryInfo>> {
    let class = class_id(kind) as u64;
    let list: Envelope<Vec<Category>> = net::get_json(&format!("{PROXY}/categories?gameId=432&classId={class}"), false).await?;
    let mut out: Vec<CategoryInfo> = list
        .data
        .into_iter()
        .filter(|c| c.parent_category_id == Some(class) && c.id != class)
        .map(|c| CategoryInfo { id: c.id.to_string(), slug: c.slug, name: c.name })
        .collect();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

async fn file_list(kind: Kind, loader: &str, mod_id: u64, mc: Option<&str>) -> R<Vec<File>> {
    let mut url = format!("{PROXY}/mods/{mod_id}/files?pageSize=50");
    if let Some(mc) = mc {
        url.push_str(&format!("&gameVersion={mc}"));
    }
    if kind == Kind::Mod {
        url.push_str(&format!("&modLoaderType={}", loader_type(loader)));
    }
    let page: Page<File> = net::get_json(&url, false).await?;
    let mut files: Vec<File> = page.data.into_iter().filter(|f| f.is_available != Some(false)).collect();
    files.sort_by_key(|f| std::cmp::Reverse(f.id));
    Ok(files)
}

async fn pick_file(kind: Kind, loader: &str, mod_id: u64, mc: &str, channel: &str) -> R<File> {
    let mut files = file_list(kind, loader, mod_id, Some(mc)).await?;
    if files.is_empty() {
        bail!("這個項目沒有支援 Minecraft {mc} 的版本");
    }
    let index = files.iter().position(|f| channel_allows(channel, channel_name(f.release_type))).unwrap_or(0);
    Ok(files.swap_remove(index))
}

fn relation(kind: u8) -> Option<&'static str> {
    match kind {
        3 => Some("required"),
        2 => Some("optional"),
        5 => Some("incompatible"),
        _ => None,
    }
}

pub async fn detail(project: &str, kind: Option<Kind>, mc: Option<&str>, loader: &str) -> R<Detail> {
    let id = parse_id(project)?;
    if mc.is_some_and(|m| !valid_mc(m)) {
        bail!("模組代號不正確");
    }
    let compat_file = async {
        match (kind, mc) {
            (Some(kind), Some(mc)) => pick_file(kind, loader, id, mc, "release").await.ok(),
            _ => None,
        }
    };
    let (project_url, body_url) = (format!("{PROXY}/mods/{id}"), format!("{PROXY}/mods/{id}/description"));
    let (found, body, file) = tokio::join!(net::get_json::<Envelope<Mod>>(&project_url, false), net::get_json::<Envelope<String>>(&body_url, false), compat_file);
    let m = found?.data;
    let compat = match (file, mc) {
        (Some(file), Some(mc)) => {
            let dep_ids: Vec<u64> = file.dependencies.iter().filter(|d| relation(d.relation_type).is_some()).map(|d| d.mod_id).collect();
            let changelog_url = format!("{PROXY}/mods/{id}/files/{}/changelog", file.id);
            let (titles, changelog) = tokio::join!(mods_by_id(&dep_ids), net::get_json::<Envelope<String>>(&changelog_url, false));
            let deps = file
                .dependencies
                .iter()
                .filter_map(|d| {
                    let kind = relation(d.relation_type)?;
                    let title = titles.get(&d.mod_id).map(|m| m.name.clone()).unwrap_or_else(|| d.mod_id.to_string());
                    Some(DepInfo { project_id: tag(d.mod_id), title, kind: kind.into() })
                })
                .collect();
            Some(Compat {
                mc: mc.to_string(),
                version_number: file.display_name.clone(),
                version_type: channel_name(file.release_type).into(),
                date_published: file.file_date.clone(),
                changelog: clip(changelog.map(|c| c.data).unwrap_or_default().trim(), 6000),
                deps,
            })
        }
        _ => None,
    };
    let links = m.links.clone().unwrap_or_default();
    Ok(Detail {
        id: tag(m.id),
        slug: m.slug.clone(),
        title: m.name.clone(),
        description: m.summary.clone().unwrap_or_default(),
        body: clip(&body.map(|b| b.data).unwrap_or_default(), 20000),
        icon_url: m.logo.as_ref().and_then(|l| l.thumbnail_url.clone().or(l.url.clone())),
        downloads: m.download_count as u64,
        categories: m.categories.iter().map(|c| c.slug.clone()).collect(),
        gallery: m.screenshots.iter().filter_map(|s| Some(GalleryImage { url: s.url.clone()?, title: s.title.clone().filter(|t| !t.is_empty()) })).take(12).collect(),
        project_type: match kind {
            Some(Kind::ResourcePack) => "resourcepack",
            Some(Kind::Shader) => "shader",
            _ => "mod",
        }
        .into(),
        followers: m.thumbs_up_count,
        published: m.date_created.clone().unwrap_or_default(),
        updated: m.date_modified.clone().unwrap_or_default(),
        license: None,
        client_side: String::new(),
        server_side: String::new(),
        source_url: links.source_url,
        issues_url: links.issues_url,
        wiki_url: links.wiki_url,
        discord_url: None,
        donation_urls: Vec::new(),
        authors: m.authors.iter().take(4).map(|a| a.name.clone()).collect(),
        compat,
    })
}

pub async fn versions(kind: Kind, loader: &str, project: &str, mc: Option<&str>) -> R<Vec<VersionInfo>> {
    let id = parse_id(project)?;
    if mc.is_some_and(|m| !valid_mc(m)) {
        bail!("項目或版本參數不正確");
    }
    Ok(file_list(kind, loader, id, mc)
        .await?
        .into_iter()
        .map(|f| VersionInfo {
            file: f.update(kind),
            id: tag(f.id),
            version_number: f.display_name.clone(),
            version_type: channel_name(f.release_type).into(),
            date_published: f.file_date.clone(),
            game_versions: f.game_versions.iter().filter(|v| v.starts_with(|c: char| c.is_ascii_digit())).cloned().collect(),
            loaders: f.game_versions.iter().filter(|v| matches!(v.as_str(), "Fabric" | "Quilt" | "Forge" | "NeoForge" | "Iris" | "OptiFine")).map(|v| v.to_lowercase()).collect(),
        })
        .collect())
}

async fn present_mods(dir: &std::path::Path, kind: Kind) -> HashSet<u64> {
    let mut prints = Vec::new();
    if let Ok(mut entries) = tokio::fs::read_dir(dir).await {
        while let Ok(Some(entry)) = entries.next_entry().await {
            let file = entry.file_name().to_string_lossy().to_string();
            if safe_file(&file, kind) && entry.path().is_file() {
                if let Some(found) = crate::modrinth::digest(&entry.path()).await {
                    prints.push(found.print);
                }
            }
        }
    }
    identify(&prints).await.into_values().map(|f| f.mod_id).collect()
}

#[allow(clippy::too_many_arguments)]
pub async fn install(app: &AppHandle, instance_id: &str, kind: Kind, loader: &str, project: &str, mc: &str, skip_fabric_api: bool, channel: &str) -> R<Vec<String>> {
    let root = parse_id(project)?;
    if !valid_mc(mc) {
        bail!("項目或版本參數不正確");
    }
    let dir = kind.dir(instance_id);
    let mut seen = present_mods(&dir, kind).await;
    let mut queue = vec![root];
    let mut files = Vec::new();
    let mut names = Vec::new();
    let mut blocked = Vec::new();
    while let Some(mod_id) = queue.pop() {
        if !seen.insert(mod_id) || names.len() > 24 || (skip_fabric_api && mod_id == FABRIC_API) {
            continue;
        }
        let file = pick_file(kind, loader, mod_id, mc, channel).await?;
        if kind == Kind::Mod {
            queue.extend(file.dependencies.iter().filter(|d| d.relation_type == 3).map(|d| d.mod_id));
        }
        if !safe_file(&file.file_name, kind) {
            bail!("檔名不正確：{}", file.file_name);
        }
        if dir.join(&file.file_name).is_file() || dir.join(format!("{}{DISABLED_SUFFIX}", file.file_name)).is_file() {
            continue;
        }
        let Some(update) = file.update(kind) else {
            blocked.push(file.file_name.clone());
            continue;
        };
        names.push(update.filename.clone());
        files.push(DlFile::new(update.url, dir.join(&update.filename)).sha1(Some(update.sha1)).size(update.size));
    }
    if !files.is_empty() {
        net::download_all(app, "mods", files, false).await?;
    }
    if !blocked.is_empty() {
        bail!("{} 的作者不允許第三方啟動器下載，請到 CurseForge 網頁手動下載，再拖進啟動器安裝{}", blocked.join("、"), if names.is_empty() { String::new() } else { format!("（其他 {} 個已安裝完成）", names.len()) });
    }
    Ok(names)
}

pub async fn fill(out: &mut [Installed], prints: &[u32], embedded: &[Vec<String>], kind: Kind, scope: &Scope<'_>) -> HashMap<String, String> {
    let Scope { loader, mc, pinned, auto_api, channel, frozen, .. } = *scope;
    let unknown: Vec<usize> = (0..out.len()).filter(|&i| out[i].project_id.is_none() && !out[i].managed).collect();
    let matches = identify(&unknown.iter().map(|&i| prints[i]).collect::<Vec<_>>()).await;
    let found: Vec<(usize, File)> = unknown.into_iter().filter_map(|i| Some((i, matches.get(&prints[i])?.clone()))).collect();
    if found.is_empty() {
        return HashMap::new();
    }
    let mut ids: HashSet<u64> = found.iter().map(|(_, f)| f.mod_id).collect();
    ids.extend(found.iter().filter(|(i, _)| out[*i].enabled).flat_map(|(_, f)| f.dependencies.iter().filter(|d| matches!(d.relation_type, 3 | 5)).map(|d| d.mod_id)));
    let mods = mods_by_id(&ids.into_iter().collect::<Vec<_>>()).await;
    let present: HashMap<u64, bool> = found.iter().map(|(i, f)| (f.mod_id, out[*i].enabled)).collect();
    let slug_of = |id: u64| mods.get(&id).map(|m| m.slug.to_lowercase()).unwrap_or_default();
    let title_of = |id: u64| mods.get(&id).map(|m| m.name.clone()).unwrap_or_else(|| id.to_string());
    let mut needed_by: HashMap<u64, Vec<String>> = HashMap::new();
    let mut wanted: Vec<(usize, u64)> = Vec::new();
    for (index, file) in &found {
        let entry = &mut out[*index];
        let project = tag(file.mod_id);
        entry.version = Some(file.display_name.clone());
        entry.version_id = Some(tag(file.id));
        entry.pinned = pinned.contains(&project);
        entry.project_id = Some(project);
        let Some(m) = mods.get(&file.mod_id) else { continue };
        entry.name = m.name.clone();
        entry.icon_url = m.logo.as_ref().and_then(|l| l.thumbnail_url.clone().or(l.url.clone()));
        if entry.enabled {
            for dep in &file.dependencies {
                let slug = slug_of(dep.mod_id);
                let bundled = !slug.is_empty() && embedded[*index].iter().any(|n| n.starts_with(&slug) || n.replace('_', "-").starts_with(&slug));
                match (dep.relation_type, present.get(&dep.mod_id)) {
                    (3, Some(true)) => needed_by.entry(dep.mod_id).or_default().push(m.name.clone()),
                    (3, Some(false)) => entry.missing.push(DepRef { project_id: tag(dep.mod_id), title: title_of(dep.mod_id), disabled: true }),
                    (3, None) if !(auto_api && dep.mod_id == FABRIC_API) && !bundled => {
                        entry.missing.push(DepRef { project_id: tag(dep.mod_id), title: title_of(dep.mod_id), disabled: false })
                    }
                    (5, Some(true)) => entry.conflicts.push(title_of(dep.mod_id)),
                    _ => {}
                }
            }
        }
        let fits: Vec<&FileIndex> = m.latest_files_indexes.iter().filter(|x| x.game_version == mc && (kind != Kind::Mod || x.mod_loader == Some(loader_type(loader)))).collect();
        entry.compatible = Some(!fits.is_empty());
        if frozen {
            continue;
        }
        if let Some(best) = fits.iter().filter(|x| channel_allows(channel, channel_name(x.release_type))).max_by_key(|x| x.file_id) {
            if best.file_id > file.id {
                wanted.push((*index, best.file_id));
            }
        }
    }
    let newer = files_by_id(&wanted.iter().map(|(_, id)| *id).collect::<Vec<_>>()).await.unwrap_or_default();
    for (index, file_id) in wanted {
        out[index].update = newer.iter().find(|f| f.id == file_id).and_then(|f| f.update(kind));
    }
    for entry in out.iter_mut() {
        if let Some(names) = entry.project_id.as_deref().and_then(|p| p.strip_prefix(PREFIX)).and_then(|n| n.parse::<u64>().ok()).and_then(|id| needed_by.get(&id)) {
            entry.dependents = names.clone();
        }
    }
    mods.values().map(|m| (tag(m.id), m.slug.to_lowercase())).collect()
}

pub async fn resolve_pack(refs: &[(u64, u64)]) -> R<(Vec<PackFile>, Vec<Blocked>, Vec<String>)> {
    let files = files_by_id(&refs.iter().map(|(_, f)| *f).collect::<Vec<_>>()).await?;
    let mods = mods_by_id(&refs.iter().map(|(m, _)| *m).collect::<Vec<_>>()).await;
    let (mut out, mut blocked, mut skipped) = (Vec::new(), Vec::new(), Vec::new());
    for (mod_id, file_id) in refs {
        let m = mods.get(mod_id);
        let label = m.map(|m| m.name.clone()).unwrap_or_else(|| format!("CurseForge 項目 {mod_id}"));
        let Some(file) = files.iter().find(|f| f.id == *file_id) else {
            skipped.push(label);
            continue;
        };
        let (folder, kind) = match m.and_then(|m| m.class_id) {
            Some(12) => ("resourcepacks", Kind::ResourcePack),
            Some(6552) => ("shaderpacks", Kind::Shader),
            Some(6) | None => ("mods", Kind::Mod),
            _ => {
                skipped.push(label);
                continue;
            }
        };
        match file.update(kind) {
            Some(u) => out.push(PackFile { path: format!("{folder}/{}", u.filename), url: u.url, sha1: u.sha1, size: u.size }),
            None if file.download_url.is_none() && safe_file(&file.file_name, kind) => blocked.push(Blocked { name: label, url: m.map(Mod::web_url).unwrap_or_default() }),
            None => skipped.push(label),
        }
    }
    Ok((out, blocked, skipped))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn allowed_by_csp(url: &str) -> bool {
        let conf: serde_json::Value = serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        let csp = conf["app"]["security"]["csp"].as_str().unwrap();
        let img = csp.split(';').find(|p| p.trim_start().starts_with("img-src")).unwrap();
        img.split_whitespace().skip(1).any(|src| src.starts_with("https://") && url.starts_with(&format!("{src}/")))
    }

    #[tokio::test]
    #[ignore]
    async fn live_proxy_end_to_end() {
        for (kind, loader) in [(Kind::Mod, "fabric"), (Kind::Mod, "forge"), (Kind::Mod, "neoforge"), (Kind::ResourcePack, ""), (Kind::Shader, "")] {
            let found = search(kind, loader, "", "1.21.1", "", "relevance", 0).await.unwrap();
            assert!(!found.hits.is_empty(), "{kind:?} {loader} 沒有結果");
            for hit in &found.hits {
                let icon = hit.icon_url.as_deref().unwrap_or_default();
                assert!(icon.is_empty() || allowed_by_csp(icon), "圖示被 CSP 擋住：{icon}");
            }
        }
        let jei = search(Kind::Mod, "neoforge", "jei", "1.21.1", "", "relevance", 0).await.unwrap();
        assert!(jei.hits.iter().any(|h| h.project_id == "cf-238222"));
        assert!(!categories(Kind::Mod).await.unwrap().is_empty());
        let detail = detail("cf-238222", Some(Kind::Mod), Some("1.21.1"), "neoforge").await.unwrap();
        assert!(detail.title.contains("Just Enough Items"));
        assert!(detail.gallery.iter().all(|g| allowed_by_csp(&g.url)), "畫廊圖片被 CSP 擋住");
        let list = versions(Kind::Mod, "neoforge", "cf-238222", Some("1.21.1")).await.unwrap();
        let file = list.iter().find_map(|v| v.file.clone()).expect("沒有可下載的版本");
        assert!(cdn(&file.url));
        let bytes = net::get_bytes(&file.url, false).await.unwrap();
        assert_eq!(net::sha1_hex(&bytes), file.sha1);
        let print = fingerprint(&bytes);
        let (mod_id, file_id) = *match_files(&[print]).await.get(&print).expect("指紋比對不到");
        assert_eq!(mod_id, 238222);
        let (out, blocked, skipped) = resolve_pack(&[(mod_id, file_id)]).await.unwrap();
        assert!(blocked.is_empty() && skipped.is_empty() && out.len() == 1 && out[0].path.starts_with("mods/"));
    }

    #[test]
    fn murmur2_matches_reference_verification() {
        let key: Vec<u8> = (0..=255u8).collect();
        let mut hashes = Vec::new();
        for i in 0..256usize {
            hashes.extend_from_slice(&murmur2(&key[..i], 256 - i as u32).to_le_bytes());
        }
        assert_eq!(murmur2(&hashes, 0), 0x27864C1E);
        assert_eq!(fingerprint(b"a b\r\n\tc"), murmur2(b"abc", 1));
    }

    #[test]
    fn ids_and_hosts() {
        assert_eq!(parse_id("cf-306612").unwrap(), 306612);
        assert!(parse_id("cf-x").is_err() && parse_id("AANobbMI").is_err() && parse_id("cf-0").is_err());
        assert!(is_cf("cf-1") && !is_cf("P7dR8mSH"));
        assert!(cdn("https://edge.forgecdn.net/files/1/2/a.jar") && !cdn("https://edge.forgecdn.net.evil/x.jar"));
    }

    #[test]
    fn reads_api_shapes() {
        let file: File = serde_json::from_str(r#"{"id":5,"modId":9,"displayName":"A 1.0","fileName":"a-1.0.jar","releaseType":2,"hashes":[{"value":"0123456789012345678901234567890123456789","algo":1},{"value":"x","algo":2}],"fileLength":10,"downloadUrl":"https://edge.forgecdn.net/files/0/5/a-1.0.jar","gameVersions":["Fabric","26.3"],"dependencies":[{"modId":306612,"relationType":3}],"fileFingerprint":3000000000}"#).unwrap();
        let u = file.update(Kind::Mod).unwrap();
        assert_eq!((u.filename.as_str(), u.size, channel_name(file.release_type)), ("a-1.0.jar", 10, "beta"));
        let blocked: File = serde_json::from_str(r#"{"id":6,"modId":9,"fileName":"b.jar","downloadUrl":null}"#).unwrap();
        assert!(blocked.update(Kind::Mod).is_none());
        let m: Mod = serde_json::from_str(r#"{"id":9,"name":"A","slug":"a","summary":null,"downloadCount":12.0,"logo":null,"links":{"websiteUrl":"https://www.curseforge.com/minecraft/mc-mods/a","sourceUrl":null}}"#).unwrap();
        assert_eq!(m.web_url(), "https://www.curseforge.com/minecraft/mc-mods/a");
        assert_eq!(hit(m).project_id, "cf-9");
    }
}
