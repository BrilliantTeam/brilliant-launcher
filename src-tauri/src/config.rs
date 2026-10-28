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
    pub instance_groups: Vec<InstanceGroup>,
    pub pinned: String,
    pub button_style: String,
    pub corners: String,
    pub panel_opacity: u32,
    pub bg_dim: u32,
    pub rail_labels: bool,
    pub card_size: String,
    pub show_news: bool,
    pub tours_seen: Vec<String>,
    pub content_width: String,
    pub density: String,
    pub accent_bar: String,
    pub bg_blur: u32,
    pub bg_motion: bool,
    pub thin_scrollbars: bool,
    pub card_details: bool,
    pub show_brand: bool,
    pub home_tagline: String,
    pub bg_filter: String,
    pub home_layout: String,
    pub rail_style: String,
    pub cover_height: String,
    pub cursor_style: String,
    pub click_sound: bool,
    pub show_clock: bool,
    pub bg_source: String,
    pub bg_color: String,
    pub bg_color2: String,
    pub bg_pin: String,
    pub bg_transition: String,
    pub bg_pan: bool,
    pub weather: String,
    pub clock_format: String,
    pub clock_seconds: bool,
    pub clock_style: String,
    pub clock_position: String,
    pub play_size: String,
    pub play_text: String,
    pub shard_layout: String,
    pub hidden_shards: Vec<String>,
    pub show_credit: bool,
    pub show_social: bool,
    pub show_shard_stats: bool,
    pub show_last_played: bool,
    pub dark_from: u32,
    pub dark_to: u32,
    pub accent_from_bg: bool,
    pub auto_palette: bool,
    pub color_success: String,
    pub color_warning: String,
    pub color_link: String,
    pub high_contrast: bool,
    pub font_scale: u32,
    pub heading_style: String,
    pub card_view: String,
    pub card_hover: String,
    pub cover_shade: u32,
    pub instance_sort: String,
    pub cover_position: String,
    pub page_stats: bool,
    pub rail_side: String,
    pub rail_order: Vec<String>,
    pub rail_hidden: Vec<String>,
    pub hide_account_name: bool,
    pub hidden_tools: Vec<String>,
    pub start_page: String,
    pub anim_speed: String,
    pub click_sound_type: String,
    pub click_volume: u32,
    pub page_transition: String,
    pub toast_position: String,
    pub toast_seconds: u32,
    pub launch_splash: bool,
    pub custom_font: String,
    pub fonts: Vec<MediaItem>,
    pub playlist: Vec<MediaItem>,
    pub music_mode: String,
    pub shortcuts: std::collections::BTreeMap<String, String>,
    pub global_shortcut: String,
    pub rail_links: Vec<RailLink>,
    pub profiles: Vec<Profile>,
    pub saved_looks: Vec<SavedLook>,
    pub streamer_mode: bool,
    pub instance_order: Vec<String>,
    pub options_sync: bool,
    pub options_sync_groups: Vec<String>,
    pub window_mode: String,
    pub window_monitor: String,
    pub remember_window: bool,
    pub mod_snapshots: bool,
    pub home_widgets: Vec<WidgetPos>,
    pub home_note: String,
    pub countdown_title: String,
    pub countdown_at: i64,
    pub color_overrides: std::collections::BTreeMap<String, String>,
    pub language: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct WidgetPos {
    pub id: String,
    pub x: f64,
    pub y: f64,
    pub scale: f64,
    pub visible: bool,
}

pub const WIDGETS: [&str; 9] = ["brand", "clock", "news", "dock", "social", "credit", "stats", "note", "countdown"];

