use crate::err::R;
use crate::instance;
use crate::net::{self, DlFile};
use crate::state::AppState;
use crate::{bail, paths};
use anyhow::Context;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use tauri::{AppHandle, State};

const MODRINTH: &str = "https://api.modrinth.com/v2";
const DISABLED: &str = crate::instance::DISABLED_SUFFIX;

#[derive(Serialize, Deserialize, Default, Clone, Debug, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct Entry {
    pub path: String,
    pub sha1: String,
    pub removed: bool,
}

#[derive(Serialize, Deserialize, Default, Clone)]
#[serde(default, rename_all = "camelCase")]
pub struct Source {
    pub platform: String,
    pub project: String,
    pub version_id: String,
    pub version: String,
    pub name: String,
    pub icon: String,
}

#[derive(Serialize, Deserialize, Default, Clone)]
#[serde(default, rename_all = "camelCase")]
pub struct Link {
    #[serde(flatten)]
    pub source: Source,
    pub parts: Vec<String>,
    pub files: Vec<Entry>,
}

#[derive(Serialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct Release {
    pub version_id: String,
    pub version: String,
    pub url: String,
    pub sha1: String,
    pub size: u64,
    pub changelog: String,
    pub page: String,
    pub mc: String,
}

pub struct Candidate {
    pub mc: Vec<String>,
    pub loaders: Vec<String>,
    pub stable: bool,
    pub release: Release,
}

#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Check {
    pub choices: Vec<Release>,
    pub page: String,
}

fn mc_key(version: &str) -> Option<Vec<u32>> {
    version.split('.').map(|p| p.parse().ok()).collect()
}

pub fn choose(newest_first: Vec<Candidate>, current: &str, mc: &str, loader: &str) -> Check {
    let loader_ok = |c: &Candidate| loader.is_empty() || loader == "vanilla" || c.loaders.is_empty() || c.loaders.iter().any(|l| l.eq_ignore_ascii_case(loader));
    let mine = mc_key(mc);
    let page = newest_first.first().map(|c| c.release.page.clone()).unwrap_or_default();
    let on_stable = newest_first.iter().find(|c| c.release.version_id == current).is_none_or(|c| c.stable);
    let mut choices = Vec::new();
    let same: Vec<&Candidate> = newest_first
        .iter()
        .take_while(|c| c.release.version_id != current)
        .filter(|c| loader_ok(c) && c.mc.iter().any(|v| v == mc) && (c.stable || !on_stable))
        .collect();
    if let Some(pick) = same.iter().find(|c| c.stable).or(same.first()) {
        choices.push(Release { mc: mc.to_string(), ..pick.release.clone() });
    }
    let mut lines: Vec<(Vec<u32>, String, Release)> = Vec::new();
    for candidate in newest_first.iter().filter(|c| c.stable && loader_ok(c)) {
        for version in &candidate.mc {
            let Some(key) = mc_key(version) else { continue };
            if mine.as_ref().is_some_and(|m| &key > m) && !lines.iter().any(|(_, v, _)| v == version) {
                lines.push((key, version.clone(), Release { mc: version.clone(), ..candidate.release.clone() }));
            }
        }
    }
    lines.sort_by(|a, b| b.0.cmp(&a.0));
    choices.extend(lines.into_iter().take(3).map(|(_, _, r)| r));
    Check { choices, page }
}

pub enum Content {
    Download { url: String, size: u64 },
    Bytes(Vec<u8>),
}

pub struct NewFile {
    pub rel: String,
    pub sha1: String,
    pub content: Content,
}

#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Outcome {
    pub version: String,
    pub added: Vec<String>,
    pub updated: Vec<String>,
    pub removed: Vec<String>,
    pub kept: Vec<String>,
    pub skipped: Vec<String>,
    pub blocked: Vec<crate::curseforge::Blocked>,
    pub game_changed: Option<String>,
}

fn link_file(id: &str) -> PathBuf {
    paths::instance_dir(id).join("modpack.json")
}

pub async fn load(id: &str) -> Option<Link> {
    serde_json::from_slice(&tokio::fs::read(link_file(id)).await.ok()?).ok()
}

pub async fn save(id: &str, link: &Link) -> R<()> {
    net::write_file(&link_file(id), &serde_json::to_vec_pretty(link)?, false).await
}

