use crate::config::Settings;
use crate::err::R;
use crate::instance::Instance;
use crate::modrinth::{self, ModMeta, Kind, Update, VersionInfo};
use crate::net::{self, DlFile};
use crate::paths;
use crate::state::AppState;
use anyhow::Context;
use serde::Serialize;
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
    project: Option<String>,
    version_id: Option<String>,
    fixed: bool,
    original: Pick,
    offered: Option<Pick>,
    chosen: Pick,
    blame: Option<usize>,
    versions: Option<Vec<VersionInfo>>,
}

#[derive(Serialize)]
pub struct Resolved {
    pub fixed: bool,
    pub notes: Vec<String>,
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

async fn settle(app: &AppHandle, slots: &mut [Slot], loader: &str, mc: &str, channel: &str) -> bool {
    let mut budget = BUDGET;
    for _ in 0..ROUNDS {
        let Some(&(owner, target)) = clashes(slots).first() else { return true };
        let mut best: Option<(usize, u8, usize, Pick)> = None;
        for s in [target, owner] {
            if slots[s].fixed {
                continue;
            }
            let mut tries = 0;
            for mut pick in candidates(&mut slots[s], loader, mc, channel).await {
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

fn describe(slots: &[Slot], updated: usize, fixed: bool) -> Vec<String> {
    let mut notes = Vec::new();
    if updated > 0 {
        notes.push(format!("啟動前已自動更新 {updated} 個模組"));
    }
    for slot in slots {
        let Some(other) = slot.blame.map(|b| &slots[b].name) else { continue };
        let version = slot.chosen.meta.as_ref().map(|m| m.version.clone()).unwrap_or_default();
        let note = match (&slot.offered, slot.chosen.rank) {
            (Some(offered), 0) => format!("「{}」先不更新：新版 {} 跟「{other}」不相容", slot.name, offered.meta.as_ref().map(|m| m.version.as_str()).unwrap_or("")),
            (_, 0) => continue,
            (_, 1) => format!("「{}」更新到 {version}，才能跟「{other}」一起使用", slot.name),
            (_, 2) => format!("「{}」改用測試版 {version}，才能跟「{other}」一起使用", slot.name),
            (_, 3) => format!("「{}」改回舊版 {version}，才能跟「{other}」一起使用", slot.name),
            _ => format!("「{}」改回舊的測試版 {version}，才能跟「{other}」一起使用", slot.name),
        };
        notes.push(note);
    }
    if !fixed {
        for (owner, target) in clashes(slots) {
            notes.push(format!("「{}」跟「{}」不相容，找不到能一起使用的版本，請到「模組與資源」調整", slots[owner].name, slots[target].name));
        }
    }
    notes
}

pub async fn run(app: &AppHandle, instance: &Instance, settings: &Settings, updates: bool) -> R<Resolved> {
    let scope = instance.scope(settings, &instance.mc_version, false);
    let dir = Kind::Mod.dir(&instance.id);
    let mut slots = Vec::new();
    for entry in modrinth::installed(&instance.id, Kind::Mod, &scope).await? {
        if !entry.enabled {
            continue;
        }
        let Some(found) = modrinth::digest(&dir.join(&entry.file)).await else { continue };
        let original = Pick { update: None, sha1: found.sha1, meta: found.meta, rank: 0 };
        let fixed = entry.managed || entry.pinned;
        let mut offered = None;
        if let Some(update) = entry.update.filter(|_| updates && !fixed) {
            if let Some(meta) = fetch(app, &update).await {
                offered = Some(Pick { sha1: update.sha1.clone(), update: Some(update), meta: Some(meta), rank: 1 });
            }
        }
        slots.push(Slot {
            chosen: offered.clone().unwrap_or_else(|| original.clone()),
            file: entry.file,
            name: entry.name,
            project: entry.project_id,
            version_id: entry.version_id,
            fixed,
            original,
            offered,
            blame: None,
            versions: None,
        });
    }
    let fixed = settle(app, &mut slots, &instance.loader, &instance.mc_version, scope.channel).await;
    if !fixed {
        let stuck: Vec<usize> = clashes(&slots).into_iter().flat_map(|(a, b)| [a, b]).collect();
        for i in stuck {
            let slot = &mut slots[i];
            if slot.chosen.sha1 != slot.original.sha1 {
                slot.chosen = slot.original.clone();
                slot.blame = None;
            }
        }
    }
    let changed: Vec<&Slot> = slots.iter().filter(|s| s.chosen.sha1 != s.original.sha1).collect();
    if !changed.is_empty() {
        crate::snapshots::take(&instance.id).await;
    }
    let mut updated = 0;
    for slot in changed {
        let Some(update) = &slot.chosen.update else { continue };
        let target = dir.join(&update.filename);
        tokio::fs::copy(cached(update), &target).await.with_context(|| format!("寫入模組失敗：{}", update.filename))?;
        if slot.file != update.filename {
            paths::discard(&format!("舊版 {}", slot.file), &[(dir.join(&slot.file), slot.file.as_str())]).with_context(|| format!("移除舊版失敗：{}", slot.file))?;
        }
        crate::modpack::note_replaced(&instance.id, Kind::Mod.folder(), &slot.file, &update.filename, &update.sha1).await;
        if slot.blame.is_none() {
            updated += 1;
        }
    }
    let _ = tokio::fs::remove_dir_all(cache_dir()).await;
    Ok(Resolved { notes: describe(&slots, updated, fixed), fixed })
}

#[tauri::command]
pub async fn resolve_mods(app: AppHandle, state: State<'_, AppState>, id: String) -> R<Resolved> {
    state.ensure_idle(&id).await?;
    let instance = crate::instance::get(&id).await?;
    let settings = crate::config::load_settings().await;
    run(&app, &instance, &settings, false).await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn slot(id: &str, version: &str, breaks: &[(&str, &str)]) -> Slot {
        let meta = ModMeta { id: id.into(), version: version.into(), breaks: breaks.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect(), ..Default::default() };
        let pick = Pick { update: None, sha1: format!("{id}{version}"), meta: Some(meta), rank: 0 };
        Slot { file: id.into(), name: id.into(), project: None, version_id: None, fixed: false, original: pick.clone(), offered: None, chosen: pick, blame: None, versions: None }
    }

    #[test]
    fn finds_breaks_and_holds_back_updates() {
        let mut slots = vec![slot("sodium", "0.9.2+mc26.3", &[("iris", "<=1.11.2")]), slot("iris", "1.11.7+mc26.3", &[])];
        assert!(clashes(&slots).is_empty());
        let newer = slot("sodium", "0.9.3+mc26.3", &[("iris", "<=1.12.0"), ("moreculling", "<1.9.2")]).chosen;
        slots[0].offered = Some(Pick { rank: 1, ..newer.clone() });
        slots[0].chosen = Pick { rank: 1, ..newer };
        assert_eq!(clashes(&slots), vec![(0, 1)]);
        let rt = tokio::runtime::Builder::new_current_thread().build().unwrap();
        let picks = rt.block_on(candidates(&mut slots[0], "fabric", "26.3", "release"));
        assert_eq!(picks.len(), 1);
        assert_eq!(picks[0].rank, 0);
        slots[0].chosen = picks[0].clone();
        slots[0].blame = Some(1);
        assert!(clashes(&slots).is_empty());
        assert_eq!(describe(&slots, 0, true), vec!["「sodium」先不更新：新版 0.9.3+mc26.3 跟「iris」不相容"]);
    }
}
