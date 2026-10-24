use crate::err::R;
use crate::{bail, instance, paths};
use anyhow::Context;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

pub const PARTS: [&str; 8] = ["saves", "options.txt", "servers.dat", "resourcepacks", "shaderpacks", "screenshots", "mods", "config"];

static KNOWN: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Source {
    pub launcher: String,
    pub name: String,
    pub path: String,
    pub mc_version: String,
    pub loader: String,
    pub loader_version: String,
    pub saves: usize,
    pub mods: usize,
}

pub struct Roots {
    pub data: PathBuf,
    pub local: PathBuf,
    pub home: PathBuf,
}

impl Roots {
    fn system() -> Option<Self> {
        let home = dirs::home_dir()?;
        if std::env::var_os("FLATPAK_ID").is_some() {
            let data = home.join(".local/share");
            return Some(Self { data: data.clone(), local: data, home });
        }
        Some(Self { data: dirs::data_dir()?, local: dirs::data_local_dir()?, home })
    }
}

fn count(dir: &Path) -> usize {
    std::fs::read_dir(dir).map(|d| d.flatten().count()).unwrap_or(0)
}

fn source(launcher: &str, name: &str, game_dir: &Path, mc: &str, loader: &str, loader_version: &str) -> Option<Source> {
    if !game_dir.is_dir() {
        return None;
    }
    Some(Source {
        launcher: launcher.into(),
        name: name.into(),
        path: game_dir.to_string_lossy().to_string(),
        mc_version: mc.into(),
        loader: if loader.is_empty() { "vanilla".into() } else { loader.to_lowercase() },
        loader_version: loader_version.into(),
        saves: count(&game_dir.join("saves")),
        mods: count(&game_dir.join("mods")),
    })
}

fn read_json(path: &Path) -> Option<Value> {
    serde_json::from_slice(&std::fs::read(path).ok()?).ok()
}

fn text(v: &Value, pointer: &str) -> String {
    v.pointer(pointer).and_then(Value::as_str).unwrap_or("").to_string()
}

fn subdirs(dir: &Path) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = std::fs::read_dir(dir)
        .map(|d| d.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect())
        .unwrap_or_default();
    out.sort();
    out
}

fn cfg_value(cfg: &str, key: &str) -> Option<String> {
    cfg.lines().find_map(|l| l.strip_prefix(key)?.strip_prefix('=').map(|v| v.trim().to_string()))
}

pub fn parse_version_id(id: &str) -> (String, String, String) {
    if let Some(rest) = id.strip_prefix("fabric-loader-") {
        if let Some((lv, mc)) = rest.split_once('-') {
            return (mc.into(), "fabric".into(), lv.into());
        }
    }
    if let Some(rest) = id.strip_prefix("quilt-loader-") {
        if let Some((lv, mc)) = rest.split_once('-') {
            return (mc.into(), "quilt".into(), lv.into());
        }
    }
    if let Some((mc, rest)) = id.split_once("-forge-") {
        return (mc.into(), "forge".into(), rest.into());
    }
    if let Some(lv) = id.strip_prefix("neoforge-") {
        return (String::new(), "neoforge".into(), lv.into());
    }
    (id.into(), "vanilla".into(), String::new())
}

pub fn parse_mmc_pack(pack: &Value) -> (String, String, String) {
    let mut mc = String::new();
    let mut loader = ("vanilla".to_string(), String::new());
    for component in pack.get("components").and_then(Value::as_array).into_iter().flatten() {
        let version = component.get("version").and_then(Value::as_str).unwrap_or("").to_string();
        match component.get("uid").and_then(Value::as_str).unwrap_or("") {
            "net.minecraft" => mc = version,
            "net.fabricmc.fabric-loader" => loader = ("fabric".into(), version),
            "org.quiltmc.quilt-loader" => loader = ("quilt".into(), version),
            "net.minecraftforge" => loader = ("forge".into(), version),
            "net.neoforged" => loader = ("neoforge".into(), version),
            _ => {}
        }
    }
    (mc, loader.0, loader.1)
}

