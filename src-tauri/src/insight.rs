use crate::bail;
use crate::config::{self, now_secs};
use crate::err::R;
use crate::paths;
use crate::state::AppState;
use anyhow::Context;
use base64::Engine;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::io::Read;
use std::path::{Path, PathBuf};
use tauri::State;

#[derive(Serialize, Default, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ModMeta {
    pub file: String,
    pub id: String,
    pub name: String,
    pub version: String,
    pub enabled: bool,
    pub managed: bool,
    pub blame: u32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CrashContext {
    pub report: String,
    pub native: String,
    pub mods: Vec<ModMeta>,
}

const SKIP: [&str; 20] = [
    "java.", "javax.", "jdk.", "sun.", "com.sun.", "net.minecraft.", "com.mojang.", "org.spongepowered.", "net.fabricmc.", "org.lwjgl.",
    "io.netty.", "com.google.", "org.apache.", "it.unimi.", "org.joml.", "oshi.", "org.slf4j.", "kotlin.", "org.objectweb.", "cpw.",
];

fn class_path(name: &str) -> Option<String> {
    let name = name.rsplit('/').next()?.split('$').next()?;
    let ok = name.split('.').count() >= 3
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_')
        && !SKIP.iter().any(|p| name.starts_with(p));
    ok.then(|| format!("{}.class", name.replace('.', "/")))
}

pub fn stack_hints(text: &str) -> (HashSet<String>, HashSet<String>) {
    let mut classes = HashSet::new();
    let mut ids = HashSet::new();
    for line in text.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("at ") {
            let call = rest.split('(').next().unwrap_or("");
            let method = call.rsplit('.').next().unwrap_or("");
            let parts: Vec<&str> = method.split('$').collect();
            if parts.len() >= 4 && parts[1].len() == 6 && parts[1].starts_with('z') {
                ids.insert(parts[2].to_string());
            }
            if let Some((class, _)) = call.rsplit_once('.') {
                classes.extend(class_path(class));
            }
        } else if let Some((class, tail)) = line.split_once(" (") {
            if tail.ends_with(".json)") {
                classes.extend(class_path(class));
            }
        }
    }
    (classes, ids)
}

fn read_meta(path: &Path, classes: &HashSet<String>) -> ModMeta {
    let file = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let mut meta = ModMeta {
        enabled: !file.ends_with(".disabled"),
        managed: crate::modinject::is_managed(&file),
        name: file.trim_end_matches(".disabled").trim_end_matches(".jar").to_string(),
        file,
        ..Default::default()
    };
    let Ok(mut zip) = std::fs::File::open(path).map_err(anyhow::Error::from).and_then(|f| Ok(zip::ZipArchive::new(f)?)) else {
        return meta;
    };
    if let Ok(mut entry) = zip.by_name("fabric.mod.json") {
        let mut raw = String::new();
        let _ = entry.read_to_string(&mut raw);
        let clean: String = raw.chars().map(|c| if c.is_control() { ' ' } else { c }).collect();
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&clean) {
            let get = |k: &str| json.get(k).and_then(|v| v.as_str()).unwrap_or("").to_string();
            meta.id = get("id");
            meta.version = get("version");
            if !get("name").is_empty() {
                meta.name = get("name");
            }
        }
    }
    meta.blame = zip.file_names().filter(|n| classes.contains(*n)).count() as u32;
    meta
}

pub fn mod_index(id: &str, text: &str) -> Vec<ModMeta> {
    let (classes, ids) = stack_hints(text);
    let mut out: Vec<ModMeta> = std::fs::read_dir(paths::game_dir(id).join("mods"))
        .map(|d| d.flatten().map(|e| e.path()).collect::<Vec<_>>())
        .unwrap_or_default()
        .into_iter()
        .filter(|p| p.is_file() && { let n = p.to_string_lossy(); n.ends_with(".jar") || n.ends_with(".jar.disabled") })
        .map(|p| read_meta(&p, &classes))
        .collect();
    for meta in out.iter_mut() {
        if ids.contains(&meta.id) {
            meta.blame += 5;
        }
        if meta.managed {
            meta.blame = 0;
        }
    }
    out
}

