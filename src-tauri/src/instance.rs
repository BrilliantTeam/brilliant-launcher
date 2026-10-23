use crate::config::{now_secs, Settings};
use crate::err::R;
use crate::paths;
use crate::bail;
use anyhow::Context;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct ModProfile {
    pub name: String,
    pub enabled: Vec<String>,
    pub known: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct Instance {
    pub id: String,
    pub name: String,
    pub icon: String,
    pub mc_version: String,
    pub loader: String,
    pub loader_version: String,
    pub version_id: String,
    pub memory_mb: u32,
    pub jvm_args: String,
    pub width: u32,
    pub height: u32,
    pub fullscreen: bool,
    pub java_path: String,
    pub image: String,
    pub pinned: Vec<String>,
    pub mod_profiles: Vec<ModProfile>,
    pub active_profile: String,
    pub inject_rich_presence: Option<bool>,
    pub general: bool,
    pub created_at: i64,
    pub last_played: i64,
    pub play_seconds: i64,
    pub java_major: u32,
    pub env_vars: String,
    pub wrapper: String,
    pub use_shared: bool,
    pub mod_channel: String,
    pub freeze_mods: bool,
    pub account: String,
    pub group: String,
}

impl Instance {
    pub fn clean(&mut self) {
        use crate::config::{clean_env_vars, clean_jvm_args, clean_line, clean_size, HEIGHT_RANGE, MEMORY_RANGE, WIDTH_RANGE};
        clean_line(&mut self.name, 40);
        if self.name.is_empty() {
            self.name = "未命名實例".into();
        }
        clean_size(&mut self.memory_mb, MEMORY_RANGE, true);
        clean_size(&mut self.width, WIDTH_RANGE, true);
        clean_size(&mut self.height, HEIGHT_RANGE, true);
        if self.width == 0 || self.height == 0 {
            self.width = 0;
            self.height = 0;
        }
        if ![0, 8, 16, 17, 21, 25].contains(&self.java_major) {
            self.java_major = 0;
        }
        if !["", "release", "beta", "alpha"].contains(&self.mod_channel.as_str()) {
            self.mod_channel.clear();
        }
        clean_jvm_args(&mut self.jvm_args);
        clean_env_vars(&mut self.env_vars);
        for (field, max) in [(&mut self.java_path, 1024), (&mut self.wrapper, 1024), (&mut self.image, 1024), (&mut self.icon, 1024), (&mut self.group, 24), (&mut self.account, 64)] {
            clean_line(field, max);
        }
    }

    pub fn memory(&self, s: &Settings) -> u32 {
        if self.memory_mb > 0 {
            self.memory_mb
        } else {
            s.memory_mb
        }
    }
    pub fn jvm(&self, s: &Settings) -> String {
        if self.jvm_args.trim().is_empty() {
            s.jvm_args.clone()
        } else {
            self.jvm_args.clone()
        }
    }
    pub fn java<'a>(&'a self, s: &'a Settings) -> &'a str {
        if self.java_path.trim().is_empty() { &s.java_path } else { &self.java_path }
    }
    pub fn resolution(&self, s: &Settings) -> (u32, u32, bool) {
        if self.width > 0 && self.height > 0 {
            (self.width, self.height, self.fullscreen)
        } else {
            (s.width, s.height, s.fullscreen)
        }
    }
    pub fn channel<'a>(&'a self, s: &'a Settings) -> &'a str {
        let own = self.mod_channel.trim();
        let global = s.mod_channel.trim();
        if !own.is_empty() {
            own
        } else if !global.is_empty() {
            global
        } else {
            "release"
        }
    }
    pub fn env(&self, s: &Settings) -> Vec<(String, String)> {
        let mut vars = crate::advanced::parse_env(&s.env_vars);
        for (key, value) in crate::advanced::parse_env(&self.env_vars) {
            vars.retain(|(k, _)| k != &key);
            vars.push((key, value));
        }
        vars
    }
    pub fn wrapper_cmd(&self, s: &Settings) -> Vec<String> {
        let own = self.wrapper.trim();
        crate::advanced::split_args(if own.is_empty() { &s.wrapper } else { own })
    }
    pub fn wants_rich_presence(&self, s: &Settings) -> bool {
        !self.general && self.loader == "fabric" && self.inject_rich_presence.unwrap_or(s.inject_rich_presence)
    }
}

fn make_id(name: &str) -> String {
    let slug: String = name
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '-' })
        .collect();
    let slug = slug.trim_matches('-').to_string();
    let head = if slug.is_empty() { "instance".into() } else { slug };
    format!("{}-{}", &head[..head.len().min(24)], now_secs())
}