pub fn parse_curseforge(v: &Value) -> (String, String, String) {
    let mc = text(v, "/gameVersion");
    let name = text(v, "/baseModLoader/name");
    for loader in ["fabric", "quilt", "neoforge", "forge"] {
        if let Some(rest) = name.strip_prefix(&format!("{loader}-")) {
            return (mc, loader.into(), rest.split('-').next().unwrap_or("").to_string());
        }
    }
    (mc, "vanilla".into(), String::new())
}

pub fn parse_xmcl(v: &Value) -> (String, String, String) {
    let mc = text(v, "/runtime/minecraft");
    for (key, loader) in [("fabricLoader", "fabric"), ("quiltLoader", "quilt"), ("neoForged", "neoforge"), ("forge", "forge")] {
        let lv = text(v, &format!("/runtime/{key}"));
        if !lv.is_empty() {
            return (mc, loader.into(), lv);
        }
    }
    (mc, "vanilla".into(), String::new())
}

pub fn inspect_instance(dir: &Path) -> Option<Source> {
    let folder = dir.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    if let Ok(cfg) = std::fs::read_to_string(dir.join("instance.cfg")) {
        let name = cfg_value(&cfg, "name").unwrap_or(folder);
        let (mut mc, loader, lv) = read_json(&dir.join("mmc-pack.json")).map(|v| parse_mmc_pack(&v)).unwrap_or_default();
        if mc.is_empty() {
            mc = cfg_value(&cfg, "IntendedVersion").unwrap_or_default();
        }
        let game = if dir.join(".minecraft").is_dir() { dir.join(".minecraft") } else { dir.join("minecraft") };
        return source("Prism／MultiMC", &name, &game, &mc, &loader, &lv);
    }
    if let Some(v) = read_json(&dir.join("minecraftinstance.json")) {
        let (mc, loader, lv) = parse_curseforge(&v);
        let name = v.get("name").and_then(Value::as_str).map(str::to_string).unwrap_or(folder);
        return source("CurseForge", &name, dir, &mc, &loader, &lv);
    }
    if let Some(v) = read_json(&dir.join("instance.json")).filter(|v| v.get("runtime").is_some()) {
        let (mc, loader, lv) = parse_xmcl(&v);
        let name = v.get("name").and_then(Value::as_str).filter(|n| !n.is_empty()).map(str::to_string).unwrap_or(folder);
        return source("X Minecraft Launcher", &name, dir, &mc, &loader, &lv);
    }
    if let Some(v) = read_json(&dir.join("profile.json")).filter(|v| v.get("metadata").is_some()) {
        let name = v.pointer("/metadata/name").and_then(Value::as_str).map(str::to_string).unwrap_or(folder);
        return source("Modrinth App", &name, dir, &text(&v, "/metadata/game_version"), &text(&v, "/metadata/loader"), &text(&v, "/metadata/loader_version/id"));
    }
    if dir.join("saves").is_dir() || dir.join("options.txt").is_file() {
        return source("Minecraft 資料夾", &folder, dir, "", "vanilla", "");
    }
    None
}

fn official(r: &Roots) -> Vec<Source> {
    let dir = if cfg!(target_os = "linux") { r.home.join(".minecraft") } else if cfg!(target_os = "macos") { r.data.join("minecraft") } else { r.data.join(".minecraft") };
    let profiles: Vec<Value> = read_json(&dir.join("launcher_profiles.json"))
        .and_then(|v| v.get("profiles")?.as_object().map(|m| m.values().cloned().collect()))
        .unwrap_or_default();
    let mut latest = profiles
        .iter()
        .filter(|p| p.get("gameDir").is_none())
        .max_by_key(|p| text(p, "/lastUsed"))
        .map(|p| text(p, "/lastVersionId"))
        .unwrap_or_default();
    if latest.starts_with("latest-") {
        latest.clear();
    }
    let (mc, loader, lv) = parse_version_id(&latest);
    let mut out: Vec<Source> = source("官方啟動器", "官方啟動器 .minecraft", &dir, &mc, &loader, &lv).into_iter().collect();
    for p in profiles.iter().filter(|p| p.get("gameDir").is_some()) {
        let (mc, loader, lv) = parse_version_id(&text(p, "/lastVersionId"));
        let name = text(p, "/name");
        out.extend(source("官方啟動器", if name.is_empty() { "自訂資料夾" } else { &name }, Path::new(&text(p, "/gameDir")), &mc, &loader, &lv));
    }
    out
}