const CHEAT: &str = "外掛客戶端，內含飛行、透視、自動攻擊等修改遊戲的功能";
const FAST: &str = "取消挖掘或放置的延遲，等於加快挖礦、蓋方塊的速度，屬於修改遊戲數據";
const RISKY: &[(&[&str], &str)] = &[
    (&["wurst", "meteor-client", "liquidbounce", "thunderhack", "bleachhack", "aristois", "aoba", "lumina", "samsara", "schizoid", "meteorplus", "gemclient-core"], CHEAT),
    (&["nobreakdelay", "fastblocks*", "fastplace", "fastplace_extras_remake"], FAST),
    (&["tweakeroo"], "部分功能（例如快速放置、快速點擊、快速破壞、延長觸及距離）會修改遊戲行為，只要啟用這類功能就違反服規"),
    (&["baritone"], "自動走路、挖礦屬於允許的自動腳本，但部分設定（例如瞬間轉向、超出正常節奏的動作）會修改遊戲行為，啟用這類設定就違反服規"),
];

pub fn risk_of(mod_id: &str) -> Option<&'static str> {
    let id = mod_id.to_ascii_lowercase();
    RISKY
        .iter()
        .find(|(ids, _)| ids.iter().any(|k| k.strip_suffix('*').map_or(id == *k, |prefix| id.starts_with(prefix))))
        .map(|(_, reason)| *reason)
}

#[derive(Serialize)]
pub struct RiskyMod {
    pub file: String,
    pub id: String,
    pub name: String,
    pub version: String,
    pub reason: String,
}

pub fn risky_mods(id: &str, only: &[String]) -> Vec<RiskyMod> {
    mod_index(id, "")
        .into_iter()
        .filter(|m| m.enabled && (only.is_empty() || only.contains(&m.file)))
        .filter_map(|m| risk_of(&m.id).map(|reason| RiskyMod { file: m.file, id: m.id, name: m.name, version: m.version, reason: reason.to_string() }))
        .collect()
}

#[tauri::command]
pub async fn mod_risks(id: String, files: Vec<String>) -> R<Vec<RiskyMod>> {
    tokio::task::spawn_blocking(move || risky_mods(&id, &files)).await.context("檢查模組中斷").map_err(Into::into)
}

fn newest_since(dir: &Path, prefix: &str, ext: &str, since: i64) -> Option<PathBuf> {
    std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .filter(|e| { let n = e.file_name().to_string_lossy().to_string(); n.starts_with(prefix) && n.ends_with(ext) })
        .filter_map(|e| Some((e.metadata().ok()?.modified().ok()?.duration_since(std::time::UNIX_EPOCH).ok()?.as_secs() as i64, e.path())))
        .filter(|(t, _)| *t >= since)
        .max_by_key(|(t, _)| *t)
        .map(|(_, p)| p)
}

fn read_head(path: Option<PathBuf>, limit: u64) -> String {
    let mut out = String::new();
    if let Some(file) = path.and_then(|p| std::fs::File::open(p).ok()) {
        let mut bytes = Vec::new();
        let _ = file.take(limit).read_to_end(&mut bytes);
        out = String::from_utf8_lossy(&bytes).to_string();
    }
    out
}

#[derive(Serialize)]
pub struct ReportSection {
    pub key: String,
    pub title: String,
    pub text: String,
}

fn tail(path: &Path, limit: u64) -> String {
    use std::io::{Seek, SeekFrom};
    let Ok(mut file) = std::fs::File::open(path) else { return String::new() };
    let len = file.metadata().map(|m| m.len()).unwrap_or(0);
    let cut = len > limit;
    if cut && file.seek(SeekFrom::Start(len - limit)).is_err() {
        return String::new();
    }
    let mut bytes = Vec::new();
    let _ = file.take(limit).read_to_end(&mut bytes);
    let text = String::from_utf8_lossy(&bytes).to_string();
    if cut {
        let rest = text.split_once('\n').map(|(_, r)| r).unwrap_or(&text);
        format!("（檔案較大，只保留最後 {} KB）\n{rest}", limit / 1024)
    } else {
        text
    }
}