pub fn rel_key(rel: &Path) -> String {
    rel.components().map(|c| c.as_os_str().to_string_lossy().to_string()).collect::<Vec<_>>().join("/")
}

fn is_mod(rel: &str) -> bool {
    rel.starts_with("mods/")
}

fn keeps_user_edits(rel: &str) -> bool {
    !["mods/", "resourcepacks/", "shaderpacks/"].iter().any(|p| rel.starts_with(p))
}

fn on_disk(game: &Path, rel: &str) -> Option<(String, String)> {
    let candidates = if is_mod(rel) { vec![rel.to_string(), format!("{rel}{DISABLED}")] } else { vec![rel.to_string()] };
    candidates.into_iter().find_map(|candidate| {
        let path = game.join(crate::mrpack::safe_relative(&candidate)?);
        let bytes = std::fs::read(path).ok()?;
        Some((candidate, net::sha1_hex(&bytes)))
    })
}

pub struct Plan {
    pub fetch: Vec<(String, NewFile)>,
    pub discard: Vec<String>,
    pub entries: Vec<Entry>,
    pub outcome: Outcome,
}

pub fn plan(game: &Path, old: &[Entry], new_files: Vec<NewFile>) -> Plan {
    let old_map: HashMap<&str, &Entry> = old.iter().map(|e| (e.path.as_str(), e)).collect();
    let new_paths: HashSet<String> = new_files.iter().map(|f| f.rel.clone()).collect();
    let mut out = Plan { fetch: Vec::new(), discard: Vec::new(), entries: Vec::new(), outcome: Outcome::default() };
    for entry in old.iter().filter(|e| !e.removed && !new_paths.contains(&e.path)) {
        match on_disk(game, &entry.path) {
            Some((disk, sha1)) if sha1.eq_ignore_ascii_case(&entry.sha1) => {
                out.discard.push(disk);
                out.outcome.removed.push(entry.path.clone());
            }
            Some(_) => out.outcome.kept.push(entry.path.clone()),
            None => {}
        }
    }
    for file in new_files {
        let previous = old_map.get(file.rel.as_str()).copied();
        let disk = on_disk(game, &file.rel);
        let mut entry = Entry { path: file.rel.clone(), sha1: file.sha1.clone(), removed: false };
        if let Some(prev) = previous {
            if prev.sha1.eq_ignore_ascii_case(&file.sha1) {
                entry.removed = prev.removed;
                if prev.removed || disk.is_some() {
                    out.entries.push(entry);
                    continue;
                }
            } else if keeps_user_edits(&file.rel) && disk.as_ref().is_some_and(|(_, sha1)| !sha1.eq_ignore_ascii_case(&prev.sha1)) {
                entry.sha1 = prev.sha1.clone();
                out.outcome.kept.push(file.rel.clone());
                out.entries.push(entry);
                continue;
            }
        }
        if disk.as_ref().is_some_and(|(_, sha1)| sha1.eq_ignore_ascii_case(&file.sha1)) {
            out.entries.push(entry);
            continue;
        }
        let target = match &disk {
            Some((path, _)) => {
                out.discard.push(path.clone());
                path.clone()
            }
            None => file.rel.clone(),
        };
        if previous.is_some() || disk.is_some() {
            out.outcome.updated.push(file.rel.clone());
        } else {
            out.outcome.added.push(file.rel.clone());
        }
        out.entries.push(entry);
        out.fetch.push((target, file));
    }
    out
}