pub async fn list() -> Vec<Instance> {
    let Ok(mut entries) = tokio::fs::read_dir(paths::instances()).await else {
        return Vec::new();
    };
    let mut out = Vec::new();
    while let Ok(Some(entry)) = entries.next_entry().await {
        let file = entry.path().join("instance.json");
        if let Ok(bytes) = tokio::fs::read(&file).await {
            match serde_json::from_slice::<Instance>(&bytes).map(|mut i| { i.clean(); i }) {
                Ok(i) => out.push(i),
                Err(e) => eprintln!("[instance] {} 解析失敗：{e}", file.display()),
            }
        }
    }
    out.sort_by(|a, b| b.last_played.cmp(&a.last_played).then(a.name.cmp(&b.name)));
    out
}

pub async fn get(id: &str) -> R<Instance> {
    let bytes = tokio::fs::read(paths::instance_json(id))
        .await
        .with_context(|| format!("找不到實例 {id}"))?;
    let mut instance: Instance = serde_json::from_slice(&bytes).with_context(|| format!("實例 {id} 設定檔損毀"))?;
    instance.clean();
    Ok(instance)
}

pub async fn save(instance: &Instance) -> R<()> {
    let mut instance = instance.clone();
    instance.clean();
    let bytes = serde_json::to_vec_pretty(&instance)?;
    crate::net::write_file(&paths::instance_json(&instance.id), &bytes, false).await
}

pub async fn create(mut instance: Instance) -> R<Instance> {
    if instance.name.trim().is_empty() {
        bail!("實例名稱不能空白");
    }
    instance.id = make_id(&instance.name);
    instance.created_at = now_secs();
    if instance.icon.is_empty() {
        instance.icon = "zap".into();
    }
    tokio::fs::create_dir_all(paths::game_dir(&instance.id).join("mods")).await?;
    save(&instance).await?;
    Ok(instance)
}

pub async fn delete(id: &str) -> R<()> {
    let dir = paths::instance_dir(id);
    if !dir.starts_with(paths::instances()) || !dir.join("instance.json").is_file() {
        bail!("不是有效的實例資料夾：{}", dir.display());
    }
    tokio::fs::remove_dir_all(&dir)
        .await
        .with_context(|| format!("刪除失敗：{}", dir.display()))?;
    Ok(())
}

pub async fn duplicate(id: &str, new_name: &str) -> R<Instance> {
    let source = get(id).await?;
    let mut copy = source.clone();
    copy.name = new_name.to_string();
    copy.last_played = 0;
    copy.play_seconds = 0;
    let mut copy = create(copy).await?;
    copy_dir(&paths::game_dir(id), &paths::game_dir(&copy.id)).await?;
    let image = std::path::Path::new(&source.image);
    if let Some(name) = image.file_name().filter(|_| image.is_file()) {
        let dest = paths::instance_dir(&copy.id).join(name);
        if tokio::fs::copy(image, &dest).await.is_ok() {
            copy.image = dest.to_string_lossy().to_string();
            save(&copy).await?;
        }
    }
    Ok(copy)
}

