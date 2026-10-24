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
        for (field, max) in [(&mut self.java_path, 1024), (&mut self.wrapper, 1024), (&mut self.image, 1024), (&mut self.icon, 1024), (&mut self.group, 24), (&mut self.account, 64), (&mut self.active_profile, 24)] {
            clean_line(field, max);
        }
        if !["vanilla", "fabric"].contains(&self.loader.as_str()) {
            self.loader = "vanilla".into();
        }
        for field in [&mut self.mc_version, &mut self.loader_version, &mut self.version_id] {
            if !field.is_empty() && !paths::valid_version_id(field) {
                field.clear();
            }
        }
        self.pinned.retain(|p| p.len() <= 64 && p.chars().all(|c| c.is_ascii_alphanumeric()));
        self.pinned.truncate(1000);
        self.mod_profiles.truncate(30);
        for profile in self.mod_profiles.iter_mut() {
            clean_line(&mut profile.name, 24);
            profile.enabled.truncate(2000);
            profile.known.truncate(2000);
        }
        self.mod_profiles.retain(|p| !p.name.is_empty());
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
    instance.name = unique_name(&instance.name).await;
    let base = make_id(&instance.name);
    instance.id = base.clone();
    for n in 2.. {
        if !paths::instance_dir(&instance.id).exists() {
            break;
        }
        instance.id = format!("{base}-{n}");
    }
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
    let name = get(id).await.map(|i| i.name).unwrap_or_else(|_| id.to_string());
    let folder = id.to_string();
    let game = paths::game_dir(id);
    tokio::task::spawn_blocking(move || {
        crate::advanced::unlink_shared(&game)?;
        paths::discard(&format!("實例 {name}"), &[(&dir, folder.as_str())])
    })
        .await
        .context("刪除工作中斷")?
        .with_context(|| format!("移到回收資料夾失敗：{}", paths::instance_dir(id).display()))?;
    Ok(())
}

pub async fn unique_name(base: &str) -> String {
    let taken: Vec<String> = list().await.into_iter().map(|i| i.name.trim().to_string()).collect();
    pick_unique(base, &taken)
}

fn pick_unique(base: &str, taken: &[String]) -> String {
    let base = base.trim();
    if !taken.iter().any(|n| n == base) {
        return base.to_string();
    }
    (2..).map(|n| format!("{base} ({n})")).find(|n| !taken.contains(n)).unwrap_or_default()
}

pub async fn duplicate(id: &str, new_name: &str) -> R<Instance> {
    let source = get(id).await?;
    let new_name = new_name.trim();
    if new_name.is_empty() {
        bail!("請輸入新實例的名稱");
    }
    if list().await.iter().any(|i| i.name.trim() == new_name) {
        bail!("已經有叫「{new_name}」的實例了，複本請取一個不同的名稱");
    }
    let mut copy = source.clone();
    copy.name = new_name.to_string();
    copy.last_played = 0;
    copy.play_seconds = 0;
    let mut copy = create(copy).await?;
    if let Err(error) = copy_dir(&paths::game_dir(id), &paths::game_dir(&copy.id)).await {
        let _ = tokio::fs::remove_dir_all(paths::instance_dir(&copy.id)).await;
        return Err(error.0.context(format!("複製「{}」失敗，已收回未完成的複本", source.name)).into());
    }
    let image = std::path::Path::new(&source.image);
    if let Some(name) = image.file_name().filter(|_| image.is_file()) {
        let dest = paths::instance_dir(&copy.id).join(name);
        if tokio::fs::copy(image, &dest).await.is_ok() {
            copy.image = dest.to_string_lossy().to_string();
            save(&copy).await?;
        }
    }
    let settings = crate::config::load_settings().await;
    let linked = copy.clone();
    let _ = tokio::task::spawn_blocking(move || crate::advanced::sync_shared(&linked, &settings)).await;
    Ok(copy)
}

