use crate::err::R;
use crate::{bail, paths};
use anyhow::Context;
use serde::Serialize;
use std::io::Read;
use std::path::{Path, PathBuf};

const KEEP_DIRS: &[&str] = &[
    "mods", "config", "saves", "resourcepacks", "shaderpacks", "screenshots", "logs", "crash-reports", "versions", "libraries",
    "assets", "natives", "backups", "trash", "defaultconfigs", "kubejs", "datapacks", "schematics", "server-resource-packs",
];

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Leftover {
    pub path: String,
    pub size: u64,
    pub dir: bool,
    pub children: Vec<Node>,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Node {
    pub name: String,
    pub size: u64,
    pub dir: bool,
    pub children: Vec<Node>,
}

const TREE_LIMIT: usize = 200;

fn tree(path: &Path, budget: &mut usize) -> Vec<Node> {
    let mut entries: Vec<_> = std::fs::read_dir(path).map(|d| d.flatten().collect()).unwrap_or_default();
    entries.sort_by_key(|e| (!e.path().is_dir(), e.file_name()));
    let mut out = Vec::new();
    for entry in entries {
        if *budget == 0 {
            break;
        }
        *budget -= 1;
        let p = entry.path();
        let dir = p.is_dir();
        out.push(Node { name: entry.file_name().to_string_lossy().to_string(), size: size_of(&p), dir, children: if dir { tree(&p, budget) } else { Vec::new() } });
    }
    out
}

fn leftover(path: String, full: &Path) -> Leftover {
    let dir = full.is_dir();
    let mut budget = TREE_LIMIT;
    Leftover { path, size: size_of(full), dir, children: if dir { tree(full, &mut budget) } else { Vec::new() } }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Leftovers {
    pub mod_id: String,
    pub items: Vec<Leftover>,
}

pub fn mod_id(jar: &Path) -> Option<String> {
    let mut zip = zip::ZipArchive::new(std::fs::File::open(jar).ok()?).ok()?;
    let mut raw = String::new();
    zip.by_name("fabric.mod.json").ok()?.read_to_string(&mut raw).ok()?;
    let clean: String = raw.chars().map(|c| if c.is_control() { ' ' } else { c }).collect();
    let json: serde_json::Value = serde_json::from_str(&clean).ok()?;
    json.get("id")?.as_str().map(|s| s.to_ascii_lowercase())
}

fn installed_ids(game_dir: &Path) -> Vec<String> {
    std::fs::read_dir(game_dir.join("mods"))
        .map(|d| d.flatten().map(|e| e.path()).collect::<Vec<_>>())
        .unwrap_or_default()
        .iter()
        .filter(|p| { let n = p.to_string_lossy(); n.ends_with(".jar") || n.ends_with(".jar.disabled") })
        .filter_map(|p| mod_id(p))
        .collect()
}

fn variants(id: &str) -> Vec<String> {
    let mut out = vec![id.to_string(), id.replace('_', "-"), id.replace('-', "_")];
    out.sort();
    out.dedup();
    out
}

fn match_len(name: &str, id: &str) -> usize {
    variants(id)
        .into_iter()
        .filter(|v| {
            name.strip_prefix(v.as_str())
                .is_some_and(|rest| rest.is_empty() || rest.starts_with(['-', '_', '.', ' ']))
        })
        .map(|v| v.len())
        .max()
        .unwrap_or(0)
}

fn owned_by(name: &str, target: &str, others: &[String]) -> bool {
    let name = name.to_ascii_lowercase();
    let mine = match_len(&name, target);
    mine > 0 && others.iter().filter(|o| o.as_str() != target).all(|o| match_len(&name, o) <= mine)
}

fn size_of(path: &Path) -> u64 {
    if path.is_dir() {
        std::fs::read_dir(path).map(|d| d.flatten().map(|e| size_of(&e.path())).sum()).unwrap_or(0)
    } else {
        std::fs::metadata(path).map(|m| m.len()).unwrap_or(0)
    }
}

pub fn find(game_dir: &Path, target: &str, others: &[String]) -> Vec<Leftover> {
    if target.len() < 3 || matches!(target, "minecraft" | "java" | "fabricloader" | "fabric") {
        return Vec::new();
    }
    let mut out = Vec::new();
    for entry in std::fs::read_dir(game_dir.join("config")).map(|d| d.flatten().collect::<Vec<_>>()).unwrap_or_default() {
        let name = entry.file_name().to_string_lossy().to_string();
        if owned_by(&name, target, others) {
            let path = entry.path();
            out.push(leftover(format!("config/{name}"), &path));
        }
    }
    for entry in std::fs::read_dir(game_dir).map(|d| d.flatten().collect::<Vec<_>>()).unwrap_or_default() {
        let name = entry.file_name().to_string_lossy().to_string();
        let lower = name.to_ascii_lowercase();
        if entry.path().is_dir() && !KEEP_DIRS.contains(&lower.as_str()) && variants(target).contains(&lower) {
            out.push(leftover(name, &entry.path()));
        }
    }
    out.sort_by(|a, b| a.path.cmp(&b.path));
    out
}

#[tauri::command]
pub async fn mod_leftovers(id: String, file: String) -> R<Leftovers> {
    let game_dir = paths::game_dir(&id);
    if file.contains(['/', '\\']) || file.contains("..") {
        bail!("檔名不正確");
    }
    let Some(target) = mod_id(&game_dir.join("mods").join(&file)) else {
        return Ok(Leftovers { mod_id: String::new(), items: Vec::new() });
    };
    let others = installed_ids(&game_dir);
    let items = find(&game_dir, &target, &others);
    Ok(Leftovers { mod_id: target, items })
}

#[tauri::command]
pub async fn remove_leftovers(state: tauri::State<'_, crate::state::AppState>, id: String, mod_id: String, paths_to_remove: Vec<String>) -> R<usize> {
    state.ensure_idle(&id).await?;
    let game_dir = paths::game_dir(&id);
    let allowed = find(&game_dir, &mod_id.to_ascii_lowercase(), &installed_ids(&game_dir));
    let items: Vec<(PathBuf, &str)> = paths_to_remove
        .iter()
        .filter(|p| allowed.iter().any(|a| &a.path == *p))
        .map(|rel| (game_dir.join(rel), rel.as_str()))
        .collect();
    if !items.is_empty() {
        paths::discard(&format!("模組殘留 {mod_id}"), &items).context("移到回收資料夾失敗")?;
    }
    Ok(items.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_only_files_owned_by_the_removed_mod() {
        let dir = std::env::temp_dir().join(format!("leftovers-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        for f in ["config/sodium-options.json", "config/sodium-mixins.properties", "config/sodium-extra-options.json", "config/iris.properties", "config/xaero/minimap.txt", "config/sodiumx.json", "xaero/data.txt", "saves/w/level.dat"] {
            let p = dir.join(f);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, "x").unwrap();
        }
        let others = vec!["sodium".to_string(), "sodium-extra".to_string(), "iris".to_string(), "xaero".to_string()];
        let paths = |id: &str| find(&dir, id, &others).into_iter().map(|l| l.path).collect::<Vec<_>>();
        assert_eq!(paths("sodium"), vec!["config/sodium-mixins.properties", "config/sodium-options.json"]);
        assert_eq!(paths("sodium-extra"), vec!["config/sodium-extra-options.json"]);
        assert_eq!(paths("xaero"), vec!["config/xaero", "xaero"]);
        assert!(paths("mc").is_empty());
        let xaero = find(&dir, "xaero", &others).into_iter().find(|l| l.path == "config/xaero").unwrap();
        assert_eq!(xaero.children.iter().map(|n| n.name.as_str()).collect::<Vec<_>>(), vec!["minimap.txt"]);
        assert_eq!(match_len("entity-model-features.json", "entity_model_features"), "entity_model_features".len());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
