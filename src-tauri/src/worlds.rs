use crate::config::now_secs;
use crate::err::R;
use crate::state::AppState;
use crate::{bail, paths};
use anyhow::Context;
use serde::Serialize;
use std::io::Read;
use std::path::{Path, PathBuf};
use tauri::State;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct World {
    pub folder: String,
    pub name: String,
    pub size: u64,
    pub last_played: i64,
    pub game_mode: i32,
    pub hardcore: bool,
    pub icon: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Pack {
    pub file: String,
    pub enabled: bool,
}

fn saves(id: &str) -> PathBuf {
    paths::game_dir(id).join("saves")
}

fn safe_name(name: &str) -> bool {
    !name.is_empty() && !name.contains(['/', '\\']) && name != "." && name != ".."
}

fn find_tag<'a>(nbt: &'a [u8], kind: u8, name: &str) -> Option<&'a [u8]> {
    let mut needle = vec![kind];
    needle.extend((name.len() as u16).to_be_bytes());
    needle.extend(name.as_bytes());
    let at = nbt.windows(needle.len()).position(|w| w == needle.as_slice())?;
    nbt.get(at + needle.len()..)
}

pub fn read_level_file(path: &Path) -> (Option<String>, i64, i32, bool) {
    let mut bytes = Vec::new();
    match std::fs::File::open(path).map(|f| f.take(16 << 20).read_to_end(&mut bytes)) {
        Ok(Ok(_)) => read_level(&bytes),
        _ => (None, 0, 0, false),
    }
}

pub fn read_level(bytes: &[u8]) -> (Option<String>, i64, i32, bool) {
    let mut nbt = Vec::new();
    if flate2::read::GzDecoder::new(bytes).take(16 << 20).read_to_end(&mut nbt).is_err() {
        nbt = bytes.to_vec();
    }
    let name = find_tag(&nbt, 8, "LevelName").and_then(|rest| {
        let len = u16::from_be_bytes([*rest.first()?, *rest.get(1)?]) as usize;
        String::from_utf8(rest.get(2..2 + len)?.to_vec()).ok()
    });
    let last = find_tag(&nbt, 4, "LastPlayed")
        .and_then(|r| r.get(..8))
        .map(|b| i64::from_be_bytes(b.try_into().unwrap_or([0; 8])))
        .unwrap_or(0);
    let mode = find_tag(&nbt, 3, "GameType")
        .and_then(|r| r.get(..4))
        .map(|b| i32::from_be_bytes(b.try_into().unwrap_or([0; 4])))
        .unwrap_or(0);
    let hardcore = find_tag(&nbt, 1, "hardcore").and_then(|r| r.first()).is_some_and(|b| *b != 0);
    (name, last, mode, hardcore)
}

pub(crate) fn dir_size(path: &Path) -> u64 {
    std::fs::read_dir(path)
        .map(|d| {
            d.flatten()
                .map(|e| match e.file_type() {
                    Ok(t) if t.is_symlink() => 0,
                    Ok(t) if t.is_dir() => dir_size(&e.path()),
                    _ => e.metadata().map(|m| m.len()).unwrap_or(0),
                })
                .sum()
        })
        .unwrap_or(0)
}

fn unique(dir: &Path, base: &str) -> String {
    if !dir.join(base).exists() {
        return base.to_string();
    }
    (2..).map(|n| format!("{base} ({n})")).find(|n| !dir.join(n).exists()).unwrap_or_default()
}

async fn ensure_idle(state: &State<'_, AppState>, id: &str) -> R<()> {
    state.ensure_idle(id).await
}

#[tauri::command]
pub async fn list_worlds(id: String) -> Vec<World> {
    tokio::task::spawn_blocking(move || {
        let mut out: Vec<World> = std::fs::read_dir(saves(&id))
            .map(|d| d.flatten().collect::<Vec<_>>())
            .unwrap_or_default()
            .into_iter()
            .filter(|e| e.path().join("level.dat").is_file())
            .map(|e| {
                let folder = e.file_name().to_string_lossy().to_string();
                let (name, last, mode, hardcore) = read_level_file(&e.path().join("level.dat"));
                let icon = e.path().join("icon.png");
                World {
                    name: name.filter(|n| !n.is_empty()).unwrap_or_else(|| folder.clone()),
                    size: dir_size(&e.path()),
                    last_played: last,
                    game_mode: mode,
                    hardcore,
                    icon: icon.is_file().then(|| icon.to_string_lossy().to_string()),
                    folder,
                }
            })
            .collect();
        out.sort_by_key(|w| std::cmp::Reverse(w.last_played));
        out
    })
    .await
    .unwrap_or_default()
}

#[tauri::command]
pub async fn world_export(id: String, folder: String) -> R<String> {
    if !safe_name(&folder) {
        bail!("世界資料夾名稱不正確");
    }
    let src = saves(&id).join(&folder);
    if !src.join("level.dat").is_file() {
        bail!("找不到這個世界");
    }
    let out_dir = dirs::download_dir().unwrap_or_else(|| paths::root().join("exports"));
    std::fs::create_dir_all(&out_dir)?;
    let base = format!("{folder}-{}", now_secs());
    let dest = paths::unused(out_dir.join(format!("{base}.zip")));
    let target = dest.clone();
    tokio::task::spawn_blocking(move || crate::extras::zip_folder(&src, &target, Some(&folder)))
        .await
        .context("匯出工作中斷")??;
    Ok(dest.to_string_lossy().to_string())
}