async fn copy_dir(from: &std::path::Path, to: &std::path::Path) -> R<()> {
    let mut stack = vec![(from.to_path_buf(), to.to_path_buf())];
    while let Some((src, dst)) = stack.pop() {
        tokio::fs::create_dir_all(&dst).await?;
        let mut entries = tokio::fs::read_dir(&src).await?;
        while let Some(entry) = entries.next_entry().await? {
            let target = dst.join(entry.file_name());
            let kind = entry.file_type().await.with_context(|| format!("讀取 {} 失敗", entry.path().display()))?;
            if kind.is_symlink() {
                continue;
            }
            if kind.is_dir() {
                stack.push((entry.path(), target));
            } else {
                tokio::fs::copy(entry.path(), &target).await.with_context(|| format!("複製 {} 失敗", entry.path().display()))?;
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
        if to.exists() {
            bail!("模組資料夾裡同時有「{stem}」的啟用與停用兩份檔案，請先移除其中一份");
        }
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

pub fn content_dest(id: &str, source: &str) -> R<(&'static str, String, std::path::PathBuf)> {
    let src = std::path::Path::new(source);
    let (kind, dir) = content_kind(src)?;
    let name = src.file_name().map(|s| s.to_string_lossy().to_string()).context("檔名不正確")?;
    if name.contains(['/', '\\']) || name.contains("..") || name.is_empty() {
        bail!("無效的檔名：{name}");
    }
    let dest = paths::game_dir(id).join(dir).join(&name);
    Ok((kind, name, dest))
}

pub fn content_exists(dest: &std::path::Path) -> bool {
    dest.exists() || std::path::PathBuf::from(format!("{}.disabled", dest.display())).exists()
}

pub async fn add_content(id: &str, source: &str) -> R<(String, String)> {
    let src = std::path::Path::new(source);
    let (kind, name, dest) = content_dest(id, source)?;
    if content_exists(&dest) {
        bail!("「{name}」已經存在，請先移除舊的或改個檔名");
    }
    if let Some(parent) = dest.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    tokio::fs::copy(src, &dest).await.with_context(|| format!("複製失敗：{source}"))?;
    Ok((kind.to_string(), name))
}

#[cfg(test)]
mod content_kind_tests {
    use super::*;

    #[cfg(windows)]
    #[tokio::test]
    async fn copy_skips_shared_links() {
        let root = std::env::temp_dir().join(format!("copy-links-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let shared = root.join("shared");
        let game = root.join("game");
        std::fs::create_dir_all(&shared).unwrap();
        std::fs::write(shared.join("pack.zip"), "x").unwrap();
        std::fs::create_dir_all(game.join("mods")).unwrap();
        std::fs::write(game.join("mods/a.jar"), "y").unwrap();
        let status = std::process::Command::new("cmd").arg("/C").arg("mklink").arg("/J").arg(game.join("resourcepacks")).arg(&shared).stdout(std::process::Stdio::null()).status().unwrap();
        assert!(status.success());
        let copy = root.join("copy");
        copy_dir(&game, &copy).await.unwrap();
        assert!(copy.join("mods/a.jar").is_file());
        assert!(!copy.join("resourcepacks").exists());
        assert!(shared.join("pack.zip").is_file());
        std::fs::remove_dir(game.join("resourcepacks")).unwrap();
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    #[ignore]
    async fn duplicate_and_delete_inside_shared_group() {
        let root = std::env::temp_dir().join(format!("brilliant-dup-share-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::env::set_var("BRILLIANT_LAUNCHER_HOME", &root);
        let mut settings = crate::config::Settings::default();
        settings.instance_groups = vec![crate::config::InstanceGroup { name: "輝煌".into(), key: "k1".into(), shared: vec!["resourcepacks".into(), "saves".into(), "screenshots".into()], ..Default::default() }];
        crate::config::save_settings(&settings).await.unwrap();
        let source = create(Instance { name: "輝煌生存".into(), mc_version: "26.3".into(), group: "輝煌".into(), ..Default::default() }).await.unwrap();
        crate::advanced::sync_shared(&source, &settings).unwrap();
        let shared = crate::advanced::group_base("k1");
        std::fs::write(shared.join("resourcepacks/Faithful.zip"), "pack").unwrap();
        std::fs::write(paths::game_dir(&source.id).join("options.txt"), "fov:1").unwrap();
        assert!(duplicate(&source.id, "輝煌生存").await.is_err());
        let copy = duplicate(&source.id, "輝煌生存 複本").await.unwrap();
        let game = paths::game_dir(&copy.id);
        assert_eq!(std::fs::read_to_string(game.join("options.txt")).unwrap(), "fov:1");
        assert!(crate::advanced::is_link(&game.join("resourcepacks")) && game.join("resourcepacks/Faithful.zip").is_file());
        std::fs::write(shared.join("screenshots/2026-10-24_10.00.00.png"), "png").unwrap();
        assert_eq!(crate::insight::list_all_screenshots().await.len(), 1);
        delete(&copy.id).await.unwrap();
        assert!(!paths::instance_dir(&copy.id).exists());
        assert!(shared.join("resourcepacks/Faithful.zip").is_file());
        let binned = paths::trash().join("實例 輝煌生存 複本").join(&copy.id).join(".minecraft");
        assert!(binned.join("options.txt").is_file() && !binned.join("resourcepacks").exists());
        let mut no_groups = settings.clone();
        no_groups.instance_groups.clear();
        crate::config::save_settings(&no_groups).await.unwrap();
        assert_eq!(crate::advanced::shared_discard("k1".into(), "輝煌".into()).await.unwrap(), "busy");
        let left = Instance { group: String::new(), ..get(&source.id).await.unwrap() };
        crate::advanced::sync_shared(&left, &no_groups).unwrap();
        assert!(paths::game_dir(&source.id).join("resourcepacks/Faithful.zip").is_file());
        assert_eq!(crate::advanced::shared_discard("k1".into(), "輝煌".into()).await.unwrap(), "moved");
        assert!(!shared.exists() && paths::trash().join("分組「輝煌」的共用資料夾").join("shared/resourcepacks/Faithful.zip").is_file());
        std::env::remove_var("BRILLIANT_LAUNCHER_HOME");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn names_never_repeat() {
        let taken: Vec<String> = ["輝煌 26.3", "輝煌 26.3 (2)", "原版"].map(String::from).to_vec();
        assert_eq!(pick_unique("輝煌 26.3", &taken), "輝煌 26.3 (3)");
        assert_eq!(pick_unique(" 原版 ", &taken), "原版 (2)");
        assert_eq!(pick_unique("建築", &taken), "建築");
    }
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
        s.jvm_args = r#"-Xmx2G -Djava.io.tmpdir="D:\My Folder" stray"#.into();
        s.clean();
        assert_eq!(s.jvm_args, r#"-Xmx2G "-Djava.io.tmpdir=D:\My Folder""#);
        assert_eq!(crate::advanced::split_args(&s.jvm_args), vec!["-Xmx2G", r"-Djava.io.tmpdir=D:\My Folder"]);
        assert_eq!(s.env_vars, "# 註解\nGOOD=1\n_OK=x");
        assert_eq!(s.shared_folders, vec!["saves".to_string()]);
        s.button_style = "neon".into();
        s.corners = "x".into();
        s.card_size = "huge".into();
        s.panel_opacity = 5;
        s.bg_dim = 99;
        s.tours_seen = vec!["instances".into(), "instances".into(), "../x".into()];
        s.content_width = "huge".into();
        s.density = "tiny".into();
        s.accent_bar = "rainbow".into();
        s.bg_blur = 99;
        s.bg_filter = "invert".into();
        s.home_layout = "upside".into();
        s.rail_style = "neon".into();
        s.cover_height = "giant".into();
        s.cursor_style = "url(x)".into();
        s.home_tagline = format!("我的伺服器\n{}", "長".repeat(80));
        s.clean();
        assert_eq!((s.button_style.as_str(), s.corners.as_str(), s.card_size.as_str(), s.panel_opacity, s.bg_dim), ("pixel", "square", "comfy", 60, 80));
        assert_eq!(s.tours_seen, vec!["instances".to_string()]);
        assert_eq!((s.content_width.as_str(), s.density.as_str(), s.accent_bar.as_str(), s.bg_blur), ("standard", "comfy", "thick", 20));
        assert_eq!((s.bg_filter.as_str(), s.home_layout.as_str(), s.rail_style.as_str(), s.cover_height.as_str(), s.cursor_style.as_str()), ("none", "standard", "glass", "normal", "system"));
        assert!(!s.home_tagline.contains('\n') && s.home_tagline.chars().count() <= 60 && s.home_tagline.starts_with("我的伺服器"));
        s.notified = (0..100).map(|n| n.to_string()).collect();
        s.my_reports = (0..50).map(|n| serde_json::json!({ "id": n })).chain([serde_json::json!("x"), serde_json::json!({ "big": "a".repeat(5000) })]).collect();
        s.clean();
        assert_eq!((s.notified.len(), s.notified[0].as_str(), s.my_reports.len()), (30, "70", 20));
        let mut ok = Settings { quick_join: "s2.brilliantw.net:25565".into(), accent: "#ff9ac8".into(), ..Default::default() };
        ok.clean();
        assert_eq!((ok.quick_join.as_str(), ok.accent.as_str()), ("s2.brilliantw.net:25565", "#ff9ac8"));
        let mut i = Instance { name: "\u{7}\u{7}".into(), width: 99_999, height: 0, memory_mb: 100, java_major: 3, mod_channel: "evil".into(), ..Default::default() };
        i.clean();
        assert_eq!((i.name.as_str(), i.width, i.height, i.memory_mb, i.java_major, i.mod_channel.as_str()), ("未命名實例", 0, 0, 1024, 0, ""));
        let mut odd = Instance { loader: "forge".into(), mc_version: "../../x".into(), loader_version: "0.19.5".into(), pinned: vec!["P7dR8mSH".into(), "../x".into()], ..Default::default() };
        odd.mod_profiles = (0..40).map(|n| ModProfile { name: if n == 0 { String::new() } else { format!("組合{n}") }, ..Default::default() }).collect();
        odd.clean();
        assert_eq!((odd.loader.as_str(), odd.mc_version.as_str(), odd.loader_version.as_str()), ("vanilla", "", "0.19.5"));
        assert_eq!((odd.pinned.clone(), odd.mod_profiles.len()), (vec!["P7dR8mSH".to_string()], 29));
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
