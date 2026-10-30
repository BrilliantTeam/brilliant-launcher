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

fn wanted(key: &str, groups: &[String]) -> bool {
    group_of(key).is_some_and(|g| groups.iter().any(|w| w == g))
}

fn apply_with(source: &Path, game_dir: &Path, groups: &[String]) -> std::io::Result<usize> {
    let synced: Vec<(String, String)> = parse(&std::fs::read_to_string(source).unwrap_or_default()).into_iter().filter(|(k, _)| wanted(k, groups)).collect();
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

pub async fn apply(instance: &crate::instance::Instance, settings: &crate::config::Settings) {
    let Some(group) = settings.instance_groups.iter().find(|g| !instance.group.is_empty() && g.name == instance.group) else {
        return;
    };
    if group.options_sync.is_empty() || group.options_source.is_empty() || group.options_source == instance.id {
        return;
    }
    let Ok(source) = crate::instance::get(&group.options_source).await else {
        return;
    };
    if source.group != instance.group {
        return;
    }
    let from = paths::game_dir(&source.id).join("options.txt");
    let to = paths::game_dir(&instance.id);
    let groups = group.options_sync.clone();
    let _ = tokio::task::spawn_blocking(move || apply_with(&from, &to, &groups)).await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copies_only_chosen_groups_from_source() {
        let base = std::env::temp_dir().join(format!("bl-opts-{}", rand::random::<u32>()));
        let (a, b) = (base.join("a"), base.join("b"));
        std::fs::create_dir_all(&a).unwrap();
        std::fs::create_dir_all(&b).unwrap();
        let source = a.join("options.txt");
        std::fs::write(&source, "fov:0.5
key_key.jump:key.keyboard.space
soundCategory_music:0.2
lang:en_us
mouseSensitivity:0.7
").unwrap();
        std::fs::write(b.join("options.txt"), "fov:0.0
lang:zh_tw
key_key.jump:key.keyboard.j
").unwrap();
        let groups = vec!["keys".to_string(), "video".to_string()];
        assert_eq!(apply_with(&source, &b, &groups).unwrap(), 2);
        assert_eq!(std::fs::read_to_string(b.join("options.txt")).unwrap(), "fov:0.5
lang:zh_tw
key_key.jump:key.keyboard.space
");
        let all: Vec<String> = GROUPS.iter().map(|g| g.to_string()).collect();
        apply_with(&source, &b, &all).unwrap();
        let text = std::fs::read_to_string(b.join("options.txt")).unwrap();
        assert!(text.contains("soundCategory_music:0.2") && text.contains("mouseSensitivity:0.7") && text.contains("lang:zh_tw"));
        assert_eq!(apply_with(&base.join("missing.txt"), &b, &all).unwrap(), 0);
        assert_eq!(std::fs::read_to_string(&source).unwrap().lines().count(), 5);
        std::fs::remove_dir_all(&base).unwrap();
    }
}