pub fn scrub_report(text: &str) -> String {
    let mut out: String = text.lines().map(crate::advanced::redact).collect::<Vec<_>>().join("\n");
    if let Some(home) = dirs::home_dir() {
        let plain = home.to_string_lossy().to_string();
        for form in [plain.clone(), plain.replace('\\', "/"), plain.replace('\\', "\\\\")] {
            if form.len() > 3 {
                out = out.replace(&form, "~");
            }
        }
    }
    out
}

fn strip_secrets(value: &mut serde_json::Value) {
    let Some(map) = value.as_object_mut() else { return };
    for key in ["selectedAccount", "notified", "lastAnnouncement", "image", "myReports"] {
        map.remove(key);
    }
    if let Some(serde_json::Value::String(env)) = map.get_mut("envVars") {
        *env = env.lines().filter_map(|l| l.split_once('=').map(|(k, _)| format!("{}=<已隱藏>", k.trim()))).collect::<Vec<_>>().join("\n");
    }
    if let Some(serde_json::Value::String(proxy)) = map.get_mut("proxy") {
        if let Some((scheme, rest)) = proxy.split_once("://") {
            *proxy = format!("{scheme}://{}", rest.rsplit_once('@').map(|(_, host)| host).unwrap_or(rest));
        }
    }
}

fn system_text(settings: &config::Settings, instance: Option<&crate::instance::Instance>) -> String {
    let info = crate::boost::system_info();
    let mut lines = vec![
        format!("輝煌啟動器 v{}（{} 通道）", env!("CARGO_PKG_VERSION"), config::channel_of(settings)),
        format!("作業系統：{} {}", std::env::consts::OS, std::env::consts::ARCH),
        format!("記憶體：{} MB，CPU {} 核心", info.memory_mb, info.cpus),
    ];
    if let Some(i) = instance {
        lines.push(format!("實例：Minecraft {} {} {}", i.mc_version, i.loader, i.loader_version));
        lines.push(format!("記憶體上限：{} MB", i.memory(settings)));
        lines.push(format!("Java：{}", if i.java(settings).is_empty() { "自動" } else { i.java(settings) }));
        lines.push(format!("JVM 參數：{}", i.jvm(settings)));
    }
    lines.join("\n")
}

#[tauri::command]
pub async fn collect_report(id: String, items: Vec<String>, since: i64) -> R<Vec<ReportSection>> {
    let settings = config::load_settings().await;
    let instance = crate::instance::get(&id).await.ok();
    tokio::task::spawn_blocking(move || {
        let game = paths::game_dir(&id);
        let mut out = Vec::new();
        for key in items {
            let (title, text) = match key.as_str() {
                "game_log" => ("完整遊戲日誌", tail(&game.join("logs").join("latest.log"), 1 << 20)),
                "crash_report" => ("當機報告", read_head(newest_since(&game.join("crash-reports"), "crash-", ".txt", since), 256 << 10)),
                "mods" => ("模組清單", mod_index(&id, "").iter().map(|m| format!("{}｜{} {}｜{}", m.file, m.id, m.version, if m.enabled { "啟用" } else { "停用" })).collect::<Vec<_>>().join("\n")),
                "system" => ("系統資訊", system_text(&settings, instance.as_ref())),
                "launcher_log" => ("啟動器日誌", newest_since(&paths::logs(), &format!("{id}-"), ".log", 0).map(|p| tail(&p, 1 << 20)).unwrap_or_default()),
                "panic" => ("啟動器錯誤紀錄", tail(&paths::logs().join("panic.log"), 64 << 10)),
                "native" => ("Java 崩潰紀錄", read_head(newest_since(&game, "hs_err_pid", ".log", since), 256 << 10)),
                "perf" => ("效能紀錄", std::fs::read_to_string(game.join("config/brilliant-server-mod/perf.json")).unwrap_or_default()),
                "settings" => {
                    let mut launcher = serde_json::to_value(&settings).unwrap_or_default();
                    strip_secrets(&mut launcher);
                    let mut inst = instance.as_ref().and_then(|i| serde_json::to_value(i).ok()).unwrap_or_default();
                    strip_secrets(&mut inst);
                    ("啟動器與實例設定", format!("【啟動器】\n{}\n\n【實例】\n{}", serde_json::to_string_pretty(&launcher).unwrap_or_default(), serde_json::to_string_pretty(&inst).unwrap_or_default()))
                }
                _ => continue,
            };
            let text = scrub_report(&text);
            if !text.trim().is_empty() {
                out.push(ReportSection { key, title: title.to_string(), text });
            }
        }
        out
    })
    .await
    .context("收集回報資料中斷")
    .map_err(Into::into)
}