fn prism(r: &Roots) -> Vec<Source> {
    let mut out = Vec::new();
    for (base, cfg) in [
        (r.data.join("PrismLauncher"), "prismlauncher.cfg"),
        (r.home.join(".var/app/org.prismlauncher.PrismLauncher/data/PrismLauncher"), "prismlauncher.cfg"),
        (r.data.join("PolyMC"), "polymc.cfg"),
        (r.data.join("MultiMC"), "multimc.cfg"),
    ] {
        let configured = std::fs::read_to_string(base.join(cfg)).ok().and_then(|c| cfg_value(&c, "InstanceDir"));
        let dir = match configured.filter(|d| !d.is_empty()) {
            Some(d) if Path::new(&d).is_absolute() => PathBuf::from(d),
            Some(d) => base.join(d),
            None => base.join("instances"),
        };
        out.extend(subdirs(&dir).iter().filter(|d| d.join("instance.cfg").is_file()).filter_map(|d| inspect_instance(d)));
    }
    out
}

fn curseforge(r: &Roots) -> Vec<Source> {
    [r.home.join("curseforge"), r.home.join("Documents").join("curseforge")]
        .iter()
        .flat_map(|base| subdirs(&base.join("minecraft").join("Instances")))
        .filter_map(|d| inspect_instance(&d))
        .collect()
}

fn modrinth_rows(db: &Path) -> (Vec<(String, String, String, String, String)>, Option<String>) {
    let Ok(conn) = rusqlite::Connection::open_with_flags(db, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX) else {
        return (Vec::new(), None);
    };
    let custom_dir = conn
        .query_row("SELECT custom_dir FROM settings LIMIT 1", [], |row| row.get::<_, Option<String>>(0))
        .ok()
        .flatten()
        .filter(|d| !d.is_empty());
    let rows = (|| -> rusqlite::Result<Vec<_>> {
        let mut stmt = conn.prepare("SELECT * FROM profiles")?;
        let names: Vec<String> = stmt.column_names().iter().map(|s| s.to_string()).collect();
        let col = |row: &rusqlite::Row, key: &str| -> String {
            names.iter().position(|n| n == key).and_then(|i| row.get::<_, Option<String>>(i).ok().flatten()).unwrap_or_default()
        };
        let mapped = stmt.query_map([], |row| {
            Ok((col(row, "path"), col(row, "name"), col(row, "game_version"), col(row, "mod_loader"), col(row, "mod_loader_version")))
        })?;
        mapped.collect()
    })()
    .unwrap_or_default();
    (rows, custom_dir)
}

fn modrinth_root(root: &Path) -> Vec<Source> {
    let (rows, custom_dir) = modrinth_rows(&root.join("app.db"));
    let profiles = custom_dir.map(|d| PathBuf::from(d).join("profiles")).filter(|p| p.is_dir()).unwrap_or_else(|| root.join("profiles"));
    subdirs(&profiles)
        .iter()
        .filter_map(|dir| {
            let folder = dir.file_name()?.to_string_lossy().to_string();
            match rows.iter().find(|r| r.0 == folder) {
                Some((_, name, mc, loader, lv)) => source("Modrinth App", if name.is_empty() { &folder } else { name }, dir, mc, loader, lv),
                None => inspect_instance(dir).map(|mut s| {
                    s.launcher = "Modrinth App".into();
                    s
                }).or_else(|| source("Modrinth App", &folder, dir, "", "vanilla", "")),
            }
        })
        .collect()
}

fn xmcl_roots(r: &Roots) -> Vec<PathBuf> {
    let mut pointers = vec![r.data.join("xmcl").join("root"), r.home.join(".config/xmcl/root")];
    pointers.extend(
        subdirs(&r.local.join("Packages"))
            .into_iter()
            .filter(|p| p.file_name().is_some_and(|n| n.to_string_lossy().starts_with("xmcl_")))
            .map(|p| p.join("LocalCache").join("Roaming").join("xmcl").join("root")),
    );
    let mut roots: Vec<PathBuf> = pointers
        .iter()
        .filter_map(|p| std::fs::read_to_string(p).ok())
        .map(|s| PathBuf::from(s.trim()))
        .filter(|p| p.is_dir())
        .collect();
    roots.push(r.home.join(".minecraftx"));
    roots.sort();
    roots.dedup();
    roots
}