#[tauri::command]
pub async fn world_copy(state: State<'_, AppState>, id: String, folder: String, target: String) -> R<String> {
    if !safe_name(&folder) {
        bail!("世界資料夾名稱不正確");
    }
    ensure_idle(&state, &target).await?;
    crate::instance::get(&target).await?;
    let src = saves(&id).join(&folder);
    if !src.join("level.dat").is_file() {
        bail!("找不到這個世界");
    }
    let dest_dir = saves(&target);
    std::fs::create_dir_all(&dest_dir)?;
    let name = unique(&dest_dir, &folder);
    let dest = dest_dir.join(&name);
    tokio::task::spawn_blocking(move || crate::importer::copy_tree(&src, &dest))
        .await
        .context("複製工作中斷")?
        .context("複製世界失敗")?;
    Ok(name)
}

#[tauri::command]
pub async fn world_delete(state: State<'_, AppState>, id: String, folder: String) -> R<String> {
    if !safe_name(&folder) {
        bail!("世界資料夾名稱不正確");
    }
    ensure_idle(&state, &id).await?;
    let src = saves(&id).join(&folder);
    if !src.is_dir() {
        bail!("找不到這個世界");
    }
    let bin = paths::discard(&format!("世界 {folder}"), &[(&src, folder.as_str())]).context("移到回收資料夾失敗")?;
    Ok(bin.join(&folder).to_string_lossy().to_string())
}

fn world_root(dir: &Path) -> Option<PathBuf> {
    if dir.join("level.dat").is_file() {
        return Some(dir.to_path_buf());
    }
    std::fs::read_dir(dir).ok()?.flatten().map(|e| e.path()).find(|p| p.join("level.dat").is_file())
}

#[tauri::command]
pub async fn world_import(state: State<'_, AppState>, id: String, path: String) -> R<String> {
    ensure_idle(&state, &id).await?;
    let src = PathBuf::from(&path);
    let dest_dir = saves(&id);
    std::fs::create_dir_all(&dest_dir)?;
    tokio::task::spawn_blocking(move || -> R<String> {
        let staging = paths::instance_dir(&id).join(format!(".import-{}", now_secs()));
        let result = import_world(&src, &staging, &dest_dir);
        let _ = std::fs::remove_dir_all(&staging);
        result
    })
    .await
    .context("匯入工作中斷")?
}

fn import_world(src: &Path, staging: &Path, dest_dir: &Path) -> R<String> {
    let from = if src.is_dir() {
        src.to_path_buf()
    } else {
        let mut zip = zip::ZipArchive::new(std::fs::File::open(src).context("打不開這個檔案")?).context("這不是 zip 檔")?;
        zip.extract(staging).context("解壓縮失敗")?;
        staging.to_path_buf()
    };
    let root = world_root(&from).context("裡面找不到 level.dat，不像是 Minecraft 世界")?;
    let base = root
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .filter(|n| safe_name(n) && !n.starts_with(".import-"))
        .unwrap_or_else(|| src.file_stem().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "匯入的世界".into()));
    let name = unique(dest_dir, &base);
    crate::importer::copy_tree(&root, &dest_dir.join(&name)).context("複製世界失敗")?;
    Ok(name)
}

