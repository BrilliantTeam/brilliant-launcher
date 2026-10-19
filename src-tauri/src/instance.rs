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
}

impl Instance {
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
            match serde_json::from_slice::<Instance>(&bytes) {
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
    Ok(serde_json::from_slice(&bytes).with_context(|| format!("實例 {id} 設定檔損毀"))?)
}

pub async fn save(instance: &Instance) -> R<()> {
    let bytes = serde_json::to_vec_pretty(instance)?;
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

pub async fn add_mod(id: &str, source: &str) -> R<String> {
    let src = std::path::Path::new(source);
    let name = src
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .filter(|n| n.ends_with(".jar"))
        .context("只能安裝 .jar 檔")?;
    let dest = mod_path(id, &name)?;
    if let Some(parent) = dest.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    tokio::fs::copy(src, &dest).await.with_context(|| format!("複製失敗：{source}"))?;
    Ok(name)
}

#[cfg(test)]
mod general_tests {
    use super::*;

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
