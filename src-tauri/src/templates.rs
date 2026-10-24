use crate::err::R;
use crate::instance::{self, Instance};
use crate::paths;
use crate::bail;
use anyhow::Context;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

const PARTS: [&str; 6] = ["mods", "config", "resourcepacks", "shaderpacks", "options.txt", "servers.dat"];

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct Template {
    pub key: String,
    pub name: String,
    pub created_at: i64,
    pub instance: Instance,
}

fn root() -> PathBuf {
    paths::root().join("templates")
}

fn valid_key(key: &str) -> bool {
    !key.is_empty() && key.len() <= 32 && key.chars().all(|c| c.is_ascii_alphanumeric())
}

fn copy_parts(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for part in PARTS {
        let src = from.join(part);
        if src.is_dir() {
            crate::importer::copy_tree(&src, &to.join(part))?;
        } else if src.is_file() {
            std::fs::copy(&src, to.join(part))?;
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn list_templates() -> Vec<Template> {
    let mut list: Vec<Template> = std::fs::read_dir(root())
        .map(|d| d.flatten().collect::<Vec<_>>())
        .unwrap_or_default()
        .into_iter()
        .filter_map(|e| serde_json::from_str::<Template>(&std::fs::read_to_string(e.path().join("template.json")).ok()?).ok())
        .filter(|t| valid_key(&t.key))
        .collect();
    list.sort_by_key(|t| std::cmp::Reverse(t.created_at));
    list
}

#[tauri::command]
pub async fn save_template(id: String, name: String) -> R<Template> {
    let source = instance::get(&id).await?;
    let mut name = name;
    crate::config::clean_line(&mut name, 40);
    if name.is_empty() {
        bail!("請輸入範本名稱");
    }
    if list_templates().await.iter().any(|t| t.name == name) {
        bail!("已經有叫「{name}」的範本了，請換一個名稱");
    }
    let key = format!("{:016x}", rand::random::<u64>());
    let dir = root().join(&key);
    let mut snapshot = source.clone();
    snapshot.id.clear();
    snapshot.last_played = 0;
    snapshot.play_seconds = 0;
    let image = PathBuf::from(&source.image);
    if let Some(file) = image.file_name().filter(|_| image.is_file()) {
        snapshot.image = dir.join(file).to_string_lossy().to_string();
    } else {
        snapshot.image.clear();
    }
    let template = Template { key: key.clone(), name, created_at: crate::config::now_secs(), instance: snapshot };
    let game = paths::game_dir(&id);
    let target = dir.clone();
    let json = serde_json::to_string_pretty(&template)?;
    let result = tokio::task::spawn_blocking(move || -> std::io::Result<()> {
        copy_parts(&game, &target.join("game"))?;
        if image.is_file() {
            if let Some(file) = image.file_name() {
                std::fs::copy(&image, target.join(file))?;
            }
        }
        paths::write_atomic(&target.join("template.json"), json.as_bytes())
    })
    .await
    .context("儲存範本中斷")?;
    if let Err(error) = result {
        let _ = std::fs::remove_dir_all(&dir);
        return Err(anyhow::Error::from(error).context("儲存範本失敗").into());
    }
    Ok(template)
}

#[tauri::command]
pub async fn create_from_template(key: String, name: String) -> R<Instance> {
    if !valid_key(&key) {
        bail!("範本不存在");
    }
    let dir = root().join(&key);
    let template: Template = serde_json::from_str(&std::fs::read_to_string(dir.join("template.json")).context("範本不存在")?).context("範本內容損毀")?;
    let mut draft = template.instance.clone();
    draft.name = if name.trim().is_empty() { template.name.clone() } else { name.trim().to_string() };
    let cover = PathBuf::from(&draft.image);
    draft.image.clear();
    draft.group.clear();
    let mut created = instance::create(draft).await?;
    let id = created.id.clone();
    let game = paths::game_dir(&id);
    let from = dir.join("game");
    let copied = tokio::task::spawn_blocking(move || copy_parts(&from, &game)).await.context("建立實例中斷")?;
    if let Err(error) = copied {
        let _ = tokio::fs::remove_dir_all(paths::instance_dir(&id)).await;
        return Err(anyhow::Error::from(error).context("從範本複製檔案失敗，已收回未完成的實例").into());
    }
    if cover.starts_with(&dir) && cover.is_file() {
        if let Some(file) = cover.file_name() {
            let dest = paths::instance_dir(&id).join(file);
            if tokio::fs::copy(&cover, &dest).await.is_ok() {
                created.image = dest.to_string_lossy().to_string();
                instance::save(&created).await?;
            }
        }
    }
    Ok(created)
}

#[tauri::command]
pub async fn delete_template(key: String) -> R<()> {
    if !valid_key(&key) {
        bail!("範本不存在");
    }
    let dir = root().join(&key);
    let name = list_templates().await.into_iter().find(|t| t.key == key).map(|t| t.name).unwrap_or(key.clone());
    tokio::task::spawn_blocking(move || paths::discard(&format!("範本 {name}"), &[(&dir, key.as_str())]))
        .await
        .context("刪除範本中斷")?
        .context("移到回收資料夾失敗")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn template_parts_skip_saves() {
        let base = std::env::temp_dir().join(format!("bl-tpl-{}", rand::random::<u32>()));
        let game = base.join("game");
        std::fs::create_dir_all(game.join("mods")).unwrap();
        std::fs::create_dir_all(game.join("saves/World")).unwrap();
        std::fs::write(game.join("mods/a.jar"), b"jar").unwrap();
        std::fs::write(game.join("options.txt"), b"fov:90").unwrap();
        std::fs::write(game.join("saves/World/level.dat"), b"x").unwrap();
        copy_parts(&game, &base.join("out")).unwrap();
        assert!(base.join("out/mods/a.jar").is_file());
        assert_eq!(std::fs::read_to_string(base.join("out/options.txt")).unwrap(), "fov:90");
        assert!(!base.join("out/saves").exists());
        assert!(valid_key("abc123") && !valid_key("../x") && !valid_key(""));
        std::fs::remove_dir_all(&base).unwrap();
    }
}