pub fn read_options(path: &Path) -> R<String> {
    match std::fs::read(path) {
        Ok(bytes) => String::from_utf8(bytes).map_err(|_| anyhow::anyhow!("options.txt 含有無法辨識的文字編碼，為了不弄壞遊戲設定，這次不修改").into()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(e) => Err(anyhow::Error::from(e).context("讀取 options.txt 失敗，為了不弄壞遊戲設定，這次不修改").into()),
    }
}

fn options_path(id: &str) -> PathBuf {
    paths::game_dir(id).join("options.txt")
}

pub fn parse_pack_list(line: &str) -> Vec<String> {
    serde_json::from_str(line).unwrap_or_default()
}

pub fn merge_order(current: &[String], enabled_top_first: &[String]) -> Vec<String> {
    let mut out: Vec<String> = current.iter().filter(|p| !p.starts_with("file/")).cloned().collect();
    if !out.iter().any(|p| p == "vanilla") {
        out.insert(0, "vanilla".into());
    }
    out.extend(enabled_top_first.iter().rev().map(|f| format!("file/{f}")));
    out
}

#[tauri::command]
pub async fn pack_order(id: String) -> Vec<Pack> {
    let options = std::fs::read_to_string(options_path(&id)).unwrap_or_default();
    let current = options.lines().find_map(|l| l.strip_prefix("resourcePacks:")).map(parse_pack_list).unwrap_or_default();
    let on: Vec<String> = current.iter().rev().filter_map(|p| p.strip_prefix("file/").map(str::to_string)).collect();
    let mut files: Vec<String> = std::fs::read_dir(paths::game_dir(&id).join("resourcepacks"))
        .map(|d| d.flatten().map(|e| e.file_name().to_string_lossy().to_string()).collect())
        .unwrap_or_default();
    files.retain(|f| !f.starts_with('.'));
    files.sort_by_key(|f| f.to_lowercase());
    let mut out: Vec<Pack> = on.iter().filter(|f| files.contains(f)).map(|f| Pack { file: f.clone(), enabled: true }).collect();
    out.extend(files.into_iter().filter(|f| !on.contains(f)).map(|file| Pack { file, enabled: false }));
    out
}

#[tauri::command]
pub async fn set_pack_order(state: State<'_, AppState>, id: String, enabled: Vec<String>) -> R<()> {
    ensure_idle(&state, &id).await?;
    let dir = paths::game_dir(&id).join("resourcepacks");
    if enabled.iter().any(|f| !safe_name(f) || !dir.join(f).exists()) {
        bail!("資源包清單不正確");
    }
    let path = options_path(&id);
    let options = read_options(&path)?;
    let current = options.lines().find_map(|l| l.strip_prefix("resourcePacks:")).map(parse_pack_list).unwrap_or_default();
    let line = format!("resourcePacks:{}", serde_json::to_string(&merge_order(&current, &enabled))?);
    let mut lines: Vec<String> = options.lines().map(str::to_string).collect();
    match lines.iter().position(|l| l.starts_with("resourcePacks:")) {
        Some(i) => lines[i] = line,
        None => lines.push(line),
    }
    let incompatible = lines.iter().position(|l| l.starts_with("incompatibleResourcePacks:"));
    if let Some(i) = incompatible {
        let keep: Vec<String> = parse_pack_list(&lines[i]["incompatibleResourcePacks:".len()..])
            .into_iter()
            .filter(|p| !enabled.iter().any(|f| *p == format!("file/{f}")))
            .collect();
        lines[i] = format!("incompatibleResourcePacks:{}", serde_json::to_string(&keep)?);
    }
    std::fs::create_dir_all(paths::game_dir(&id))?;
    paths::write_atomic(&path, (lines.join("\n") + "\n").as_bytes()).context("寫入 options.txt 失敗")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn unreadable_options_are_never_treated_as_empty() {
        let dir = std::env::temp_dir().join(format!("options-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        assert_eq!(read_options(&dir.join("missing.txt")).unwrap(), "");
        std::fs::write(dir.join("ok.txt"), "fov:1\nlang:zh_tw\n").unwrap();
        assert!(read_options(&dir.join("ok.txt")).unwrap().contains("lang:zh_tw"));
        std::fs::write(dir.join("bad.txt"), b"fov:1\nlastServer:\xa7\xda\n").unwrap();
        assert!(read_options(&dir.join("bad.txt")).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    use super::*;
    use std::io::Write;

    #[test]
    fn reads_level_dat_fields() {
        let mut nbt = vec![10u8, 0, 0, 10, 0, 4];
        nbt.extend(b"Data");
        nbt.extend([8, 0, 9]);
        nbt.extend(b"LevelName");
        nbt.extend(("新的世界ab".len() as u16).to_be_bytes());
        nbt.extend("新的世界ab".as_bytes());
        nbt.extend([4, 0, 10]);
        nbt.extend(b"LastPlayed");
        nbt.extend(1_700_000_000_000i64.to_be_bytes());
        nbt.extend([3, 0, 8]);
        nbt.extend(b"GameType");
        nbt.extend(1i32.to_be_bytes());
        nbt.extend([1, 0, 8]);
        nbt.extend(b"hardcore");
        nbt.push(1);
        let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        gz.write_all(&nbt).unwrap();
        let (name, last, mode, hardcore) = read_level(&gz.finish().unwrap());
        assert_eq!(name.as_deref(), Some("新的世界ab"));
        assert_eq!((last, mode, hardcore), (1_700_000_000_000, 1, true));
    }

    #[test]
    #[ignore]
    fn reads_real_worlds() {
        let root = std::env::var("SAVES_FROM").expect("SAVES_FROM");
        for entry in std::fs::read_dir(root).unwrap().flatten() {
            let level = entry.path().join("level.dat");
            if let Ok(bytes) = std::fs::read(&level) {
                println!("{:?} => {:?}", entry.file_name(), read_level(&bytes));
            }
        }
    }

    #[test]
    fn pack_order_keeps_builtin_entries() {
        let current: Vec<String> = parse_pack_list(r#"["vanilla","fabric","continuity:default","file/Old.zip"]"#);
        let merged = merge_order(&current, &["Top.zip".into(), "Bottom.zip".into()]);
        assert_eq!(merged, vec!["vanilla", "fabric", "continuity:default", "file/Bottom.zip", "file/Top.zip"]);
        assert_eq!(merge_order(&[], &[])[0], "vanilla");
        assert!(!safe_name("../x") && safe_name("Faithful 32x.zip"));
    }
}
