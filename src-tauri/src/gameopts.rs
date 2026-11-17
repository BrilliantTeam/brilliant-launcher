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

const MOBILE: [(&str, &str); 20] = [
    ("graphicsPreset", "\"custom\""),
    ("renderDistance", "8"),
    ("simulationDistance", "6"),
    ("maxFps", "60"),
    ("enableVsync", "false"),
    ("particles", "1"),
    ("entityShadows", "false"),
    ("renderClouds", "\"fast\""),
    ("cloudRange", "64"),
    ("cutoutLeaves", "false"),
    ("improvedTransparency", "false"),
    ("biomeBlendRadius", "0"),
    ("chunkSectionFadeInTime", "0.0"),
    ("weatherRadius", "5"),
    ("entityDistanceScaling", "0.75"),
    ("vignette", "false"),
    ("menuBackgroundBlurriness", "0"),
    ("mipmapLevels", "0"),
    ("inactivityFpsLimit", "\"afk\""),
    ("tutorialStep", "none"),
];

const LOW_MEMORY: [(&str, &str); 4] = [
    ("renderDistance", "5"),
    ("simulationDistance", "5"),
    ("particles", "2"),
    ("entityDistanceScaling", "0.5"),
];

fn low_memory_device() -> bool {
    matches!(crate::boost::total_memory_mb(), 1..5120)
}

fn data_version(client_jar: &Path) -> Option<i64> {
    let mut zip = zip::ZipArchive::new(std::fs::File::open(client_jar).ok()?).ok()?;
    let json: serde_json::Value = serde_json::from_reader(zip.by_name("version.json").ok()?).ok()?;
    json["world_version"].as_i64()
}

fn tune_for_mobile(game_dir: &Path, client_jar: &Path, low_memory: bool) -> std::io::Result<bool> {
    let marker = game_dir.join(".brilliant-mobile-options");
    if marker.exists() {
        return Ok(false);
    }
    let file = game_dir.join("options.txt");
    let mut current = parse(&std::fs::read_to_string(&file).unwrap_or_default());
    if !current.iter().any(|(k, _)| k == "version") {
        let Some(version) = data_version(client_jar) else {
            return Ok(false);
        };
        current.insert(0, ("version".into(), version.to_string()));
    }
    for (key, value) in MOBILE.iter().chain(if low_memory { LOW_MEMORY.iter() } else { [].iter() }) {
        match current.iter_mut().find(|(k, _)| k == key) {
            Some(entry) => entry.1 = value.to_string(),
            None => current.push((key.to_string(), value.to_string())),
        }
    }
    std::fs::create_dir_all(game_dir)?;
    paths::write_atomic(&file, render(&current).as_bytes())?;
    std::fs::write(marker, b"")?;
    Ok(true)
}

pub async fn mobile_defaults(instance: &crate::instance::Instance) {
    let (dir, jar) = (paths::game_dir(&instance.id), paths::version_jar(&instance.mc_version));
    let _ = tokio::task::spawn_blocking(move || tune_for_mobile(&dir, &jar, low_memory_device())).await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mobile_tuning_writes_data_version_once() {
        let base = std::env::temp_dir().join(format!("bl-mobile-{}", rand::random::<u32>()));
        std::fs::create_dir_all(&base).unwrap();
        let jar = base.join("client.jar");
        {
            let mut zip = zip::ZipWriter::new(std::fs::File::create(&jar).unwrap());
            zip.start_file("version.json", zip::write::SimpleFileOptions::default()).unwrap();
            std::io::Write::write_all(&mut zip, br#"{"world_version": 5023}"#).unwrap();
            zip.finish().unwrap();
        }
        let game = base.join("game");
        assert!(tune_for_mobile(&game, &jar, false).unwrap());
        let text = std::fs::read_to_string(game.join("options.txt")).unwrap();
        assert!(text.starts_with("version:5023\n") && text.contains("renderDistance:8\n") && text.contains("graphicsPreset:\"custom\"\n") && text.contains("tutorialStep:none\n"));
        std::fs::write(game.join("options.txt"), "version:5023\nrenderDistance:12\n").unwrap();
        assert!(!tune_for_mobile(&game, &jar, false).unwrap());
        assert_eq!(std::fs::read_to_string(game.join("options.txt")).unwrap(), "version:5023\nrenderDistance:12\n");
        std::fs::remove_dir_all(&game).unwrap();
        std::fs::create_dir_all(&game).unwrap();
        std::fs::write(game.join("options.txt"), "version:4000\nrenderDistance:20\nlang:zh_tw\n").unwrap();
        assert!(tune_for_mobile(&game, &base.join("missing.jar"), true).unwrap());
        let text = std::fs::read_to_string(game.join("options.txt")).unwrap();
        assert!(text.starts_with("version:4000\nrenderDistance:5\nlang:zh_tw\n") && text.contains("simulationDistance:5\n") && text.contains("particles:2\n"));
        std::fs::remove_dir_all(&base).unwrap();
    }

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