#[tauri::command]
pub async fn crash_context(id: String, since: i64, lines: Vec<String>) -> R<CrashContext> {
    tokio::task::spawn_blocking(move || {
        let game = paths::game_dir(&id);
        let report = read_head(newest_since(&game.join("crash-reports"), "crash-", ".txt", since), 256 << 10);
        let native = read_head(newest_since(&game, "hs_err_pid", ".log", since), 16 << 10);
        let text = format!("{report}\n{}", lines.join("\n"));
        CrashContext { mods: mod_index(&id, &text), report, native }
    })
    .await
    .context("讀取當機資料中斷")
    .map_err(Into::into)
}

pub const PERF_KEYS: [&str; 12] = [
    "renderDistance", "simulationDistance", "graphicsMode", "particles", "biomeBlendRadius", "maxFps",
    "enableVsync", "entityShadows", "cloudStatus", "entityDistanceScaling", "mipmapLevels", "ao",
];

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PerfReport {
    pub options: HashMap<String, String>,
    pub mods: Vec<String>,
    pub sessions: serde_json::Value,
    pub memory_mb: u32,
    pub system: crate::boost::SystemInfo,
}

fn read_options(path: &Path) -> HashMap<String, String> {
    std::fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .filter_map(|l| l.split_once(':'))
        .filter(|(k, _)| PERF_KEYS.contains(k))
        .map(|(k, v)| (k.to_string(), v.trim().to_string()))
        .collect()
}

pub fn patch_options(text: &str, changes: &HashMap<String, String>) -> String {
    let mut seen = HashSet::new();
    let mut out: Vec<String> = text
        .lines()
        .map(|line| match line.split_once(':').and_then(|(k, _)| changes.get(k).map(|v| (k, v))) {
            Some((k, v)) => { seen.insert(k.to_string()); format!("{k}:{v}") }
            None => line.to_string(),
        })
        .collect();
    let mut rest: Vec<_> = changes.iter().filter(|(k, _)| !seen.contains(*k)).map(|(k, v)| format!("{k}:{v}")).collect();
    rest.sort();
    out.extend(rest);
    out.join("\n") + "\n"
}

fn valid_option(key: &str, value: &str) -> bool {
    PERF_KEYS.contains(&key) && !value.is_empty() && value.len() <= 16 && value.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '"'))
}

#[tauri::command]
pub async fn perf_report(id: String) -> R<PerfReport> {
    let settings = config::load_settings().await;
    let instance = crate::instance::get(&id).await?;
    tokio::task::spawn_blocking(move || {
        let game = paths::game_dir(&id);
        let sessions = std::fs::read_to_string(game.join("config/brilliant-server-mod/perf.json"))
            .ok()
            .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
            .and_then(|v| v.get("sessions").cloned())
            .unwrap_or(serde_json::json!([]));
        PerfReport {
            options: read_options(&game.join("options.txt")),
            mods: mod_index(&id, "").into_iter().filter(|m| m.enabled && !m.id.is_empty()).map(|m| m.id).collect(),
            sessions,
            memory_mb: instance.memory(&settings),
            system: crate::boost::system_info(),
        }
    })
    .await
    .context("讀取效能資料中斷")
    .map_err(Into::into)
}