fn xmcl_root(root: &Path, index: Option<&Path>) -> Vec<Source> {
    let mut dirs: Vec<PathBuf> = subdirs(&root.join("instances"))
        .into_iter()
        .filter(|d| !d.file_name().is_some_and(|n| n.to_string_lossy().starts_with('.')))
        .collect();
    let listed: Vec<String> = index
        .and_then(read_json)
        .and_then(|v| v.get("instances")?.as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect()))
        .unwrap_or_default();
    for entry in listed {
        let p = PathBuf::from(&entry);
        dirs.push(if p.is_absolute() { p } else { root.join("instances").join(entry) });
    }
    dirs.sort();
    dirs.dedup();
    dirs.iter()
        .filter(|d| d.join("instance.json").is_file())
        .filter_map(|d| inspect_instance(d))
        .collect()
}

fn xmcl(r: &Roots) -> Vec<Source> {
    let index_for = |root: &Path| -> Option<PathBuf> {
        let pointer_dirs = [r.data.join("xmcl"), r.home.join(".config/xmcl")].into_iter().chain(
            subdirs(&r.local.join("Packages"))
                .into_iter()
                .filter(|p| p.file_name().is_some_and(|n| n.to_string_lossy().starts_with("xmcl_")))
                .map(|p| p.join("LocalCache").join("Roaming").join("xmcl")),
        );
        pointer_dirs
            .filter(|d| std::fs::read_to_string(d.join("root")).map(|s| Path::new(s.trim()) == root).unwrap_or(false))
            .map(|d| d.join("instances.json"))
            .find(|p| p.is_file())
    };
    xmcl_roots(r).iter().flat_map(|root| xmcl_root(root, index_for(root).as_deref())).collect()
}

pub fn scan(r: &Roots) -> Vec<Source> {
    let mut all = [official(r), prism(r), curseforge(r), modrinth_root(&r.data.join("ModrinthApp")), modrinth_root(&r.home.join(".var/app/com.modrinth.ModrinthApp/data/ModrinthApp")), xmcl(r)].concat();
    let mut seen = HashSet::new();
    all.retain(|s| seen.insert(s.path.to_lowercase()));
    all
}

pub fn inspect(path: &Path) -> Vec<Source> {
    if path.join("app.db").is_file() || path.join("profiles").is_dir() && !path.join("instances").is_dir() {
        return modrinth_root(path);
    }
    if let Some(one) = inspect_instance(path).filter(|s| s.launcher != "Minecraft 資料夾" || !path.join("instances").is_dir()) {
        return vec![one];
    }
    if path.join("instances").is_dir() {
        let index = path.join("instances.json");
        let mut found: Vec<Source> = subdirs(&path.join("instances")).iter().filter_map(|d| inspect_instance(d)).collect();
        if found.is_empty() {
            found = xmcl_root(path, index.is_file().then_some(index.as_path()));
        }
        return found;
    }
    subdirs(path).iter().filter_map(|d| inspect_instance(d)).collect()
}

fn remember(sources: &[Source]) {
    if let Ok(mut known) = KNOWN.lock() {
        known.extend(sources.iter().map(|s| PathBuf::from(&s.path)));
    }
}

fn is_known(path: &Path) -> bool {
    KNOWN.lock().map(|k| k.iter().any(|p| p == path)).unwrap_or(false)
}

#[tauri::command]
pub async fn import_sources() -> Vec<Source> {
    let found = tokio::task::spawn_blocking(|| Roots::system().map(|r| scan(&r)).unwrap_or_default())
        .await
        .unwrap_or_default();
    remember(&found);
    found
}

