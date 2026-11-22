use crate::bail;
use crate::config::Settings;
use crate::err::R;
use crate::instance::Instance;
use crate::modrinth::{self, Kind, ModMeta, Update, VersionInfo};
use crate::net::{self, DlFile};
use crate::paths;
use crate::state::AppState;
use anyhow::Context;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use tauri::{AppHandle, State};

const ROUNDS: usize = 16;
const TRIES: usize = 6;
const BUDGET: usize = 24;

#[derive(Clone)]
struct Pick {
    update: Option<Update>,
    sha1: String,
    meta: Option<ModMeta>,
    rank: u8,
}

struct Slot {
    file: String,
    name: String,
    version: String,
    project: Option<String>,
    version_id: Option<String>,
    fixed: bool,
    original: Pick,
    offered: Option<Pick>,
    chosen: Pick,
    blame: Option<usize>,
    versions: Option<Vec<VersionInfo>>,
}

#[derive(Serialize, Deserialize, Clone, Default, Debug)]
#[serde(default, rename_all = "camelCase")]
pub struct Change {
    pub file: String,
    pub name: String,
    pub from: String,
    pub to: String,
    pub latest: String,
    pub kind: String,
    pub because: String,
    pub note: String,
    pub update: Option<Update>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Plan {
    pub changes: Vec<Change>,
    pub conflicts: Vec<String>,
    pub fixed: bool,
}

#[derive(Serialize)]
pub struct Resolved {
    pub fixed: bool,
    pub notes: Vec<String>,
}

fn allowed(mode: &str, rank: u8) -> bool {
    match mode {
        "off" => false,
        "keep" => rank == 0,
        "stable" => rank != 2 && rank != 4,
        _ => true,
    }
}

fn clashes(slots: &[Slot]) -> Vec<(usize, usize)> {
    let mut owner: HashMap<&str, usize> = HashMap::new();
    for (i, slot) in slots.iter().enumerate() {
        if let Some(meta) = &slot.chosen.meta {
            for id in std::iter::once(&meta.id).chain(&meta.provides) {
                owner.entry(id.as_str()).or_insert(i);
            }
        }
    }
    let mut out = Vec::new();
    for (i, slot) in slots.iter().enumerate() {
        let Some(meta) = &slot.chosen.meta else { continue };
        for (rules, bad) in [(&meta.depends, false), (&meta.breaks, true)] {
            for (id, req) in rules {
                let Some(&t) = owner.get(id.as_str()).filter(|t| **t != i) else { continue };
                let Some(target) = &slots[t].chosen.meta else { continue };
                if meta.accepts(req, &target.version) == Some(bad) && !out.contains(&(i, t)) {
                    out.push((i, t));
                }
            }
        }
    }
    out
}

fn cache_dir() -> PathBuf {
    paths::root().join("cache").join("mod-candidates")
}

fn cached(update: &Update) -> PathBuf {
    cache_dir().join(format!("{}.jar", update.sha1.to_ascii_lowercase()))
}

async fn fetch(app: &AppHandle, update: &Update) -> Option<ModMeta> {
    if !modrinth::trusted(update, Kind::Mod) {
        return None;
    }
    let path = cached(update);
    let dl = DlFile::new(update.url.clone(), path.clone()).sha1(Some(update.sha1.clone())).size(update.size);
    net::download_all(app, "mods", vec![dl], false).await.ok()?;
    modrinth::digest(&path).await?.meta
}

async fn candidates(slot: &mut Slot, loader: &str, mc: &str, channel: &str) -> Vec<Pick> {
    let mut out = Vec::new();
    if slot.chosen.sha1 != slot.original.sha1 {
        out.push(slot.original.clone());
    }
    let Some(project) = slot.project.clone() else { return out };
    if slot.versions.is_none() {
        let list = if crate::curseforge::is_cf(&project) {
            crate::curseforge::versions(Kind::Mod, loader, &project, Some(mc)).await
        } else {
            modrinth::versions(Kind::Mod, loader, &project, Some(mc)).await
        };
        slot.versions = Some(list.unwrap_or_default());
    }
    let list = slot.versions.as_deref().unwrap_or_default();
    let current = list.iter().position(|v| slot.version_id.as_deref() == Some(v.id.as_str()));
    let mut found: Vec<Pick> = list
        .iter()
        .enumerate()
        .filter_map(|(i, v)| {
            let file = v.file.clone()?;
            let older = current.is_some_and(|c| i > c);
            let rank = 1 + 2 * older as u8 + !modrinth::channel_allows(channel, &v.version_type) as u8;
            (Some(i) != current && file.sha1 != slot.chosen.sha1).then(|| Pick { sha1: file.sha1.clone(), update: Some(file), meta: None, rank })
        })
        .collect();
    found.sort_by_key(|p| p.rank);
    out.extend(found);
    out
}

async fn settle(app: &AppHandle, slots: &mut [Slot], loader: &str, mc: &str, channel: &str, mode: &str) -> bool {
    let mut budget = BUDGET;
    for _ in 0..ROUNDS {
        let Some(&(owner, target)) = clashes(slots).first() else { return true };
        let mut best: Option<(usize, u8, usize, Pick)> = None;
        for s in [target, owner] {
            if slots[s].fixed {
                continue;
            }
            let mut tries = 0;
            for mut pick in candidates(&mut slots[s], loader, mc, channel).await.into_iter().filter(|p| allowed(mode, p.rank)) {
                if let Some(update) = pick.update.clone().filter(|_| pick.meta.is_none()) {
                    if tries == TRIES || budget == 0 {
                        break;
                    }
                    tries += 1;
                    budget -= 1;
                    pick.meta = fetch(app, &update).await;
                    if pick.meta.is_none() {
                        continue;
                    }
                }
                let kept = std::mem::replace(&mut slots[s].chosen, pick.clone());
                let after = clashes(slots);
                slots[s].chosen = kept;
                if after.contains(&(owner, target)) {
                    continue;
                }
                if best.as_ref().is_none_or(|b| (after.len(), pick.rank) < (b.0, b.1)) {
                    best = Some((after.len(), pick.rank, s, pick));
                }
                break;
            }
        }
        let Some((_, _, s, pick)) = best else { return false };
        slots[s].blame = Some(if s == owner { target } else { owner });
        slots[s].chosen = pick;
    }
    clashes(slots).is_empty()
}

fn label(pick: &Pick) -> String {
    pick.meta.as_ref().map(|m| m.version.clone()).filter(|v| !v.is_empty()).or_else(|| pick.update.as_ref().map(|u| u.version.clone())).unwrap_or_default()
}

fn changes(slots: &[Slot]) -> Vec<Change> {
    slots
        .iter()
        .filter_map(|slot| {
            let same = slot.chosen.sha1 == slot.original.sha1;
            if same && slot.offered.is_none() {
                return None;
            }
            let kind = if same {
                "hold"
            } else if slot.offered.as_ref().is_some_and(|o| o.sha1 == slot.chosen.sha1) {
                "update"
            } else {
                ["", "newer", "beta", "older", "olderBeta"][slot.chosen.rank.clamp(1, 4) as usize]
            };
            let from = label(&slot.original);
            let from = if from.is_empty() { slot.version.clone() } else { from };
            let latest = slot.offered.as_ref().map(label).unwrap_or_default();
            let to = if same { from.clone() } else { label(&slot.chosen) };
            let because = slot.blame.map(|b| slots[b].name.clone()).unwrap_or_default();
            let name = &slot.name;
            let note = match kind {
                "update" => String::new(),
                "hold" if because.is_empty() => format!("「{name}」先不更新：新版 {latest} 會造成模組不相容"),
                "hold" => format!("「{name}」先不更新：新版 {latest} 跟「{because}」不相容"),
                "newer" => format!("「{name}」更新到 {to}，才能跟「{because}」一起使用"),
                "beta" => format!("「{name}」改用測試版 {to}，才能跟「{because}」一起使用"),
                "older" => format!("「{name}」改回舊版 {to}，才能跟「{because}」一起使用"),
                _ => format!("「{name}」改回舊的測試版 {to}，才能跟「{because}」一起使用"),
            };
            Some(Change {
                file: slot.file.clone(),
                name: name.clone(),
                from,
                to,
                latest,
                kind: kind.into(),
                because,
                note,
                update: if same { None } else { slot.chosen.update.clone() },
            })
        })
        .collect()
}

pub async fn plan(app: &AppHandle, instance: &Instance, settings: &Settings, updates: bool, only: &[String]) -> R<Plan> {
    let scope = instance.scope(settings, &instance.mc_version, false);
    let dir = Kind::Mod.dir(&instance.id);
    let mode = settings.mod_resolve.as_str();
    let _ = tokio::fs::remove_dir_all(cache_dir()).await;
    let mut slots = Vec::new();
    for entry in modrinth::installed(&instance.id, Kind::Mod, &scope).await? {
        if !entry.enabled {
            continue;
        }
        let Some(found) = modrinth::digest(&dir.join(&entry.file)).await else { continue };
        let original = Pick { update: None, sha1: found.sha1, meta: found.meta, rank: 0 };
        let fixed = entry.managed || entry.pinned;
        let mut offered = None;
        if let Some(update) = entry.update.filter(|_| updates && !fixed && (only.is_empty() || only.contains(&entry.file))) {
            let meta = if mode == "off" { None } else { fetch(app, &update).await };
            if mode == "off" || meta.is_some() {
                offered = Some(Pick { sha1: update.sha1.clone(), update: Some(update), meta, rank: 1 });
            }
        }
        slots.push(Slot {
            chosen: offered.clone().unwrap_or_else(|| original.clone()),
            file: entry.file,
            name: entry.name,
            version: entry.version.unwrap_or_default(),
            project: entry.project_id,
            version_id: entry.version_id,
            fixed,
            original,
            offered,
            blame: None,
            versions: None,
        });
    }
    let fixed = mode == "off" || settle(app, &mut slots, &instance.loader, &instance.mc_version, scope.channel, mode).await;
    let mut conflicts = Vec::new();
    if !fixed {
        for (a, b) in clashes(&slots) {
            for (i, other) in [(a, b), (b, a)] {
                if slots[i].chosen.sha1 != slots[i].original.sha1 {
                    slots[i].chosen = slots[i].original.clone();
                    slots[i].blame = Some(other);
                }
            }
        }
        for (a, b) in clashes(&slots) {
            conflicts.push(format!("「{}」跟「{}」不相容，找不到能一起使用的版本，請到「模組與資源」調整", slots[a].name, slots[b].name));
        }
    }
    Ok(Plan { changes: changes(&slots), fixed: conflicts.is_empty(), conflicts })
}

pub async fn apply(app: &AppHandle, instance_id: &str, changes: &[Change]) -> R<usize> {
    let todo: Vec<(&Change, &Update)> = changes.iter().filter_map(|c| Some((c, c.update.as_ref()?))).collect();
    if todo.iter().any(|(c, u)| !modrinth::safe_file(&c.file, Kind::Mod) || !modrinth::trusted(u, Kind::Mod)) {
        bail!("更新資訊不正確");
    }
    if !todo.is_empty() {
        crate::snapshots::take(instance_id).await;
    }
    let dir = Kind::Mod.dir(instance_id);
    for (change, update) in &todo {
        let cache = cached(update);
        let target = if cache.is_file() {
            tokio::fs::copy(&cache, dir.join(&update.filename)).await.with_context(|| format!("寫入模組失敗：{}", update.filename))?;
            if change.file != update.filename {
                paths::discard(&format!("舊版 {}", change.file), &[(dir.join(&change.file), change.file.as_str())]).with_context(|| format!("移除舊版失敗：{}", change.file))?;
            }
            update.filename.clone()
        } else {
            modrinth::replace(app, instance_id, Kind::Mod, &change.file, (*update).clone()).await?
        };
        crate::modpack::note_replaced(instance_id, Kind::Mod.folder(), &change.file, &target, &update.sha1).await;
    }
    let _ = tokio::fs::remove_dir_all(cache_dir()).await;
    Ok(todo.len())
}

fn notes(plan: &Plan) -> Vec<String> {
    let updated = plan.changes.iter().filter(|c| c.kind == "update").count();
    let mut out = Vec::new();
    if updated > 0 {
        out.push(format!("啟動前已自動更新 {updated} 個模組"));
    }
    out.extend(plan.changes.iter().filter(|c| !c.note.is_empty()).map(|c| c.note.clone()));
    out.extend(plan.conflicts.iter().cloned());
    out
}

async fn local_clash(instance_id: &str) -> bool {
    let mut slots = Vec::new();
    let Ok(mut entries) = tokio::fs::read_dir(Kind::Mod.dir(instance_id)).await else { return false };
    while let Ok(Some(entry)) = entries.next_entry().await {
        let file = entry.file_name().to_string_lossy().to_string();
        if !file.ends_with(".jar") {
            continue;
        }
        let Some(found) = modrinth::digest(&entry.path()).await else { continue };
        let pick = Pick { update: None, sha1: found.sha1, meta: found.meta, rank: 0 };
        slots.push(Slot { file: file.clone(), name: file, version: String::new(), project: None, version_id: None, fixed: true, original: pick.clone(), offered: None, chosen: pick, blame: None, versions: None });
    }
    !clashes(&slots).is_empty()
}

pub async fn run(app: &AppHandle, instance: &Instance, settings: &Settings, updates: bool) -> R<Resolved> {
    if !updates && !local_clash(&instance.id).await {
        return Ok(Resolved { fixed: true, notes: Vec::new() });
    }
    let plan = plan(app, instance, settings, updates, &[]).await?;
    apply(app, &instance.id, &plan.changes).await?;
    Ok(Resolved { notes: notes(&plan), fixed: plan.fixed })
}

#[tauri::command]
pub async fn resolve_mods(app: AppHandle, state: State<'_, AppState>, id: String) -> R<Resolved> {
    state.ensure_idle(&id).await?;
    let instance = crate::instance::get(&id).await?;
    let settings = crate::config::load_settings().await;
    run(&app, &instance, &settings, false).await
}

#[tauri::command]
pub async fn plan_mod_updates(app: AppHandle, id: String, files: Vec<String>) -> R<Plan> {
    let instance = crate::instance::get(&id).await?;
    let settings = crate::config::load_settings().await;
    plan(&app, &instance, &settings, true, &files).await
}

#[tauri::command]
pub async fn apply_mod_changes(app: AppHandle, state: State<'_, AppState>, id: String, changes: Vec<Change>) -> R<usize> {
    state.ensure_idle(&id).await?;
    crate::instance::get(&id).await?;
    apply(&app, &id, &changes).await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn slot(id: &str, version: &str, breaks: &[(&str, &str)]) -> Slot {
        let meta = ModMeta { id: id.into(), version: version.into(), breaks: breaks.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect(), ..Default::default() };
        let pick = Pick { update: None, sha1: format!("{id}{version}"), meta: Some(meta), rank: 0 };
        Slot { file: id.into(), name: id.into(), version: version.into(), project: None, version_id: None, fixed: false, original: pick.clone(), offered: None, chosen: pick, blame: None, versions: None }
    }

    #[test]
    fn finds_breaks_and_holds_back_updates() {
        let mut slots = vec![slot("sodium", "0.9.2+mc26.3", &[("iris", "<=1.11.2")]), slot("iris", "1.11.7+mc26.3", &[])];
        assert!(clashes(&slots).is_empty());
        let newer = slot("sodium", "0.9.3+mc26.3", &[("iris", "<=1.12.0"), ("moreculling", "<1.9.2")]).chosen;
        slots[0].offered = Some(Pick { rank: 1, ..newer.clone() });
        slots[0].chosen = Pick { rank: 1, ..newer };
        assert_eq!(clashes(&slots), vec![(0, 1)]);
        assert_eq!(changes(&slots)[0].kind, "update");
        let rt = tokio::runtime::Builder::new_current_thread().build().unwrap();
        let picks = rt.block_on(candidates(&mut slots[0], "fabric", "26.3", "release"));
        assert_eq!(picks.len(), 1);
        assert_eq!(picks[0].rank, 0);
        slots[0].chosen = picks[0].clone();
        slots[0].blame = Some(1);
        assert!(clashes(&slots).is_empty());
        let plan = changes(&slots);
        assert_eq!((plan[0].kind.as_str(), plan[0].from.as_str(), plan[0].latest.as_str()), ("hold", "0.9.2+mc26.3", "0.9.3+mc26.3"));
        assert_eq!(plan[0].note, "「sodium」先不更新：新版 0.9.3+mc26.3 跟「iris」不相容");
        assert!(plan[0].update.is_none());
    }

    #[test]
    fn modes_limit_choices() {
        assert!(allowed("auto", 4) && allowed("stable", 3) && !allowed("stable", 2) && !allowed("stable", 4));
        assert!(allowed("keep", 0) && !allowed("keep", 1) && !allowed("off", 0));
    }
}
