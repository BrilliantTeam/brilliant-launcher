use crate::paths;
use std::path::Path;

pub const GROUPS: [&str; 4] = ["keys", "video", "audio", "controls"];

fn group_of(key: &str, sync: &SyncGroups) -> Option<&'static str> {
    if key.starts_with("key_") {
        Some("keys")
    } else if key.starts_with("soundCategory_") {
        Some("audio")
    } else if sync.video.iter().any(|k| k == key) {
        Some("video")
    } else if sync.controls.iter().any(|k| k == key) {
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

fn wanted(key: &str, groups: &[String], sync: &SyncGroups) -> bool {
    group_of(key, sync).is_some_and(|g| groups.iter().any(|w| w == g))
}

fn apply_with(source: &Path, game_dir: &Path, groups: &[String]) -> std::io::Result<usize> {
    let sync = defaults().sync_groups;
    let synced: Vec<(String, String)> = parse(&std::fs::read_to_string(source).unwrap_or_default()).into_iter().filter(|(k, _)| wanted(k, groups, &sync)).collect();
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

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct Defaults {
    mobile: Vec<(String, String)>,
    low_memory: Vec<(String, String)>,
    first_launch: Vec<(String, String)>,
    sync_groups: SyncGroups,
    server: Server,
}

#[derive(serde::Deserialize)]
struct SyncGroups {
    video: Vec<String>,
    controls: Vec<String>,
}

#[derive(serde::Deserialize)]
struct Server {
    ip: String,
    names: std::collections::HashMap<String, String>,
}

fn defaults() -> Defaults {
    crate::hotfix::data("game-options.json")
}

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
    let Defaults { mobile, low_memory: low, .. } = defaults();
    for (key, value) in mobile.into_iter().chain(if low_memory { low } else { Vec::new() }) {
        match current.iter_mut().find(|(k, _)| *k == key) {
            Some(entry) => entry.1 = value,
            None => current.push((key, value)),
        }
    }
    std::fs::create_dir_all(game_dir)?;
    paths::write_atomic(&file, render(&current).as_bytes())?;
    std::fs::write(marker, b"")?;
    Ok(true)
}


fn nbt_string(out: &mut Vec<u8>, text: &str) {
    out.extend((text.len() as u16).to_be_bytes());
    out.extend(text.as_bytes());
}

fn server_entry(ip: &str, name: &str) -> Vec<u8> {
    let mut out = Vec::new();
    for (key, value) in [("ip", ip), ("name", name)] {
        out.push(8);
        nbt_string(&mut out, key);
        nbt_string(&mut out, value);
    }
    out.push(1);
    nbt_string(&mut out, "acceptTextures");
    out.extend([1, 0]);
    out
}

fn add_server(bytes: &[u8], ip: &str, name: &str) -> Option<Vec<u8>> {
    let mut wanted = vec![8];
    nbt_string(&mut wanted, "ip");
    nbt_string(&mut wanted, &ip.to_ascii_lowercase());
    if bytes.to_ascii_lowercase().windows(wanted.len()).any(|w| w == wanted.as_slice()) {
        return None;
    }
    let mut head = vec![10, 0, 0, 9];
    nbt_string(&mut head, "servers");
    if bytes.is_empty() {
        let mut out = head;
        out.push(10);
        out.extend(1i32.to_be_bytes());
        out.extend(server_entry(ip, name));
        out.push(0);
        return Some(out);
    }
    if !bytes.starts_with(&head) || !matches!(bytes.get(head.len()), Some(0 | 10)) {
        return None;
    }
    let at = head.len() + 1;
    let count = i32::from_be_bytes(bytes.get(at..at + 4)?.try_into().ok()?);
    if bytes[head.len()] == 0 && count != 0 {
        return None;
    }
    let mut out = head;
    out.push(10);
    out.extend((count + 1).to_be_bytes());
    out.extend(server_entry(ip, name));
    out.extend(&bytes[at + 4..]);
    Some(out)
}

fn first_launch(game_dir: &Path, client_jar: &Path, language: &str, brilliant: bool) -> std::io::Result<()> {
    std::fs::create_dir_all(game_dir)?;
    let file = game_dir.join("options.txt");
    let fresh = !file.exists();
    let mut current = parse(&std::fs::read_to_string(&file).unwrap_or_default());
    if fresh {
        if let Some(version) = data_version(client_jar) {
            current.push(("version".into(), version.to_string()));
        }
        let lang = match language {
            "zh-CN" => "zh_cn",
            "en" => "en_us",
            _ => "zh_tw",
        };
        current.push(("lang".into(), lang.into()));
    }
    if brilliant {
        for (key, value) in defaults().first_launch {
            match current.iter_mut().find(|(k, _)| *k == key) {
                Some(entry) => entry.1 = value,
                None => current.push((key, value)),
            }
        }
    }
    if fresh || brilliant {
        paths::write_atomic(&file, render(&current).as_bytes())?;
    }
    if brilliant {
        let servers = game_dir.join("servers.dat");
        let server = defaults().server;
        let name = server.names.get(language).or_else(|| server.names.get("zh-TW")).cloned().unwrap_or_else(|| server.ip.clone());
        if let Some(bytes) = add_server(&std::fs::read(&servers).unwrap_or_default(), &server.ip, &name) {
            paths::write_atomic(&servers, &bytes)?;
        }
    }
    Ok(())
}

pub async fn first_launch_defaults(instance: &crate::instance::Instance, language: &str) {
    if instance.last_played != 0 {
        return;
    }
    let (dir, jar, language, brilliant) = (paths::game_dir(&instance.id), paths::version_jar(&instance.mc_version), language.to_string(), !instance.general);
    let _ = tokio::task::spawn_blocking(move || first_launch(&dir, &jar, &language, brilliant)).await;
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
    fn first_launch_presets() {
        let base = std::env::temp_dir().join(format!("bl-first-{}", rand::random::<u32>()));
        let game = base.join("game");
        let jar = base.join("missing.jar");
        first_launch(&game, &jar, "en", false).unwrap();
        assert_eq!(std::fs::read_to_string(game.join("options.txt")).unwrap(), "lang:en_us
");
        assert!(!game.join("servers.dat").exists());

        std::fs::write(game.join("options.txt"), "lang:ja_jp
tutorialStep:movement
").unwrap();
        first_launch(&game, &jar, "zh-TW", true).unwrap();
        let text = std::fs::read_to_string(game.join("options.txt")).unwrap();
        assert!(text.starts_with("lang:ja_jp
tutorialStep:none
") && text.contains("skipMultiplayerWarning:true
") && text.contains("onboardAccessibility:false
"));
        let one = std::fs::read(game.join("servers.dat")).unwrap();
        assert!(one.ends_with(&[1, 0, 0]) && one.windows(14).any(|w| w == b"brilliantw.net"));
        first_launch(&game, &jar, "zh-TW", true).unwrap();
        assert_eq!(std::fs::read(game.join("servers.dat")).unwrap(), one, "already listed");

        let mut other = vec![10, 0, 0, 9];
        nbt_string(&mut other, "servers");
        other.push(10);
        other.extend(1i32.to_be_bytes());
        for (k, v) in [("ip", "mc.example.com"), ("name", "Other")] {
            other.push(8);
            nbt_string(&mut other, k);
            nbt_string(&mut other, v);
        }
        other.extend([0, 0]);
        let merged = add_server(&other, "brilliantw.net", "輝煌伺服器").unwrap();
        assert_eq!(&merged[14..18], &2i32.to_be_bytes());
        assert!(merged.ends_with(b"Other  "));
        assert!(add_server(&merged, "BrilliantW.net", "x").is_none());
        assert!(add_server(b"garbage", "brilliantw.net", "x").is_none());
        let mut empty = vec![10, 0, 0, 9];
        nbt_string(&mut empty, "servers");
        empty.push(0);
        empty.extend(0i32.to_be_bytes());
        empty.push(0);
        assert_eq!(&add_server(&empty, "brilliantw.net", "x").unwrap()[13..18], &[10, 0, 0, 0, 1]);
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
