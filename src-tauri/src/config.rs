use crate::err::R;
use crate::paths;
use serde::{Deserialize, Serialize};
use std::path::Path;

pub const MSA_CLIENT_ID: &str = "ce34fb5a-d8f0-4754-8fbf-68f6e034cdfc";

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    pub use_mirror: bool,
    pub memory_mb: u32,
    pub jvm_args: String,
    pub java_path: String,
    pub java_auto_update: bool,
    pub width: u32,
    pub height: u32,
    pub fullscreen: bool,
    pub after_launch: String,
    pub theme: String,
    pub font: String,
    pub inject_rich_presence: bool,
    pub selected_account: String,
    pub selected_instance: String,
    pub show_snapshots: bool,
    pub show_old: bool,
    pub quick_join: String,
    pub pre_launch_cmd: String,
    pub post_exit_cmd: String,
    pub backup_on_launch: bool,
    pub backup_keep: u32,
    pub download_threads: u32,
    pub close_to_tray: bool,
    pub autostart: bool,
    pub accent: String,
    pub background: String,
    pub slide_seconds: u32,
    pub ui_scale: u32,
    pub reduce_motion: bool,
    pub walkers: bool,
    pub notifications: bool,
    pub inherit_options: bool,
    pub hardware_tuned: bool,
    pub last_announcement: String,
    pub notified: Vec<String>,
    pub my_reports: Vec<serde_json::Value>,
    pub music: String,
    pub music_volume: u32,
    pub usage_stats: bool,
    pub memory_min_mb: u32,
    pub env_vars: String,
    pub wrapper: String,
    pub prefer_dgpu: bool,
    pub shared_folders: Vec<String>,
    pub mod_channel: String,
    pub auto_update_mods: bool,
    pub proxy: String,
    pub debug_mode: bool,
    pub open_log_on_launch: bool,
    pub launcher_update: String,
    pub mod_update: String,
    pub update_channel: String,
    pub alpha_unlocked: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            use_mirror: true,
            memory_mb: 4096,
            jvm_args: DEFAULT_JVM_ARGS.to_string(),
            java_path: String::new(),
            java_auto_update: false,
            width: 854,
            height: 480,
            fullscreen: false,
            after_launch: "hide".into(),
            theme: "auto".into(),
            font: "pixel".into(),
            inject_rich_presence: true,
            selected_account: String::new(),
            selected_instance: String::new(),
            show_snapshots: false,
            show_old: false,
            quick_join: String::new(),
            pre_launch_cmd: String::new(),
            post_exit_cmd: String::new(),
            backup_on_launch: false,
            backup_keep: 5,
            download_threads: 12,
            close_to_tray: false,
            autostart: false,
            accent: String::new(),
            background: String::new(),
            slide_seconds: 15,
            ui_scale: 100,
            reduce_motion: false,
            walkers: false,
            notifications: true,
            inherit_options: true,
            hardware_tuned: false,
            last_announcement: String::new(),
            notified: Vec::new(),
            my_reports: Vec::new(),
            music: String::new(),
            music_volume: 40,
            usage_stats: true,
            memory_min_mb: 0,
            env_vars: String::new(),
            wrapper: String::new(),
            prefer_dgpu: false,
            shared_folders: Vec::new(),
            mod_channel: "release".into(),
            auto_update_mods: false,
            proxy: String::new(),
            debug_mode: false,
            open_log_on_launch: false,
            launcher_update: "auto".into(),
            mod_update: "auto".into(),
            update_channel: "stable".into(),
            alpha_unlocked: false,
        }
    }
}

pub const MEMORY_RANGE: (u32, u32) = (1024, 32768);
pub const WIDTH_RANGE: (u32, u32) = (320, 7680);
pub const HEIGHT_RANGE: (u32, u32) = (240, 4320);

pub fn clean_line(value: &mut String, max: usize) {
    *value = value.chars().filter(|c| !c.is_control()).take(max).collect::<String>().trim().to_string();
}

pub fn clean_text(value: &mut String, max: usize) {
    *value = value.chars().filter(|c| *c == '\n' || !c.is_control()).take(max).collect();
}

pub fn clean_choice(value: &mut String, allowed: &[&str], fallback: &str) {
    if !allowed.contains(&value.as_str()) {
        *value = fallback.to_string();
    }
}

pub fn clean_jvm_args(value: &mut String) {
    *value = value.chars().take(4096).map(|c| if c.is_control() { ' ' } else { c }).collect::<String>();
    *value = value.split_whitespace().filter(|t| t.starts_with('-') && t.len() > 1).collect::<Vec<_>>().join(" ");
}

