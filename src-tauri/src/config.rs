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
    read_json(&paths::config_file()).await
}

pub async fn save_settings(s: &Settings) -> R<()> {
    write_json(&paths::config_file(), s).await
}

pub async fn load_accounts() -> Vec<Account> {
    read_json(&paths::accounts_file()).await
}

pub async fn save_accounts(a: &[Account]) -> R<()> {
    write_json(&paths::accounts_file(), &a).await
}