#[tauri::command]
pub async fn import_inspect(path: String) -> Vec<Source> {
    let found = tokio::task::spawn_blocking(move || inspect(Path::new(&path))).await.unwrap_or_default();
    remember(&found);
    found
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportRequest {
    pub path: String,
    pub instance_id: String,
    pub parts: Vec<String>,
}

pub fn copy_tree(from: &Path, to: &Path) -> std::io::Result<u64> {
    if from.is_file() {
        if let Some(parent) = to.parent() {
            std::fs::create_dir_all(parent)?;
        }
        return std::fs::copy(from, to);
    }
    let mut total = 0;
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)?.flatten() {
        if entry.file_name() == "session.lock" {
            continue;
        }
        total += copy_tree(&entry.path(), &to.join(entry.file_name()))?;
    }
    Ok(total)
}

pub async fn import_parts(request: ImportRequest) -> R<u64> {
    let src = PathBuf::from(&request.path);
    if !is_known(&src) || !src.is_dir() {
        bail!("請先從匯入清單選擇來源");
    }
    instance::get(&request.instance_id).await?;
    let dest = paths::game_dir(&request.instance_id);
    tokio::task::spawn_blocking(move || -> R<u64> {
        let mut total = 0;
        for part in request.parts.iter().filter(|p| PARTS.contains(&p.as_str())) {
            let from = src.join(part);
            if from.exists() {
                total += copy_tree(&from, &dest.join(part)).with_context(|| format!("複製 {part} 失敗"))?;
            }
        }
        Ok(total)
    })
    .await
    .context("匯入工作中斷")?
}