async fn copy_dir(from: &std::path::Path, to: &std::path::Path) -> R<()> {
    let mut stack = vec![(from.to_path_buf(), to.to_path_buf())];
    while let Some((src, dst)) = stack.pop() {
        tokio::fs::create_dir_all(&dst).await?;
        let mut entries = tokio::fs::read_dir(&src).await?;
        while let Some(entry) = entries.next_entry().await? {
            let target = dst.join(entry.file_name());
            if entry.file_type().await?.is_dir() {
                stack.push((entry.path(), target));
            } else {
                tokio::fs::copy(entry.path(), target).await?;
            }
        }
    }
    Ok(())
}

pub const DISABLED_SUFFIX: &str = ".disabled";

fn mod_path(id: &str, file: &str) -> R<std::path::PathBuf> {
    if file.contains(['/', '\\']) || file.contains("..") || file.is_empty() {
        bail!("無效的檔名：{file}");
    }
    Ok(paths::game_dir(id).join("mods").join(file))
}

pub async fn set_mod_enabled(id: &str, file: &str, enabled: bool) -> R<()> {
    let from = mod_path(id, file)?;
    let stem = file.trim_end_matches(DISABLED_SUFFIX);
    let to = mod_path(id, &if enabled { stem.to_string() } else { format!("{stem}{DISABLED_SUFFIX}") })?;
    if from != to {
        tokio::fs::rename(&from, &to).await.context("切換模組狀態失敗")?;
    }
    Ok(())
}

pub fn content_kind(source: &std::path::Path) -> R<(&'static str, &'static str)> {
    let lower = source.file_name().map(|s| s.to_string_lossy().to_lowercase()).unwrap_or_default();
    if lower.ends_with(".jar") {
        return Ok(("mod", "mods"));
    }
    if !lower.ends_with(".zip") {
        bail!("只能安裝 .jar 模組、.zip 資源包或光影：{}", source.display());
    }
    let file = std::fs::File::open(source).with_context(|| format!("無法開啟 {}", source.display()))?;
    let zip = zip::ZipArchive::new(std::io::BufReader::new(file)).with_context(|| format!("無法讀取壓縮檔 {}", source.display()))?;
    let names: Vec<String> = zip.file_names().map(|n| n.replace('\\', "/").to_lowercase()).collect();
    if names.iter().any(|n| n.starts_with("shaders/")) {
        return Ok(("shader", "shaderpacks"));
    }
    if names.iter().any(|n| n == "pack.mcmeta") {
        return Ok(("resourcepack", "resourcepacks"));
    }
    bail!("{} 不是資源包也不是光影（找不到 pack.mcmeta 或 shaders 資料夾）", source.display())
}

pub async fn add_content(id: &str, source: &str) -> R<(String, String)> {
    let src = std::path::Path::new(source);
    let (kind, dir) = content_kind(src)?;
    let name = src.file_name().map(|s| s.to_string_lossy().to_string()).context("檔名不正確")?;
    if name.contains(['/', '\\']) || name.contains("..") || name.is_empty() {
        bail!("無效的檔名：{name}");
    }
    let dest = paths::game_dir(id).join(dir).join(&name);
    if let Some(parent) = dest.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    tokio::fs::copy(src, &dest).await.with_context(|| format!("複製失敗：{source}"))?;
    Ok((kind.to_string(), name))
}

#[cfg(test)]
mod content_kind_tests {
    use super::*;
    use std::io::Write;

    fn zip_with(path: &std::path::Path, entries: &[&str]) {
        let mut zip = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
        for name in entries {
            zip.start_file(*name, zip::write::SimpleFileOptions::default()).unwrap();
            zip.write_all(b"x").unwrap();
        }
        zip.finish().unwrap();
    }