#[tauri::command]
pub async fn apply_perf(state: State<'_, AppState>, id: String, options: HashMap<String, String>, memory_mb: Option<u32>) -> R<()> {
    if state.is_running(&id).await {
        bail!("請先關閉遊戲再套用，遊戲結束時會覆寫設定");
    }
    if options.iter().any(|(k, v)| !valid_option(k, v)) {
        bail!("設定值不正確");
    }
    if !options.is_empty() {
        let file = paths::game_dir(&id).join("options.txt");
        let text = tokio::fs::read_to_string(&file).await.unwrap_or_default();
        tokio::fs::create_dir_all(paths::game_dir(&id)).await?;
        tokio::fs::write(&file, patch_options(&text, &options)).await.context("寫入遊戲設定失敗")?;
    }
    if let Some(mb) = memory_mb.filter(|mb| (1024..=65536).contains(mb)) {
        let mut instance = crate::instance::get(&id).await?;
        instance.memory_mb = mb;
        crate::instance::save(&instance).await?;
    }
    Ok(())
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    pub instance: String,
    pub name: String,
    pub start: i64,
    pub seconds: i64,
    pub code: i32,
}

fn sessions_file() -> PathBuf {
    paths::root().join("sessions.jsonl")
}

pub async fn record_session(session: Session) {
    use tokio::io::AsyncWriteExt;
    let Ok(line) = serde_json::to_string(&session) else { return };
    if let Ok(mut file) = tokio::fs::OpenOptions::new().create(true).append(true).open(sessions_file()).await {
        let _ = file.write_all(format!("{line}\n").as_bytes()).await;
    }
}