pub const COLOR_TOKENS: [&str; 12] = ["content-bg", "content-bg-alt", "content-bg-sunken", "text", "text-subtle", "border", "border-soft", "panel-face", "chrome-bg", "chrome-ink", "glass-bg", "glass-ink"];

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct RailLink {
    pub name: String,
    pub target: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct SavedLook {
    pub name: String,
    pub description: String,
    pub code: String,
    pub background: String,
    pub music: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct Profile {
    pub name: String,
    pub data: serde_json::Value,
}

fn clean_combo(value: &mut String) {
    clean_line(value, 40);
    if !value.chars().all(|c| c.is_ascii_alphanumeric() || "+,.;/-=[]'`\\".contains(c)) {
        value.clear();
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct MediaItem {
    pub path: String,
    pub name: String,
}

fn clean_media(list: &mut Vec<MediaItem>, legacy: &str, fallback: &str) {
    if list.is_empty() && !legacy.is_empty() {
        list.push(MediaItem { path: legacy.to_string(), name: fallback.to_string() });
    }
    let mut seen = std::collections::HashSet::new();
    for item in list.iter_mut() {
        clean_line(&mut item.path, 1024);
        clean_line(&mut item.name, 80);
        if item.name.is_empty() {
            item.name = fallback.to_string();
        }
    }
    list.retain(|item| !item.path.is_empty() && seen.insert(item.path.clone()));
    list.truncate(50);
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct InstanceGroup {
    pub name: String,
    pub icon: String,
    pub note: String,
    pub key: String,
    pub shared: Vec<String>,
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
            instance_groups: Vec::new(),
            pinned: String::new(),
            button_style: "pixel".into(),
            corners: "square".into(),
            panel_opacity: 100,
            bg_dim: 0,
            rail_labels: true,
            card_size: "comfy".into(),
            show_news: true,
            tours_seen: Vec::new(),
            content_width: "standard".into(),
            density: "comfy".into(),
            accent_bar: "thick".into(),
            bg_blur: 0,
            bg_motion: false,
            thin_scrollbars: false,
            card_details: true,
            show_brand: true,
            home_tagline: String::new(),
            bg_filter: "none".into(),
            home_layout: "standard".into(),
            rail_style: "glass".into(),
            cover_height: "normal".into(),
            cursor_style: "system".into(),
            click_sound: false,
            show_clock: false,
            bg_source: "auto".into(),
            bg_color: String::new(),
            bg_color2: String::new(),
            bg_pin: String::new(),
            bg_transition: "fade".into(),
            bg_pan: false,
            weather: "none".into(),
            clock_format: "24".into(),
            clock_seconds: false,
            clock_style: "bold".into(),
            clock_position: "right".into(),
            play_size: "normal".into(),
            play_text: String::new(),
            shard_layout: "grid".into(),
            hidden_shards: Vec::new(),
            show_credit: true,
            show_social: true,
            show_shard_stats: true,
            show_last_played: true,
            dark_from: 19,
            dark_to: 7,
            accent_from_bg: false,
            auto_palette: false,
            color_success: String::new(),
            color_warning: String::new(),
            color_link: String::new(),
            high_contrast: false,
            font_scale: 100,
            heading_style: "game".into(),
            card_view: "grid".into(),
            card_hover: "lift".into(),
            cover_shade: 35,
            instance_sort: "manual".into(),
            cover_position: "center".into(),
            page_stats: true,
            rail_side: "left".into(),
            rail_order: Vec::new(),
            rail_hidden: Vec::new(),
            hide_account_name: false,
            hidden_tools: Vec::new(),
            start_page: "home".into(),
            anim_speed: "normal".into(),
            click_sound_type: "block".into(),
            click_volume: 50,
            page_transition: "rise".into(),
            toast_position: "bottom-right".into(),
            toast_seconds: 6,
            launch_splash: false,
            custom_font: String::new(),
            fonts: Vec::new(),
            playlist: Vec::new(),
            music_mode: "loop".into(),
            shortcuts: Default::default(),
            global_shortcut: String::new(),
            rail_links: Vec::new(),
            profiles: Vec::new(),
            saved_looks: Vec::new(),
            streamer_mode: false,
            instance_order: Vec::new(),
            options_sync: false,
            options_sync_groups: crate::gameopts::GROUPS.iter().map(|g| g.to_string()).collect(),
            window_mode: "normal".into(),
            window_monitor: String::new(),
            remember_window: false,
            mod_snapshots: false,
            home_widgets: Vec::new(),
            home_note: String::new(),
            countdown_title: String::new(),
            countdown_at: 0,
            color_overrides: Default::default(),
            language: "zh-TW".into(),
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

pub const RAIL_VIEWS: &[&str] = &["home", "profile", "album", "instances", "explore", "accounts", "log", "settings", "about"];

pub static STREAMER: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn clean_hex(value: &mut String) {
    if !(value.len() == 7 && value.starts_with('#') && value[1..].chars().all(|c| c.is_ascii_hexdigit())) {
        value.clear();
    }
}

pub fn clean_list(list: &mut Vec<String>, allowed: &[&str]) {
    let mut seen = std::collections::HashSet::new();
    list.retain(|v| allowed.contains(&v.as_str()) && seen.insert(v.clone()));
}

pub fn clean_choice(value: &mut String, allowed: &[&str], fallback: &str) {
    if !allowed.contains(&value.as_str()) {
        *value = fallback.to_string();
    }
}

pub fn clean_jvm_args(value: &mut String) {
    *value = value.chars().take(4096).map(|c| if c.is_control() { ' ' } else { c }).collect::<String>();
    *value = crate::advanced::split_args(value)
        .into_iter()
        .filter(|t| t.starts_with('-') && t.len() > 1)
        .map(|t| if t.contains(char::is_whitespace) { format!("\"{t}\"") } else { t })
        .collect::<Vec<_>>()
        .join(" ");
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
        self.slide_seconds = self.slide_seconds.min(300);
        self.ui_scale = self.ui_scale.clamp(80, 125);
        self.music_volume = self.music_volume.min(100);
        clean_choice(&mut self.after_launch, &["hide", "keep", "exit"], "hide");
        clean_choice(&mut self.theme, &["auto", "light", "dark", "schedule", "background"], "auto");
        clean_choice(&mut self.font, &["pixel", "system", "naikai", "serif", "custom"], "pixel");
        clean_line(&mut self.custom_font, 1024);
        clean_media(&mut self.fonts, &self.custom_font, "自訂字型");
        if !self.fonts.iter().any(|f| f.path == self.custom_font) {
            self.custom_font = self.fonts.first().map(|f| f.path.clone()).unwrap_or_default();
        }
        if self.font == "custom" && self.custom_font.is_empty() {
            self.font = "pixel".into();
        }
        clean_choice(&mut self.music_mode, &["loop", "shuffle", "one"], "loop");
        self.shortcuts.retain(|k, _| k.len() <= 40 && k.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'));
        self.shortcuts.values_mut().for_each(clean_combo);
        while self.shortcuts.len() > 80 {
            let last = self.shortcuts.keys().last().cloned().unwrap_or_default();
            self.shortcuts.remove(&last);
        }
        clean_combo(&mut self.global_shortcut);
        for link in self.rail_links.iter_mut() {
            clean_line(&mut link.name, 12);
            clean_line(&mut link.target, 1024);
        }
        self.rail_links.retain(|l| {
            let lower = l.target.to_ascii_lowercase();
            !l.name.is_empty() && (lower.starts_with("https://") || lower.starts_with("http://") || !lower.contains("://") && !lower.starts_with("javascript:") && !lower.starts_with("data:"))
        });
        self.rail_links.retain(|l| !l.target.is_empty());
        self.rail_links.truncate(8);
        for profile in self.profiles.iter_mut() {
            clean_line(&mut profile.name, 24);
            if !profile.data.is_object() || profile.data.to_string().len() > 65536 {
                profile.data = serde_json::json!({});
            }
        }
        self.profiles.retain(|p| !p.name.is_empty());
        self.profiles.truncate(10);
        for look in self.saved_looks.iter_mut() {
            clean_line(&mut look.name, 24);
            clean_line(&mut look.description, 80);
            look.code.retain(|c| !c.is_whitespace());
            if !look.code.starts_with("BRLOOK1:") || look.code.len() > 16384 {
                look.code.clear();
            }
            clean_line(&mut look.background, 1024);
            let background = std::path::Path::new(&look.background);
            if !background.starts_with(crate::paths::root()) || background.components().any(|c| matches!(c, std::path::Component::ParentDir)) {
                look.background.clear();
            }
            look.music.retain(|track| {
                let path = std::path::Path::new(track);
                track.len() <= 1024 && path.starts_with(crate::paths::root()) && !path.components().any(|c| matches!(c, std::path::Component::ParentDir))
            });
            look.music.truncate(20);
        }
        self.saved_looks.retain(|l| !l.name.is_empty() && !l.code.is_empty());
        let mut seen = std::collections::HashSet::new();
        self.saved_looks.retain(|l| seen.insert(l.name.clone()));
        self.saved_looks.truncate(30);
        self.instance_order.retain(|id| id.len() <= 128);
        self.instance_order.dedup();
        self.instance_order.truncate(500);
        clean_list(&mut self.options_sync_groups, &crate::gameopts::GROUPS);
        clean_choice(&mut self.window_mode, &["normal", "borderless"], "normal");
        clean_line(&mut self.window_monitor, 64);
        let mut seen = std::collections::HashSet::new();
        self.home_widgets.retain(|w| WIDGETS.contains(&w.id.as_str()) && seen.insert(w.id.clone()));
        for w in self.home_widgets.iter_mut() {
            w.x = if w.x.is_finite() { w.x.clamp(0.0, 100.0) } else { 0.0 };
            w.y = if w.y.is_finite() { w.y.clamp(0.0, 100.0) } else { 0.0 };
            w.scale = if w.scale.is_finite() && w.scale > 0.0 { w.scale.clamp(0.5, 2.0) } else { 1.0 };
        }
        clean_text(&mut self.home_note, 500);
        clean_line(&mut self.countdown_title, 40);
        self.countdown_at = self.countdown_at.max(0);
        self.color_overrides.retain(|key, _| key.split_once('.').is_some_and(|(mode, token)| (mode == "light" || mode == "dark") && COLOR_TOKENS.contains(&token)));
        self.color_overrides.values_mut().for_each(clean_hex);
        self.color_overrides.retain(|_, v| !v.is_empty());
        clean_choice(&mut self.language, &["zh-TW", "zh-CN", "en"], "zh-TW");
        STREAMER.store(self.streamer_mode, std::sync::atomic::Ordering::Relaxed);
        clean_choice(&mut self.mod_channel, &["release", "beta", "alpha"], "release");
        clean_choice(&mut self.launcher_update, &["auto", "notify", "off"], "auto");
        clean_choice(&mut self.mod_update, &["auto", "keep"], "auto");
        clean_choice(&mut self.update_channel, &["stable", "beta", "alpha"], "stable");
        clean_choice(&mut self.button_style, &["pixel", "flat"], "pixel");
        clean_choice(&mut self.corners, &["square", "rounded"], "square");
        clean_choice(&mut self.card_size, &["comfy", "compact"], "comfy");
        self.panel_opacity = self.panel_opacity.clamp(60, 100);
        clean_choice(&mut self.content_width, &["standard", "wide", "full"], "standard");
        clean_choice(&mut self.density, &["comfy", "compact"], "comfy");
        clean_choice(&mut self.accent_bar, &["thick", "thin", "none"], "thick");
        self.bg_blur = self.bg_blur.min(20);
        clean_line(&mut self.home_tagline, 60);
        clean_choice(&mut self.bg_filter, &["none", "mono", "sepia", "cool", "warm", "vivid"], "none");
        clean_choice(&mut self.home_layout, &["standard", "mirrored"], "standard");
        clean_choice(&mut self.rail_style, &["glass", "solid", "accent"], "glass");
        clean_choice(&mut self.cover_height, &["short", "normal", "tall"], "normal");
        clean_choice(&mut self.cursor_style, &["system", "pixel"], "system");
        clean_choice(&mut self.bg_source, &["auto", "color"], "auto");
        for field in [&mut self.bg_color, &mut self.bg_color2, &mut self.color_success, &mut self.color_warning, &mut self.color_link] {
            clean_hex(field);
        }
        clean_line(&mut self.bg_pin, 1024);
        if !self.bg_pin.starts_with("https://") {
            self.bg_pin.clear();
        }
        clean_choice(&mut self.bg_transition, &["fade", "slide", "pixel", "none"], "fade");
        clean_choice(&mut self.weather, &["none", "snow", "rain", "sakura", "fireflies", "seasonal"], "none");
        clean_choice(&mut self.clock_format, &["24", "12"], "24");
        clean_choice(&mut self.clock_style, &["bold", "thin", "pixel"], "bold");
        clean_choice(&mut self.clock_position, &["right", "center"], "right");
        clean_choice(&mut self.play_size, &["normal", "large", "xl"], "normal");
        clean_line(&mut self.play_text, 12);
        clean_choice(&mut self.shard_layout, &["grid", "row"], "grid");
        self.hidden_shards.retain(|h| h.len() <= 64 && h.ends_with(".brilliantw.net"));
        self.hidden_shards.sort();
        self.hidden_shards.dedup();
        self.hidden_shards.truncate(16);
        self.dark_from = self.dark_from.min(23);
        self.dark_to = self.dark_to.min(23);
        self.font_scale = self.font_scale.clamp(90, 130);
        clean_choice(&mut self.heading_style, &["game", "modern"], "game");
        clean_choice(&mut self.card_view, &["grid", "list", "poster"], "grid");
        clean_choice(&mut self.card_hover, &["lift", "zoom", "none"], "lift");
        self.cover_shade = self.cover_shade.min(80);
        clean_choice(&mut self.instance_sort, &["manual", "recent", "name", "playtime", "created"], "manual");
        clean_choice(&mut self.cover_position, &["top", "center", "bottom"], "center");
        clean_choice(&mut self.rail_side, &["left", "right"], "left");
        clean_list(&mut self.rail_order, RAIL_VIEWS);
        clean_list(&mut self.rail_hidden, &["profile", "album", "explore", "accounts", "log"]);
        clean_list(&mut self.hidden_tools, &["walker-toggle", "font-toggle", "theme-toggle"]);
        clean_choice(&mut self.start_page, &["home", "instances", "last"], "home");
        clean_choice(&mut self.anim_speed, &["fast", "normal", "slow"], "normal");
        clean_choice(&mut self.click_sound_type, &["block", "button", "xp", "wood"], "block");
        self.click_volume = self.click_volume.min(100);
        clean_choice(&mut self.page_transition, &["rise", "fade", "slide", "none"], "rise");
        clean_choice(&mut self.toast_position, &["bottom-right", "top-right", "top-center"], "bottom-right");
        self.toast_seconds = self.toast_seconds.clamp(2, 15);
        self.bg_dim = self.bg_dim.min(80);
        self.tours_seen.retain(|t| t.len() <= 32 && t.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'));
        self.tours_seen.sort();
        self.tours_seen.dedup();
        self.tours_seen.truncate(30);
        clean_jvm_args(&mut self.jvm_args);
        clean_env_vars(&mut self.env_vars);
        for (field, max) in [(&mut self.java_path, 1024), (&mut self.pre_launch_cmd, 1024), (&mut self.post_exit_cmd, 1024), (&mut self.wrapper, 1024), (&mut self.proxy, 512), (&mut self.background, 1024), (&mut self.music, 1024), (&mut self.quick_join, 260)] {
            clean_line(field, max);
        }
        clean_media(&mut self.playlist, &self.music, "自訂音樂");
        if !self.music.is_empty() && !self.playlist.iter().any(|m| m.path == self.music) {
            self.music = self.playlist.first().map(|m| m.path.clone()).unwrap_or_default();
        }
        if !self.quick_join.is_empty() && !valid_host(&self.quick_join) {
            self.quick_join.clear();
        }
        clean_hex(&mut self.accent);
        for group in self.instance_groups.iter_mut() {
            clean_line(&mut group.name, 24);
            clean_line(&mut group.icon, 1024);
            clean_line(&mut group.note, 120);
            group.key.retain(|c| c.is_ascii_alphanumeric() || c == '-');
            group.key.truncate(32);
            if group.key.is_empty() {
                group.key = format!("{:016x}", rand::random::<u64>());
            }
            group.shared.retain(|f| crate::advanced::SHAREABLE.contains(&f.as_str()));
            group.shared.sort();
            group.shared.dedup();
        }
        let mut seen = std::collections::HashSet::new();
        self.instance_groups.retain(|g| !g.name.is_empty() && seen.insert(g.name.clone()));
        self.instance_groups.truncate(100);
        clean_line(&mut self.pinned, 80);
        for (field, max) in [(&mut self.selected_account, 128), (&mut self.selected_instance, 128), (&mut self.last_announcement, 256)] {
            clean_line(field, max);
        }
        let extra = self.notified.len().saturating_sub(30);
        self.notified.drain(..extra);
        self.notified.iter_mut().for_each(|n| clean_line(n, 256));
        self.my_reports.retain(|r| r.is_object() && r.to_string().len() <= 4096);
        self.my_reports.truncate(20);
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
    pub retry_at: i64,
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
            let keep = path.with_extension("broken.json");
            if !keep.exists() {
                let _ = std::fs::copy(path, &keep);
            }
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
    let missing_key = settings.instance_groups.iter().any(|g| g.key.is_empty());
    settings.clean();
    if missing_key {
        let _ = write_json(&paths::config_file(), &settings).await;
    }
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
