use crate::config::Settings;
use crate::err::R;
use crate::instance::Instance;
use crate::net::{self, DlFile};
use crate::paths;
use md5::{Digest, Md5};
use serde::{Deserialize, Serialize};
use std::path::Path;
use tauri::AppHandle;

const PACK_INDEX: &str = "https://www.brilliantw.net/resourcepack/latest.json";
const PACK_PREFIX: &str = "https://www.brilliantw.net/resourcepack/";
const DEFAULT_PACKS: [&str; 2] = ["bsje.zip", "bsje-riceboss.zip"];
const INHERITED_FILES: [&str; 4] = ["options.txt", "servers.dat", "optionsshaders.txt", "optionsof.txt"];

#[derive(Deserialize)]
struct PackIndex {
    packs: Vec<Pack>,
}

#[derive(Deserialize)]
struct Pack {
    name: String,
    url: String,
    hash: String,
}

pub fn name_uuid(text: &str) -> String {
    let mut h: [u8; 16] = Md5::digest(text.as_bytes()).into();
    h[6] = (h[6] & 0x0f) | 0x30;
    h[8] = (h[8] & 0x3f) | 0x80;
    let x = hex::encode(h);
    format!("{}-{}-{}-{}-{}", &x[0..8], &x[8..12], &x[12..16], &x[16..20], &x[20..32])
}

fn received_packs(game_dir: &Path) -> Vec<String> {
    let log = std::fs::read_to_string(game_dir.join("downloads").join("log.json")).unwrap_or_default();
    let mut names: Vec<String> = log
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .filter_map(|v| v.get("url")?.as_str().map(str::to_string))
        .filter(|url| url.starts_with(PACK_PREFIX))
        .filter_map(|url| url.split('#').next()?.rsplit('/').next().map(str::to_string))
        .collect();
    names.sort();
    names.dedup();
    names
}

pub async fn sync_server_packs(app: &AppHandle, instance: &Instance) -> R<()> {
    let key = format!("packs:{}", instance.id);
    if crate::modinject::synced_recently(&key) {
        return Ok(());
    }
    let index: PackIndex = net::get_json(PACK_INDEX, false).await?;
    let game_dir = paths::game_dir(&instance.id);
    let mut wanted = received_packs(&game_dir);
    if wanted.is_empty() {
        wanted = DEFAULT_PACKS.iter().map(|s| s.to_string()).collect();
    }
    let jobs: Vec<DlFile> = index
        .packs
        .into_iter()
        .filter(|p| wanted.contains(&p.name) && p.url.starts_with(PACK_PREFIX) && p.hash.len() == 40 && p.hash.chars().all(|c| c.is_ascii_hexdigit()))
        .map(|p| {
            let dest = game_dir.join("downloads").join(name_uuid(&p.url)).join(&p.hash);
            DlFile::new(p.url, dest).sha1(Some(p.hash))
        })
        .collect();
    if !jobs.is_empty() {
        net::download_all(app, "respack", jobs, false).await?;
    }
    crate::modinject::mark_synced(key);
    Ok(())
}

pub fn inherit_settings(to: &str, from: Option<&str>) {
    let src = match from {
        Some(id) => paths::game_dir(id),
        None => match official_minecraft_dir() {
            Some(d) => d,
            None => return,
        },
    };
    let dest = paths::game_dir(to);
    let _ = std::fs::create_dir_all(dest.join("config"));
    for name in INHERITED_FILES {
        if src.join(name).is_file() && !dest.join(name).exists() {
            let _ = std::fs::copy(src.join(name), dest.join(name));
        }
    }
    for entry in std::fs::read_dir(src.join("config")).map(|d| d.flatten().collect::<Vec<_>>()).unwrap_or_default() {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with("brilliant-server-mod-") && name.ends_with(".json") && entry.path().is_file() {
            let _ = std::fs::copy(entry.path(), dest.join("config").join(&name));
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemInfo {
    pub memory_mb: u64,
    pub recommended_mb: u32,
    pub cpus: usize,
}

#[cfg(windows)]
fn total_memory_mb() -> u64 {
    use windows_sys::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
    let mut status: MEMORYSTATUSEX = unsafe { std::mem::zeroed() };
    status.dwLength = std::mem::size_of::<MEMORYSTATUSEX>() as u32;
    if unsafe { GlobalMemoryStatusEx(&mut status) } != 0 {
        status.ullTotalPhys / 1_048_576
    } else {
        0
    }
}

#[cfg(target_os = "linux")]
fn total_memory_mb() -> u64 {
    std::fs::read_to_string("/proc/meminfo")
        .ok()
        .and_then(|s| s.lines().find(|l| l.starts_with("MemTotal:"))?.split_whitespace().nth(1)?.parse::<u64>().ok())
        .map(|kb| kb / 1024)
        .unwrap_or(0)
}

#[cfg(target_os = "macos")]
fn total_memory_mb() -> u64 {
    std::process::Command::new("sysctl")
        .args(["-n", "hw.memsize"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8_lossy(&o.stdout).trim().parse::<u64>().ok())
        .map(|bytes| bytes / 1_048_576)
        .unwrap_or(0)
}

#[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
fn total_memory_mb() -> u64 {
    0
}

pub fn official_minecraft_dir() -> Option<std::path::PathBuf> {
    if cfg!(target_os = "linux") {
        dirs::home_dir().map(|h| h.join(".minecraft"))
    } else if cfg!(target_os = "macos") {
        dirs::data_dir().map(|d| d.join("minecraft"))
    } else {
        dirs::data_dir().map(|d| d.join(".minecraft"))
    }
}

pub fn recommended_memory(total_mb: u64) -> u32 {
    match total_mb {
        0 => 4096,
        t if t <= 6 * 1024 => 2048,
        t if t <= 10 * 1024 => 3072,
        t if t <= 18 * 1024 => 4096,
        t if t <= 34 * 1024 => 6144,
        _ => 8192,
    }
}

#[tauri::command]
pub fn system_info() -> SystemInfo {
    let memory_mb = total_memory_mb();
    SystemInfo {
        memory_mb,
        recommended_mb: recommended_memory(memory_mb),
        cpus: std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4),
    }
}

pub fn notify(app: &AppHandle, title: &str, body: &str) {
    use tauri_plugin_notification::NotificationExt;
    let _ = app.notification().builder().title(title).body(body).show();
}

#[tauri::command]
pub fn notify_user(app: AppHandle, title: String, body: String) {
    notify(&app, &title, &body);
}

pub fn wants_packs(instance: &Instance, settings: &Settings) -> bool {
    instance.wants_rich_presence(settings)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pack_uuid_matches_game() {
        assert_eq!(name_uuid("https://www.brilliantw.net/resourcepack/20/bsje.zip"), "0fe64b10-6531-364a-8148-443c90d6b41e");
        assert_eq!(recommended_memory(16 * 1024), 4096);
        assert_eq!(recommended_memory(8 * 1024), 3072);
        assert_eq!(recommended_memory(64 * 1024), 8192);
    }
}