async fn apply(app: &AppHandle, id: &str, plan: Plan, label: &str) -> R<Vec<Entry>> {
    let game = paths::game_dir(id);
    let staging = paths::instance_dir(id).join(".modpack-staging");
    let _ = tokio::fs::remove_dir_all(&staging).await;
    tokio::fs::create_dir_all(&staging).await.context("建立暫存資料夾失敗")?;
    let mut jobs = Vec::new();
    let mut moves = Vec::new();
    for (index, (target, file)) in plan.fetch.into_iter().enumerate() {
        let temp = staging.join(index.to_string());
        match file.content {
            Content::Download { url, size } => jobs.push(DlFile::new(url, temp.clone()).sha1(Some(file.sha1)).size(size)),
            Content::Bytes(bytes) => tokio::fs::write(&temp, bytes).await.with_context(|| format!("寫入 {} 失敗", file.rel))?,
        }
        moves.push((temp, target));
    }
    if !jobs.is_empty() {
        net::emit_stage(app, "import", &format!("下載 {} 個檔案", jobs.len()));
        if let Err(error) = net::download_all(app, "import", jobs, false).await {
            let _ = tokio::fs::remove_dir_all(&staging).await;
            return Err(error.0.context("下載模組包檔案失敗，實例裡的檔案都沒有變動").into());
        }
    }
    net::emit_stage(app, "import", "套用變更");
    let discard: Vec<(PathBuf, String)> = plan.discard.iter().filter_map(|rel| Some((game.join(crate::mrpack::safe_relative(rel)?), rel.clone()))).collect();
    if !discard.is_empty() {
        let items: Vec<(&PathBuf, &str)> = discard.iter().map(|(p, r)| (p, r.as_str())).collect();
        paths::discard(label, &items).context("把舊檔案移到回收資料夾失敗")?;
    }
    for (temp, target) in moves {
        let dest = game.join(crate::mrpack::safe_relative(&target).context("檔案路徑不正確")?);
        if let Some(parent) = dest.parent() {
            tokio::fs::create_dir_all(parent).await.with_context(|| format!("建立 {} 失敗", parent.display()))?;
        }
        tokio::fs::rename(&temp, &dest).await.with_context(|| format!("寫入 {target} 失敗"))?;
    }
    let _ = tokio::fs::remove_dir_all(&staging).await;
    Ok(plan.entries)
}

pub async fn identify(bytes: &[u8], name: &str) -> Option<Source> {
    let sha1 = net::sha1_hex(bytes);
    if let Ok(version) = net::get_json::<Value>(&format!("{MODRINTH}/version_file/{sha1}?algorithm=sha1"), false).await {
        let project = version.get("project_id").and_then(Value::as_str)?.to_string();
        let info: Value = net::get_json(&format!("{MODRINTH}/project/{project}"), false).await.unwrap_or_default();
        return Some(Source {
            platform: "modrinth".into(),
            version_id: version.get("id").and_then(Value::as_str).unwrap_or_default().to_string(),
            version: version.get("version_number").and_then(Value::as_str).unwrap_or_default().to_string(),
            name: info.get("title").and_then(Value::as_str).unwrap_or_default().to_string(),
            icon: info.get("icon_url").and_then(Value::as_str).unwrap_or_default().to_string(),
            project,
        });
    }
    crate::curseforge::identify_pack(bytes, name).await
}

async fn modrinth_versions(project: &str) -> R<Vec<Candidate>> {
    if !crate::modrinth::valid_id(project) {
        bail!("模組包編號不正確");
    }
    let versions: Vec<Value> = net::get_json(&format!("{MODRINTH}/project/{project}/version"), false).await?;
    let strings = |v: &Value, key: &str| v.get(key).and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_str).map(str::to_string).collect()).unwrap_or_default();
    Ok(versions
        .iter()
        .map(|v| {
            let files = v.get("files").and_then(Value::as_array).cloned().unwrap_or_default();
            let file = files.iter().find(|f| f.get("primary").and_then(Value::as_bool) == Some(true)).or(files.first()).cloned().unwrap_or_default();
            Candidate {
                mc: strings(v, "game_versions"),
                loaders: strings(v, "loaders"),
                stable: v.get("version_type").and_then(Value::as_str) == Some("release"),
                release: Release {
                    version_id: v.get("id").and_then(Value::as_str).unwrap_or_default().to_string(),
                    version: v.get("version_number").and_then(Value::as_str).unwrap_or_default().to_string(),
                    url: file.get("url").and_then(Value::as_str).filter(|u| crate::mrpack::allowed_url(u)).unwrap_or_default().to_string(),
                    sha1: file.pointer("/hashes/sha1").and_then(Value::as_str).unwrap_or_default().to_string(),
                    size: file.get("size").and_then(Value::as_u64).unwrap_or_default(),
                    changelog: v.get("changelog").and_then(Value::as_str).unwrap_or_default().chars().take(4000).collect(),
                    page: format!("https://modrinth.com/modpack/{project}"),
                    mc: String::new(),
                },
            }
        })
        .collect())
}

async fn check(source: &Source, mc: &str, loader: &str) -> R<Check> {
    let candidates = match source.platform.as_str() {
        "modrinth" => modrinth_versions(&source.project).await?,
        "curseforge" => crate::curseforge::pack_versions(&source.project).await?,
        _ => return Ok(Check::default()),
    };
    Ok(choose(candidates, &source.version_id, mc, loader))
}