pub fn clean_env_vars(value: &mut String) {
    clean_text(value, 8192);
    *value = value
        .lines()
        .map(str::trim)
        .filter(|l| l.starts_with('#') || l.split_once('=').is_some_and(|(k, _)| !k.is_empty() && k.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') && !k.starts_with(|c: char| c.is_ascii_digit())))
        .collect::<Vec<_>>()
        .join("\n");
}

pub fn clean_size(value: &mut u32, range: (u32, u32), allow_zero: bool) {
    if !(allow_zero && *value == 0) {
        *value = (*value).clamp(range.0, range.1);
    }
}

fn valid_host(host: &str) -> bool {
    let (name, port) = host.rsplit_once(':').map_or((host, None), |(n, p)| (n, Some(p)));
    !name.is_empty()
        && name.len() <= 253
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-')
        && port.is_none_or(|p| p.parse::<u16>().is_ok_and(|p| p > 0))
}

impl Settings {
    pub fn clean(&mut self) {
        clean_size(&mut self.memory_mb, MEMORY_RANGE, false);
        self.memory_min_mb = self.memory_min_mb.min(self.memory_mb);
        clean_size(&mut self.width, WIDTH_RANGE, false);
        clean_size(&mut self.height, HEIGHT_RANGE, false);
        self.backup_keep = self.backup_keep.clamp(1, 50);
        self.download_threads = self.download_threads.clamp(1, 64);
        self.slide_seconds = self.slide_seconds.min(60);
        self.ui_scale = self.ui_scale.clamp(80, 125);
        self.music_volume = self.music_volume.min(100);
        clean_choice(&mut self.after_launch, &["hide", "keep", "exit"], "hide");
        clean_choice(&mut self.theme, &["auto", "light", "dark"], "auto");
        clean_choice(&mut self.font, &["pixel", "system"], "pixel");
        clean_choice(&mut self.mod_channel, &["release", "beta", "alpha"], "release");
        clean_choice(&mut self.launcher_update, &["auto", "notify", "off"], "auto");
        clean_choice(&mut self.mod_update, &["auto", "keep"], "auto");
        clean_choice(&mut self.update_channel, &["stable", "beta", "alpha"], "stable");
        clean_jvm_args(&mut self.jvm_args);
        clean_env_vars(&mut self.env_vars);
        for (field, max) in [(&mut self.java_path, 1024), (&mut self.pre_launch_cmd, 1024), (&mut self.post_exit_cmd, 1024), (&mut self.wrapper, 1024), (&mut self.proxy, 512), (&mut self.background, 1024), (&mut self.music, 1024), (&mut self.quick_join, 260)] {
            clean_line(field, max);
        }
        if !self.quick_join.is_empty() && !valid_host(&self.quick_join) {
            self.quick_join.clear();
        }
        if !(self.accent.is_empty() || self.accent.len() == 7 && self.accent.starts_with('#') && self.accent[1..].chars().all(|c| c.is_ascii_hexdigit())) {
            self.accent.clear();
        }
        self.shared_folders.retain(|f| crate::advanced::SHAREABLE.contains(&f.as_str()));
        self.shared_folders.dedup();
    }
}

pub const DEFAULT_JVM_ARGS: &str = "-XX:+UnlockExperimentalVMOptions -XX:+UseG1GC \
-XX:G1NewSizePercent=20 -XX:G1ReservePercent=20 -XX:MaxGCPauseMillis=50 \
-XX:G1HeapRegionSize=32M -XX:-UseAdaptiveSizePolicy -XX:-OmitStackTraceInFastThrow";

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct Account {
    pub id: String,
    pub kind: String,
    pub name: String,
    pub uuid: String,
    pub access_token: String,
    pub refresh_token: String,
    pub xuid: String,
    pub expires_at: i64,
    pub session_error: String,
}

impl Account {
    pub fn is_expired(&self) -> bool {
        self.kind == "msa" && self.expires_at <= now_secs() + 60
    }
}

pub fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

async fn read_json<T: serde::de::DeserializeOwned + Default>(path: &Path) -> T {
    match tokio::fs::read(path).await {
        Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_else(|e| {
            eprintln!("[config] {} 解析失敗，改用預設值：{e}", path.display());
            T::default()
        }),
        Err(_) => T::default(),
    }
}

async fn write_json<T: Serialize>(path: &Path, value: &T) -> R<()> {
    let bytes = serde_json::to_vec_pretty(value)?;
    crate::net::write_file(path, &bytes, false).await
}

pub fn channel_of(settings: &Settings) -> &'static str {
    match settings.update_channel.as_str() {
        "beta" => "beta",
        "alpha" if settings.alpha_unlocked => "alpha",
        _ => "stable",
    }
}

pub async fn load_settings() -> Settings {
    let mut settings: Settings = read_json(&paths::config_file()).await;
    settings.clean();
    settings
}

pub async fn save_settings(s: &Settings) -> R<()> {
    let mut s = s.clone();
    s.clean();
    write_json(&paths::config_file(), &s).await
}

pub async fn load_accounts() -> Vec<Account> {
    read_json(&paths::accounts_file()).await
}

pub async fn save_accounts(a: &[Account]) -> R<()> {
    write_json(&paths::accounts_file(), &a).await
}