#[tauri::command]
pub async fn import_into(state: tauri::State<'_, crate::state::AppState>, request: ImportRequest) -> R<u64> {
    state.ensure_idle(&request.instance_id).await?;
    import_parts(request).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    struct Fixture(PathBuf);

    impl Fixture {
        fn new(tag: &str) -> Self {
            let dir = std::env::temp_dir().join(format!("bl-import-{tag}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }
        fn write(&self, rel: &str, body: &str) -> PathBuf {
            let p = self.0.join(rel);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(&p, body).unwrap();
            p
        }
        fn roots(&self) -> Roots {
            Roots { data: self.0.join("data"), local: self.0.join("local"), home: self.0.join("home") }
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn find<'a>(all: &'a [Source], name: &str) -> &'a Source {
        all.iter().find(|s| s.name == name).unwrap_or_else(|| panic!("找不到 {name}：{all:#?}"))
    }

    #[test]
    fn parses_metadata_of_every_launcher() {
        assert_eq!(parse_version_id("fabric-loader-0.16.10-1.21.1"), ("1.21.1".into(), "fabric".into(), "0.16.10".into()));
        assert_eq!(parse_version_id("1.20.1-forge-47.2.0"), ("1.20.1".into(), "forge".into(), "47.2.0".into()));
        assert_eq!(parse_version_id("26.3"), ("26.3".into(), "vanilla".into(), String::new()));
        let pack = json!({ "components": [{ "uid": "net.minecraft", "version": "1.21.1" }, { "uid": "org.quiltmc.quilt-loader", "version": "0.26.0" }] });
        assert_eq!(parse_mmc_pack(&pack), ("1.21.1".into(), "quilt".into(), "0.26.0".into()));
        let cf = json!({ "gameVersion": "1.20.1", "baseModLoader": { "name": "neoforge-20.1.5" } });
        assert_eq!(parse_curseforge(&cf), ("1.20.1".into(), "neoforge".into(), "20.1.5".into()));
        let x = json!({ "runtime": { "minecraft": "26.3", "forge": "", "fabricLoader": "0.19.5" } });
        assert_eq!(parse_xmcl(&x), ("26.3".into(), "fabric".into(), "0.19.5".into()));
    }

    #[test]
    fn scans_all_default_locations() {
        let f = Fixture::new("scan");
        f.write("data/.minecraft/saves/World/level.dat", "x");
        f.write("data/.minecraft/launcher_profiles.json", &json!({ "profiles": {
            "a": { "name": "預設", "lastVersionId": "fabric-loader-0.16.10-1.21.1", "lastUsed": "2026-10-01T00:00:00Z" },
            "b": { "name": "自訂", "lastVersionId": "1.20.4", "gameDir": f.0.join("custom").to_string_lossy() }
        } }).to_string());
        f.write("custom/options.txt", "fov:90");

        f.write("data/PrismLauncher/instances/fo/instance.cfg", "InstanceType=OneSix\nname=Fabulously Optimized\n");
        f.write("data/PrismLauncher/instances/fo/mmc-pack.json", &json!({ "components": [{ "uid": "net.minecraft", "version": "1.21.1" }, { "uid": "net.fabricmc.fabric-loader", "version": "0.16.10" }] }).to_string());
        f.write("data/PrismLauncher/instances/fo/.minecraft/mods/sodium.jar", "x");
        f.write("data/MultiMC/multimc.cfg", &format!("InstanceDir={}\n", f.0.join("mmc").display()));
        f.write("mmc/old/instance.cfg", "name=舊版\nIntendedVersion=1.12.2\n");
        f.write("mmc/old/minecraft/saves/a/level.dat", "x");

        f.write("home/curseforge/minecraft/Instances/cf/minecraftinstance.json", &json!({ "name": "CF 包", "gameVersion": "1.20.1", "baseModLoader": { "name": "fabric-0.15.11-1.20.1" } }).to_string());

        f.write("local/Packages/xmcl_abc/LocalCache/Roaming/xmcl/root", &f.0.join("xroot").to_string_lossy());
        f.write("local/Packages/xmcl_abc/LocalCache/Roaming/xmcl/instances.json", &json!({ "instances": ["26.3", f.0.join("elsewhere").to_string_lossy()] }).to_string());
        f.write("xroot/instances/26.3/instance.json", &json!({ "name": "26.3", "runtime": { "minecraft": "26.3", "fabricLoader": "0.19.5" } }).to_string());
        f.write("xroot/instances/26.3/saves/w/level.dat", "x");
        f.write("xroot/instances/.hidden/instance.json", &json!({ "name": "隱藏", "runtime": { "minecraft": "1.0" } }).to_string());
        f.write("elsewhere/instance.json", &json!({ "name": "", "runtime": { "minecraft": "1.20.6", "fabricLoader": "" } }).to_string());

        let db = f.0.join("data/ModrinthApp/app.db");
        std::fs::create_dir_all(db.parent().unwrap()).unwrap();
        let conn = rusqlite::Connection::open(&db).unwrap();
        conn.execute_batch("CREATE TABLE profiles (path TEXT, name TEXT, game_version TEXT, mod_loader TEXT, mod_loader_version TEXT, extra TEXT);
            INSERT INTO profiles VALUES ('Survival', '生存包', '1.21.4', 'fabric', '0.16.9', NULL);
            CREATE TABLE settings (custom_dir TEXT); INSERT INTO settings VALUES (NULL);").unwrap();
        drop(conn);
        f.write("data/ModrinthApp/profiles/Survival/mods/a.jar", "x");
        f.write("data/ModrinthApp/profiles/Legacy/profile.json", &json!({ "metadata": { "name": "舊格式", "game_version": "1.19.2", "loader": "fabric", "loader_version": { "id": "0.14.21" } } }).to_string());

        let all = scan(&f.roots());
        let official = find(&all, "官方啟動器 .minecraft");
        assert_eq!((official.mc_version.as_str(), official.loader.as_str(), official.saves), ("1.21.1", "fabric", 1));
        assert_eq!(find(&all, "自訂").mc_version, "1.20.4");
        let fo = find(&all, "Fabulously Optimized");
        assert_eq!((fo.mc_version.as_str(), fo.loader_version.as_str(), fo.mods), ("1.21.1", "0.16.10", 1));
        assert!(fo.path.ends_with(".minecraft"));
        let old = find(&all, "舊版");
        assert_eq!((old.mc_version.as_str(), old.saves), ("1.12.2", 1));
        assert!(old.path.ends_with("minecraft"));
        assert_eq!(find(&all, "CF 包").loader_version, "0.15.11");
        let x = find(&all, "26.3");
        assert_eq!((x.launcher.as_str(), x.loader_version.as_str(), x.saves), ("X Minecraft Launcher", "0.19.5", 1));
        assert_eq!(find(&all, "elsewhere").mc_version, "1.20.6");
        let mr = find(&all, "生存包");
        assert_eq!((mr.mc_version.as_str(), mr.loader.as_str(), mr.mods), ("1.21.4", "fabric", 1));
        assert_eq!(find(&all, "舊格式").loader_version, "0.14.21");
        assert_eq!(all.len(), 9, "{all:#?}");
    }

    #[test]
    fn scans_linux_flatpak_and_config_locations() {
        let f = Fixture::new("linux");
        f.write("home/.var/app/org.prismlauncher.PrismLauncher/data/PrismLauncher/instances/fp/instance.cfg", "name=Flatpak Prism
IntendedVersion=1.21.1
");
        f.write("home/.var/app/org.prismlauncher.PrismLauncher/data/PrismLauncher/instances/fp/.minecraft/options.txt", "fov:90");
        f.write("home/.config/xmcl/root", &f.0.join("xr").to_string_lossy());
        f.write("xr/instances/x/instance.json", &json!({ "name": "XMCL Linux", "runtime": { "minecraft": "26.3" } }).to_string());
        let all = scan(&f.roots());
        assert_eq!(find(&all, "Flatpak Prism").mc_version, "1.21.1");
        assert_eq!(find(&all, "XMCL Linux").launcher, "X Minecraft Launcher");
    }

    #[test]
    fn inspects_manually_picked_folders() {
        let f = Fixture::new("pick");
        f.write("portable/MultiMC/instances/pack/instance.cfg", "name=手動 MultiMC\n");
        f.write("portable/MultiMC/instances/pack/mmc-pack.json", &json!({ "components": [{ "uid": "net.minecraft", "version": "1.20.1" }] }).to_string());
        f.write("portable/MultiMC/instances/pack/.minecraft/options.txt", "x");
        let picked = inspect(&f.0.join("portable/MultiMC"));
        assert_eq!(picked.len(), 1);
        assert_eq!(picked[0].name, "手動 MultiMC");
        assert_eq!(inspect(&f.0.join("portable/MultiMC/instances/pack"))[0].mc_version, "1.20.1");
        f.write("plain/saves/w/level.dat", "x");
        assert_eq!(inspect(&f.0.join("plain"))[0].launcher, "Minecraft 資料夾");
        assert!(inspect(&f.0.join("nothing")).is_empty());
    }

    #[tokio::test]
    async fn imports_selected_parts_only_from_known_sources() {
        let f = Fixture::new("copy");
        std::env::set_var("BRILLIANT_LAUNCHER_HOME", f.0.join("launcher"));
        let target = instance::create(instance::Instance { name: "目標".into(), mc_version: "1.21.1".into(), ..Default::default() }).await.unwrap();
        f.write("src/saves/World/level.dat", "level");
        f.write("src/saves/World/session.lock", "lock");
        f.write("src/options.txt", "fov:100");
        f.write("src/mods/a.jar", "jar");
        let game = paths::game_dir(&target.id);
        std::fs::write(game.join("options.txt"), "fov:70").unwrap();
        let request = |path: &Path| ImportRequest {
            path: path.to_string_lossy().to_string(),
            instance_id: target.id.clone(),
            parts: vec!["saves".into(), "options.txt".into(), "../evil".into()],
        };

        assert!(import_parts(request(&f.0.join("src"))).await.is_err(), "沒掃描過的來源要拒絕");
        remember(&inspect(&f.0.join("src")));
        let bytes = import_parts(request(&f.0.join("src"))).await.unwrap();
        assert_eq!(bytes, 5 + 7);
        assert_eq!(std::fs::read_to_string(game.join("saves/World/level.dat")).unwrap(), "level");
        assert!(!game.join("saves/World/session.lock").exists());
        assert_eq!(std::fs::read_to_string(game.join("options.txt")).unwrap(), "fov:100");
        assert!(!game.join("mods/a.jar").exists());
        std::env::remove_var("BRILLIANT_LAUNCHER_HOME");
    }

    #[test]
    #[ignore]
    fn print_sources_on_this_machine() {
        for s in Roots::system().map(|r| scan(&r)).unwrap_or_default() {
            println!("{:<22} {:<28} mc={:<8} {:<8} {:<8} saves={:<3} mods={:<3} {}", s.launcher, s.name, s.mc_version, s.loader, s.loader_version, s.saves, s.mods, s.path);
        }
    }
}