pub fn mods_from_pack(link: &Link, dir: &str) -> HashSet<String> {
    let prefix = format!("{dir}/");
    link.files.iter().filter(|e| !e.removed).filter_map(|e| e.path.strip_prefix(&prefix).filter(|n| !n.contains('/')).map(str::to_string)).collect()
}

pub async fn note_replaced(id: &str, dir: &str, old_file: &str, new_file: &str, sha1: &str) {
    let Some(mut link) = load(id).await else { return };
    let old = format!("{dir}/{}", old_file.trim_end_matches(DISABLED));
    if let Some(entry) = link.files.iter_mut().find(|e| e.path == old) {
        entry.path = format!("{dir}/{}", new_file.trim_end_matches(DISABLED));
        entry.sha1 = sha1.to_lowercase();
        let _ = save(id, &link).await;
    }
}

pub async fn note_removed(id: &str, dir: &str, file: &str) {
    let Some(mut link) = load(id).await else { return };
    let path = format!("{dir}/{}", file.trim_end_matches(DISABLED));
    if let Some(entry) = link.files.iter_mut().find(|e| e.path == path) {
        entry.removed = true;
        let _ = save(id, &link).await;
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackInfo {
    #[serde(flatten)]
    pub source: Source,
    pub files: usize,
}

#[tauri::command]
pub async fn pack_info(id: String) -> Option<PackInfo> {
    let link = load(&id).await?;
    Some(PackInfo { files: link.files.iter().filter(|e| !e.removed).count(), source: link.source })
}

#[tauri::command]
pub async fn pack_check(id: String) -> R<Check> {
    let link = load(&id).await.context("這個實例沒有記錄來自哪個模組包")?;
    let inst = instance::get(&id).await?;
    check(&link.source, &inst.mc_version, &inst.loader).await
}

#[tauri::command]
pub async fn pack_update(app: AppHandle, state: State<'_, AppState>, id: String, path: Option<String>, version: Option<String>) -> R<Outcome> {
    state.ensure_idle(&id).await?;
    let link = load(&id).await.unwrap_or_default();
    let temp = std::env::temp_dir().join(format!("brilliant-pack-{}.zip", rand::random::<u32>()));
    let (pack_path, release) = match path {
        Some(path) => (PathBuf::from(path), None),
        None => {
            let inst = instance::get(&id).await?;
            let choices = check(&link.source, &inst.mc_version, &inst.loader).await?.choices;
            let release = match version {
                Some(want) => choices.into_iter().find(|r| r.version_id == want).context("找不到選擇的模組包版本，請重新檢查更新")?,
                None => choices.into_iter().find(|r| r.mc == inst.mc_version).context("這個模組包沒有適用目前 Minecraft 版本的新版本")?,
            };
            if release.url.is_empty() {
                bail!("這個模組包的作者不允許第三方啟動器直接下載，請到 {} 下載新版的檔案，再用「從檔案更新」", if release.page.is_empty() { "模組包頁面".to_string() } else { release.page.clone() });
            }
            net::emit_stage(&app, "import", &format!("下載模組包 {}", release.version));
            net::download_all(&app, "import", vec![DlFile::new(release.url.clone(), temp.clone()).sha1(Some(release.sha1.clone()).filter(|s| s.len() == 40)).size(release.size)], false).await?;
            (temp.clone(), Some(release))
        }
    };
    let result = update_from(&app, &id, &link, &pack_path, release).await;
    let _ = tokio::fs::remove_file(&temp).await;
    result
}

async fn update_from(app: &AppHandle, id: &str, link: &Link, pack_path: &Path, release: Option<Release>) -> R<Outcome> {
    let parts = (!link.parts.is_empty()).then(|| link.parts.clone());
    let prepared = crate::mrpack::prepare(app, pack_path, parts.as_deref()).await?;
    let mut inst = instance::get(id).await?;
    let game_changed = (inst.mc_version != prepared.mc || inst.loader != prepared.loader || (!prepared.loader_version.is_empty() && inst.loader_version != prepared.loader_version))
        .then(|| format!("Minecraft {} · {} {}", prepared.mc, crate::forge::display(&prepared.loader), prepared.loader_version).trim_end().to_string());
    crate::snapshots::take(id).await;
    let source = match release {
        Some(r) => Source { version_id: r.version_id, version: r.version, ..link.source.clone() },
        None => {
            let bytes = tokio::fs::read(pack_path).await.context("讀取模組包失敗")?;
            identify(&bytes, &prepared.name).await.unwrap_or_else(|| Source { name: prepared.name.clone(), ..Default::default() })
        }
    };
    let game = paths::game_dir(id);
    let old = link.files.clone();
    let files = prepared.files;
    let plan = tokio::task::spawn_blocking(move || plan(&game, &old, files)).await.context("比對模組包中斷")?;
    let label = format!("更新模組包「{}」前的舊檔案", if source.name.is_empty() { &prepared.name } else { &source.name });
    let summary = Outcome { version: source.version.clone(), skipped: prepared.skipped, blocked: prepared.blocked, game_changed: game_changed.clone(), ..clone_lists(&plan.outcome) };
    let entries = apply(app, id, plan, &label).await?;
    if game_changed.is_some() {
        inst.mc_version = prepared.mc;
        inst.loader = prepared.loader;
        inst.loader_version = prepared.loader_version;
        crate::commands::update_instance(inst).await?;
    }
    save(id, &Link { source, parts: link.parts.clone(), files: entries }).await?;
    Ok(summary)
}

fn clone_lists(o: &Outcome) -> Outcome {
    Outcome { added: o.added.clone(), updated: o.updated.clone(), removed: o.removed.clone(), kept: o.kept.clone(), ..Default::default() }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(game: &Path, rel: &str, bytes: &[u8]) -> String {
        let path = game.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, bytes).unwrap();
        net::sha1_hex(bytes)
    }

    fn bytes(rel: &str, data: &[u8]) -> NewFile {
        NewFile { rel: rel.into(), sha1: net::sha1_hex(data), content: Content::Bytes(data.to_vec()) }
    }

    #[test]
    fn update_plan_touches_only_what_the_pack_owns() {
        let game = std::env::temp_dir().join(format!("bl-pack-plan-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&game);
        let old = vec![
            Entry { path: "mods/sodium-1.jar".into(), sha1: write(&game, "mods/sodium-1.jar", b"s1"), removed: false },
            Entry { path: "mods/dropped.jar".into(), sha1: write(&game, "mods/dropped.jar", b"d"), removed: false },
            Entry { path: "mods/same.jar".into(), sha1: write(&game, "mods/same.jar.disabled", b"same"), removed: false },
            Entry { path: "mods/off.jar".into(), sha1: write(&game, "mods/off.jar.disabled", b"off1"), removed: false },
            Entry { path: "mods/gone.jar".into(), sha1: net::sha1_hex(b"g"), removed: true },
            Entry { path: "config/a.toml".into(), sha1: net::sha1_hex(b"a1"), removed: false },
            Entry { path: "config/b.toml".into(), sha1: write(&game, "config/b.toml", b"b1"), removed: false },
        ];
        write(&game, "config/a.toml", b"a-edited-by-player");
        write(&game, "mods/mine.jar", b"player");
        let new_files = vec![
            bytes("mods/sodium-2.jar", b"s2"),
            bytes("mods/same.jar", b"same"),
            bytes("mods/off.jar", b"off2"),
            bytes("mods/gone.jar", b"g"),
            bytes("mods/fresh.jar", b"f"),
            bytes("config/a.toml", b"a2"),
            bytes("config/b.toml", b"b2"),
        ];
        let plan = plan(&game, &old, new_files);
        let mut discard = plan.discard.clone();
        discard.sort();
        assert_eq!(discard, vec!["config/b.toml", "mods/dropped.jar", "mods/off.jar.disabled", "mods/sodium-1.jar"]);
        let mut targets: Vec<&str> = plan.fetch.iter().map(|(t, _)| t.as_str()).collect();
        targets.sort();
        assert_eq!(targets, vec!["config/b.toml", "mods/fresh.jar", "mods/off.jar.disabled", "mods/sodium-2.jar"]);
        assert_eq!(plan.outcome.kept, vec!["config/a.toml"]);
        assert!(plan.entries.iter().any(|e| e.path == "mods/gone.jar" && e.removed));
        assert!(plan.entries.iter().any(|e| e.path == "config/a.toml" && e.sha1 == net::sha1_hex(b"a1")));
        assert!(!plan.discard.iter().any(|d| d.contains("mine.jar")));
        let _ = std::fs::remove_dir_all(&game);
    }
}

#[cfg(test)]
mod choose_tests {
    use super::*;

    fn c(id: &str, mc: &str, stable: bool) -> Candidate {
        Candidate { mc: vec![mc.into()], loaders: vec!["fabric".into()], stable, release: Release { version_id: id.into(), version: id.into(), ..Default::default() } }
    }

    fn ids(check: &Check) -> Vec<(String, String)> {
        check.choices.iter().map(|r| (r.version_id.clone(), r.mc.clone())).collect()
    }

    #[test]
    fn offers_same_line_first_then_newer_minecraft_lines() {
        let list = || vec![c("15b", "26.3", false), c("14.1", "26.2", true), c("14.0", "26.2", true), c("13.6", "26.1.2", true), c("12.3", "1.21.11", true), c("12.2", "1.21.11", true), c("12.1", "1.21.11", true)];
        let old = choose(list(), "12.1", "1.21.11", "fabric");
        assert_eq!(ids(&old), vec![("12.3".into(), "1.21.11".into()), ("14.1".into(), "26.2".into()), ("13.6".into(), "26.1.2".into())]);
        let latest_on_line = choose(list(), "12.3", "1.21.11", "fabric");
        assert_eq!(ids(&latest_on_line), vec![("14.1".into(), "26.2".into()), ("13.6".into(), "26.1.2".into())]);
        assert!(choose(list(), "14.1", "26.2", "fabric").choices.is_empty());
        assert!(choose(list(), "12.1", "1.21.11", "neoforge").choices.is_empty());
        assert!(choose(vec![c("2b", "26.3", false), c("1", "26.3", true)], "1", "26.3", "fabric").choices.is_empty());
        assert_eq!(ids(&choose(vec![c("3b", "26.3", false), c("2b", "26.3", false)], "2b", "26.3", "fabric")), vec![("3b".into(), "26.3".into())]);
        assert_eq!(ids(&choose(vec![c("3", "26.3", true), c("2b", "26.3", false)], "2b", "26.3", "fabric")), vec![("3".into(), "26.3".into())]);
        assert_eq!(ids(&choose(vec![c("3b", "26.3", false), c("2", "26.3", true), c("1", "26.3", true)], "1", "26.3", "fabric")), vec![("2".into(), "26.3".into())]);
    }
}

#[cfg(test)]
mod live {
    use super::*;

    #[tokio::test]
    #[ignore = "連線測試：cargo test --lib real_packs_are_identified -- --ignored --nocapture"]
    async fn real_packs_are_identified() {
        for (platform, project) in [("modrinth", "1KVo5zza"), ("curseforge", "cf-396246")] {
            let source = Source { platform: platform.into(), project: project.into(), version_id: "none".into(), ..Default::default() };
            for mc in ["1.21.11", "26.2", "26.3"] {
                let found = check(&source, mc, "fabric").await.unwrap();
                println!("{platform} {mc}：{:?}", found.choices.iter().map(|r| (&r.version, &r.mc, !r.url.is_empty())).collect::<Vec<_>>());
            }
        }
        let mr = check(&Source { platform: "modrinth".into(), project: "1KVo5zza".into(), version_id: "none".into(), ..Default::default() }, "26.2", "fabric").await.unwrap().choices.remove(0);
        let bytes = net::client().get(&mr.url).send().await.unwrap().bytes().await.unwrap();
        let found = identify(&bytes, "Fabulously Optimized").await.unwrap();
        assert_eq!((found.platform.as_str(), found.version_id.as_str()), ("modrinth", mr.version_id.as_str()));
        let cf = check(&Source { platform: "curseforge".into(), project: "cf-396246".into(), version_id: "none".into(), ..Default::default() }, "26.2", "fabric").await.unwrap().choices.remove(0);
        let bytes = net::client().get(&cf.url).send().await.unwrap().bytes().await.unwrap();
        let found = identify(&bytes, "Fabulously Optimized").await.unwrap();
        assert_eq!((found.platform.as_str(), found.version_id.as_str()), ("curseforge", cf.version_id.as_str()));
        println!("兩個平台的 26.2 版本都能辨識回來");
    }
}