#[tauri::command]
pub async fn play_history(since: i64) -> Vec<Session> {
    tokio::fs::read_to_string(sessions_file())
        .await
        .unwrap_or_default()
        .lines()
        .filter_map(|l| serde_json::from_str::<Session>(l).ok())
        .filter(|s| s.start >= since)
        .collect()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Shot {
    pub instance: String,
    pub instance_name: String,
    pub path: String,
    pub name: String,
    pub time: i64,
    pub meta: Option<serde_json::Value>,
    pub thumb: Option<String>,
}

pub fn thumbs_dir() -> std::path::PathBuf {
    paths::root().join("cache").join("thumbs")
}

fn thumb_path(id: &str, file: &str, time: i64) -> std::path::PathBuf {
    thumbs_dir().join(format!("{}.jpg", crate::net::sha1_hex(format!("{id}/{file}/{time}").as_bytes())))
}

#[tauri::command]
pub async fn list_all_screenshots() -> Vec<Shot> {
    let mut out = Vec::new();
    for instance in crate::instance::list().await {
        let metas: serde_json::Map<String, serde_json::Value> = tokio::fs::read_to_string(paths::game_dir(&instance.id).join("config/brilliant-server-mod/screenshots.json"))
            .await
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        for shot in crate::extras::list_screenshots(instance.id.clone()).await {
            let meta = metas.get(&shot.name).cloned();
            let thumb = Some(thumb_path(&instance.id, &shot.name, shot.time)).filter(|p| p.is_file()).map(|p| p.to_string_lossy().to_string());
            out.push(Shot { instance: instance.id.clone(), instance_name: instance.name.clone(), path: shot.path, name: shot.name, time: shot.time, meta, thumb });
        }
    }
    out.sort_by_key(|s| std::cmp::Reverse(s.time));
    out
}

fn safe_png(file: &str) -> bool {
    file.to_ascii_lowercase().ends_with(".png") && !file.contains(['/', '\\']) && !file.contains("..")
}

#[tauri::command]
pub async fn delete_screenshot(id: String, file: String) -> R<()> {
    if !safe_png(&file) {
        bail!("截圖檔名不正確");
    }
    tokio::fs::remove_file(paths::game_dir(&id).join("screenshots").join(file)).await.context("刪除截圖失敗")?;
    Ok(())
}

#[tauri::command]
pub async fn save_shot_thumb(id: String, file: String, time: i64, data: String) -> R<String> {
    if !safe_png(&file) {
        bail!("截圖檔名不正確");
    }
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(data.trim_start_matches("data:image/jpeg;base64,"))
        .context("圖片資料不正確")?;
    if bytes.len() > 2 << 20 || !bytes.starts_with(&[0xFF, 0xD8]) {
        bail!("圖片資料不正確");
    }
    let path = thumb_path(&id, &file, time);
    tokio::fs::create_dir_all(thumbs_dir()).await?;
    tokio::fs::write(&path, bytes).await.context("儲存縮圖失敗")?;
    Ok(path.to_string_lossy().to_string())
}

#[tauri::command]
pub async fn save_png(name: String, data: String) -> R<String> {
    if !safe_png(&name) {
        bail!("檔名不正確");
    }
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(data.trim_start_matches("data:image/png;base64,"))
        .context("圖片資料不正確")?;
    if bytes.len() > 20 << 20 || !bytes.starts_with(b"\x89PNG") {
        bail!("圖片資料不正確");
    }
    let dir = dirs::download_dir().unwrap_or_else(|| paths::root().join("exports"));
    tokio::fs::create_dir_all(&dir).await?;
    let path = dir.join(format!("{}-{}.png", name.trim_end_matches(".png"), now_secs()));
    tokio::fs::write(&path, bytes).await.context("儲存圖片失敗")?;
    Ok(path.to_string_lossy().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn risky_mods_are_recognised() {
        assert!(risk_of("wurst").is_some() && risk_of("Meteor-Client").is_some() && risk_of("tweakeroo").is_some());
        assert!(risk_of("sodium").is_none() && risk_of("litematica").is_none() && risk_of("wurstmeal").is_none());
        assert!(risk_of("xray").is_none() && risk_of("freecam").is_none());
        assert!(risk_of("fastblocks26_3").is_some() && risk_of("nobreakdelay").is_some() && risk_of("baritone").is_some() && risk_of("lethal-camera").is_none() && risk_of("inertia").is_none());
    }

    #[test]
    fn report_text_is_scrubbed() {
        let home = dirs::home_dir().unwrap().to_string_lossy().to_string();
        let raw = format!("{home}\\AppData\\x.log\n--accessToken abc.def\nok");
        let out = scrub_report(&raw);
        assert!(!out.contains(&home) && out.contains("~") && !out.contains("abc.def") && out.ends_with("ok"));
        let mut value = serde_json::json!({ "selectedAccount": "a", "envVars": "TOKEN=secret\nA=1", "proxy": "http://user:pass@127.0.0.1:8080" });
        strip_secrets(&mut value);
        assert_eq!(value, serde_json::json!({ "envVars": "TOKEN=<已隱藏>\nA=<已隱藏>", "proxy": "http://127.0.0.1:8080" }));
        let path = std::env::temp_dir().join("brilliant-tail.log");
        std::fs::write(&path, "first line\nsecond\nthird").unwrap();
        assert_eq!(tail(&path, 100), "first line\nsecond\nthird");
        assert!(tail(&path, 10).ends_with("third") && !tail(&path, 10).contains("first"));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn hints_and_options() {
        let trace = "java.lang.NullPointerException\n\tat net.minecraft.class_310.method_1(class_310.java:1)\n\tat me.jelly.sodium.client.render.Chunk$Inner.build(Chunk.java:9)\n\tat net.minecraft.class_757.handler$zbc000$iris$onRender(class_757.java:3)\nMixins in Stacktrace:\n\t\tcom.example.mixin.FooMixin (example.mixins.json)";
        let (classes, ids) = stack_hints(trace);
        assert!(classes.contains("me/jelly/sodium/client/render/Chunk.class"));
        assert!(classes.contains("com/example/mixin/FooMixin.class"));
        assert!(!classes.iter().any(|c| c.starts_with("net/minecraft")));
        assert!(ids.contains("iris"));
        let changes = HashMap::from([("renderDistance".to_string(), "8".to_string()), ("particles".to_string(), "1".to_string())]);
        let out = patch_options("version:4000\nrenderDistance:16\nlang:zh_tw\n", &changes);
        assert_eq!(out, "version:4000\nrenderDistance:8\nlang:zh_tw\nparticles:1\n");
        assert!(valid_option("graphicsMode", "1") && !valid_option("lang", "x") && !valid_option("maxFps", "1\nkey:2"));
    }
}
