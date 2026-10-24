use crate::err::R;
use crate::paths;
use std::path::Path;

pub const GROUPS: [&str; 4] = ["keys", "video", "audio", "controls"];

const VIDEO: [&str; 19] = [
    "fov", "renderDistance", "simulationDistance", "gamma", "guiScale", "maxFps", "enableVsync", "graphicsMode", "particles", "entityShadows",
    "biomeBlendRadius", "renderClouds", "fovEffectScale", "screenEffectScale", "darknessEffectScale", "ao", "mipmapLevels", "entityDistanceScaling", "bobView",
];

const CONTROLS: [&str; 12] = [
    "mouseSensitivity", "invertYMouse", "autoJump", "toggleCrouch", "toggleSprint", "rawMouseInput", "mouseWheelSensitivity", "discrete_mouse_scroll",
    "chatOpacity", "chatScale", "chatLineSpacing", "textBackgroundOpacity",
];

fn group_of(key: &str) -> Option<&'static str> {
    if key.starts_with("key_") {
        Some("keys")
    } else if key.starts_with("soundCategory_") {
        Some("audio")
    } else if VIDEO.contains(&key) {
        Some("video")
    } else if CONTROLS.contains(&key) {
        Some("controls")
    } else {
        None
    }
}

fn parse(text: &str) -> Vec<(String, String)> {
    text.lines().filter_map(|l| l.split_once(':')).map(|(k, v)| (k.to_string(), v.to_string())).collect()
}

fn render(list: &[(String, String)]) -> String {
    list.iter().map(|(k, v)| format!("{k}:{v}\n")).collect()
}

fn store_file() -> std::path::PathBuf {
    paths::root().join("options-sync.txt")
}

fn wanted(key: &str, groups: &[String]) -> bool {
    group_of(key).is_some_and(|g| groups.iter().any(|w| w == g))
}

fn apply_with(store: &Path, game_dir: &Path, groups: &[String]) -> std::io::Result<usize> {
    let synced: Vec<(String, String)> = parse(&std::fs::read_to_string(store).unwrap_or_default()).into_iter().filter(|(k, _)| wanted(k, groups)).collect();
    if synced.is_empty() {
        return Ok(0);
    }
    let file = game_dir.join("options.txt");
    let mut current = parse(&std::fs::read_to_string(&file).unwrap_or_default());
    for (key, value) in &synced {
        match current.iter_mut().find(|(k, _)| k == key) {
            Some(entry) => entry.1 = value.clone(),
            None => current.push((key.clone(), value.clone())),
        }
    }
    std::fs::create_dir_all(game_dir)?;
    paths::write_atomic(&file, render(&current).as_bytes())?;
    Ok(synced.len())
}

fn capture_with(store: &Path, game_dir: &Path, groups: &[String]) -> std::io::Result<usize> {
    let options = parse(&std::fs::read_to_string(game_dir.join("options.txt")).unwrap_or_default());
    let fresh: Vec<(String, String)> = options.into_iter().filter(|(k, _)| wanted(k, groups)).collect();
    if fresh.is_empty() {
        return Ok(0);
    }
    let mut saved = parse(&std::fs::read_to_string(store).unwrap_or_default());
    for (key, value) in &fresh {
        match saved.iter_mut().find(|(k, _)| k == key) {
            Some(entry) => entry.1 = value.clone(),
            None => saved.push((key.clone(), value.clone())),
        }
    }
    paths::write_atomic(store, render(&saved).as_bytes())?;
    Ok(fresh.len())
}

pub fn apply(game_dir: &Path, settings: &crate::config::Settings) {
    if settings.options_sync {
        let _ = apply_with(&store_file(), game_dir, &settings.options_sync_groups);
    }
}

pub fn capture(game_dir: &Path, settings: &crate::config::Settings) {
    if settings.options_sync {
        let _ = capture_with(&store_file(), game_dir, &settings.options_sync_groups);
    }
}

#[tauri::command]
pub async fn options_sync_capture(id: String) -> R<usize> {
    let settings = crate::config::load_settings().await;
    let groups = if settings.options_sync_groups.is_empty() { GROUPS.iter().map(|g| g.to_string()).collect() } else { settings.options_sync_groups.clone() };
    let game = paths::game_dir(&id);
    Ok(tokio::task::spawn_blocking(move || capture_with(&store_file(), &game, &groups)).await.map_err(anyhow::Error::from)??)
}

#[tauri::command]
pub fn options_sync_count() -> usize {
    parse(&std::fs::read_to_string(store_file()).unwrap_or_default()).len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn syncs_only_chosen_groups() {
        let base = std::env::temp_dir().join(format!("bl-opts-{}", rand::random::<u32>()));
        let (a, b) = (base.join("a"), base.join("b"));
        std::fs::create_dir_all(&a).unwrap();
        std::fs::create_dir_all(&b).unwrap();
        let store = base.join("store.txt");
        std::fs::write(a.join("options.txt"), "fov:0.5\nkey_key.jump:key.keyboard.space\nsoundCategory_music:0.2\nlang:en_us\nmouseSensitivity:0.7\n").unwrap();
        std::fs::write(b.join("options.txt"), "fov:0.0\nlang:zh_tw\nkey_key.jump:key.keyboard.j\n").unwrap();
        let groups = vec!["keys".to_string(), "video".to_string()];
        assert_eq!(capture_with(&store, &a, &groups).unwrap(), 2);
        assert_eq!(apply_with(&store, &b, &groups).unwrap(), 2);
        assert_eq!(std::fs::read_to_string(b.join("options.txt")).unwrap(), "fov:0.5\nlang:zh_tw\nkey_key.jump:key.keyboard.space\n");
        let all: Vec<String> = GROUPS.iter().map(|g| g.to_string()).collect();
        capture_with(&store, &a, &all).unwrap();
        apply_with(&store, &b, &all).unwrap();
        let text = std::fs::read_to_string(b.join("options.txt")).unwrap();
        assert!(text.contains("soundCategory_music:0.2") && text.contains("mouseSensitivity:0.7") && text.contains("lang:zh_tw"));
        std::fs::remove_dir_all(&base).unwrap();
    }
}