    #[test]
    fn detects_packs_and_shaders() {
        let dir = std::env::temp_dir().join(format!("brilliant-kind-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let pack = dir.join("CozyUI.zip");
        let shader = dir.join("Complementary.zip");
        let junk = dir.join("photos.zip");
        zip_with(&pack, &["pack.mcmeta", "assets/minecraft/textures/a.png"]);
        zip_with(&shader, &["shaders/gbuffers.fsh", "shaders/lang/en_US.lang"]);
        zip_with(&junk, &["a.jpg"]);
        assert_eq!(content_kind(&pack).unwrap().0, "resourcepack");
        assert_eq!(content_kind(&shader).unwrap().0, "shader");
        assert_eq!(content_kind(&dir.join("sodium.jar")).unwrap().0, "mod");
        assert!(content_kind(&junk).is_err());
        assert!(content_kind(&dir.join("notes.txt")).is_err());
        let _ = std::fs::remove_dir_all(dir);
    }
}

#[cfg(test)]
mod general_tests {
    use super::*;

    #[test]
    fn abnormal_input_is_clamped() {
        let mut s = Settings { width: 1_000_000, height: 2000, memory_mb: 9_999_999, memory_min_mb: 99_999, ui_scale: 5, download_threads: 0, backup_keep: 0, music_volume: 900, ..Default::default() };
        s.theme = "<script>".into();
        s.after_launch = "rm -rf".into();
        s.quick_join = "s1.brilliantw.net:99999".into();
        s.accent = "red; background:url(x)".into();
        s.jvm_args = "-Xss4m hello\0 -Dx=1\nabc".into();
        s.env_vars = "# 註解\nGOOD=1\n1BAD=2\nA B=3\nNOEQ\n_OK=x".into();
        s.shared_folders = vec!["saves".into(), "../../Windows".into()];
        s.clean();
        assert_eq!((s.width, s.height, s.memory_mb, s.memory_min_mb), (7680, 2000, 32768, 32768));
        assert_eq!((s.ui_scale, s.download_threads, s.backup_keep, s.music_volume), (80, 1, 1, 100));
        assert_eq!((s.theme.as_str(), s.after_launch.as_str(), s.quick_join.as_str(), s.accent.as_str()), ("auto", "hide", "", ""));
        assert_eq!(s.jvm_args, "-Xss4m -Dx=1");
        assert_eq!(s.env_vars, "# 註解\nGOOD=1\n_OK=x");
        assert_eq!(s.shared_folders, vec!["saves".to_string()]);
        let mut ok = Settings { quick_join: "s2.brilliantw.net:25565".into(), accent: "#ff9ac8".into(), ..Default::default() };
        ok.clean();
        assert_eq!((ok.quick_join.as_str(), ok.accent.as_str()), ("s2.brilliantw.net:25565", "#ff9ac8"));
        let mut i = Instance { name: "\u{7}\u{7}".into(), width: 99_999, height: 0, memory_mb: 100, java_major: 3, mod_channel: "evil".into(), ..Default::default() };
        i.clean();
        assert_eq!((i.name.as_str(), i.width, i.height, i.memory_mb, i.java_major, i.mod_channel.as_str()), ("未命名實例", 0, 0, 1024, 0, ""));
        let mut keep = Instance { name: "  我的世界  ".into(), memory_mb: 0, width: 1280, height: 720, java_major: 21, ..Default::default() };
        keep.clean();
        assert_eq!((keep.name.as_str(), keep.memory_mb, keep.width, keep.height, keep.java_major), ("我的世界", 0, 1280, 720, 21));
    }

    #[test]
    fn general_instances_skip_brilliant_features() {
        let settings = Settings::default();
        let mut instance = Instance { loader: "fabric".into(), ..Default::default() };
        assert!(instance.wants_rich_presence(&settings));
        instance.general = true;
        assert!(!instance.wants_rich_presence(&settings));
        assert!(!crate::boost::wants_packs(&instance, &settings));
        instance.inject_rich_presence = Some(true);
        assert!(!instance.wants_rich_presence(&settings), "一般實例就算手動開啟也不安裝輝煌模組");
        let old: Instance = serde_json::from_str(r#"{"id":"a","name":"舊","loader":"fabric"}"#).unwrap();
        assert!(!old.general, "舊實例要視為輝煌實例");
    }
}
