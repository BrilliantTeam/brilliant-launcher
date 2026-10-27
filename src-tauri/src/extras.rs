use crate::config::now_secs;
use crate::err::R;
use crate::state::AppState;
use crate::{bail, paths};
use anyhow::Context;
use serde::Serialize;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager, State};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Backup {
    pub file: String,
    pub kind: String,
    pub time: i64,
    pub size: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Screenshot {
    pub path: String,
    pub name: String,
    pub time: i64,
}

fn backups_dir(id: &str) -> PathBuf {
    paths::instance_dir(id).join("backups")
}

fn mtime(path: &Path) -> i64 {
    std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn zip_dir(src: &Path, dest: &Path) -> R<()> {
    zip_folder(src, dest, None)
}

pub fn zip_folder(src: &Path, dest: &Path, prefix: Option<&str>) -> R<()> {
    let tmp = dest.with_extension("part");
    let result = write_zip(src, &tmp, prefix).and_then(|_| std::fs::rename(&tmp, dest).context("儲存壓縮檔失敗").map_err(Into::into));
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result
}

fn write_zip(src: &Path, tmp: &Path, prefix: Option<&str>) -> R<()> {
    let mut zip = zip::ZipWriter::new(std::fs::File::create(tmp).with_context(|| format!("建立 {} 失敗", tmp.display()))?);
    let options = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    let mut stack = vec![src.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir)?.flatten() {
            let path = entry.path();
            let rel = path.strip_prefix(src).unwrap_or(&path).to_string_lossy().replace('\\', "/");
            let rel = match prefix {
                Some(p) => format!("{p}/{rel}"),
                None => rel,
            };
            let kind = entry.file_type().with_context(|| format!("讀取 {} 失敗", path.display()))?;
            if kind.is_symlink() {
                continue;
            }
            if kind.is_dir() {
                zip.add_directory(format!("{rel}/"), options)?;
                stack.push(path);
            } else if entry.file_name() != "session.lock" {
                zip.start_file(rel, options)?;
                let mut file = std::fs::File::open(&path).with_context(|| format!("讀取 {} 失敗", path.display()))?;
                std::io::copy(&mut file, &mut zip).with_context(|| format!("壓縮 {} 失敗", path.display()))?;
            }
        }
    }
    zip.finish()?;
    Ok(())
}

pub fn backup_saves(id: &str, kind: &str, keep: u32) -> R<Option<String>> {
    let saves = paths::game_dir(id).join("saves");
    if !saves.is_dir() || std::fs::read_dir(&saves).map(|mut d| d.next().is_none()).unwrap_or(true) {
        return Ok(None);
    }
    let dir = backups_dir(id);
    std::fs::create_dir_all(&dir)?;
    let mut time = now_secs();
    while dir.join(format!("{kind}-{time}.zip")).exists() {
        time += 1;
    }
    let name = format!("{kind}-{time}.zip");
    zip_dir(&saves, &dir.join(&name)).map_err(|e| anyhow::anyhow!("備份存檔失敗：{:#}", e.0))?;
    if keep > 0 {
        let mut old: Vec<_> = list(id).into_iter().filter(|b| b.kind == kind).collect();
        old.sort_by_key(|b| std::cmp::Reverse(b.time));
        for b in old.into_iter().skip(keep as usize) {
            let _ = std::fs::remove_file(dir.join(b.file));
        }
    }
    Ok(Some(name))
}

fn list(id: &str) -> Vec<Backup> {
    let mut out: Vec<Backup> = std::fs::read_dir(backups_dir(id))
        .map(|d| d.flatten().collect::<Vec<_>>())
        .unwrap_or_default()
        .into_iter()
        .filter_map(|e| {
            let file = e.file_name().to_string_lossy().to_string();
            let stem = file.strip_suffix(".zip")?;
            let (kind, time) = stem.rsplit_once('-')?;
            Some(Backup {
                kind: kind.to_string(),
                time: time.parse().ok()?,
                size: e.metadata().map(|m| m.len()).unwrap_or(0),
                file,
            })
        })
        .collect();
    out.sort_by_key(|b| std::cmp::Reverse(b.time));
    out
}

fn safe_backup_name(file: &str) -> bool {
    file.ends_with(".zip") && !file.contains(['/', '\\']) && !file.contains("..")
}

#[tauri::command]
pub async fn list_backups(id: String) -> Vec<Backup> {
    list(&id)
}

#[tauri::command]
pub async fn backup_now(id: String) -> R<String> {
    tokio::task::spawn_blocking(move || backup_saves(&id, "manual", 0))
        .await
        .context("備份工作中斷")??
        .context("這個實例還沒有任何存檔")
        .map_err(Into::into)
}

#[tauri::command]
pub async fn restore_backup(state: State<'_, AppState>, id: String, file: String) -> R<()> {
    if !safe_backup_name(&file) {
        bail!("備份檔名不正確");
    }
    state.ensure_idle(&id).await?;
    tokio::task::spawn_blocking(move || -> R<()> {
        let archive = backups_dir(&id).join(&file);
        let mut zip = zip::ZipArchive::new(std::fs::File::open(&archive).context("找不到備份檔")?)?;
        backup_saves(&id, "before-restore", 3)?;
        let saves = paths::game_dir(&id).join("saves");
        if crate::advanced::is_link(&saves) {
            let worlds: Vec<(PathBuf, String)> = std::fs::read_dir(&saves)
                .map(|d| d.flatten().map(|e| (e.path(), format!("saves/{}", e.file_name().to_string_lossy()))).collect())
                .unwrap_or_default();
            let items: Vec<(&PathBuf, &str)> = worlds.iter().map(|(p, r)| (p, r.as_str())).collect();
            if !items.is_empty() {
                paths::discard(&format!("還原備份前的共用存檔 {file}"), &items).context("把目前存檔移到回收資料夾失敗")?;
            }
        } else if saves.exists() {
            paths::discard(&format!("還原備份前的存檔 {file}"), &[(&saves, "saves")]).context("把目前存檔移到回收資料夾失敗")?;
        }
        zip.extract(&saves).context("解壓縮備份失敗")?;
        Ok(())
    })
    .await
    .context("還原工作中斷")?
}

#[tauri::command]
pub async fn delete_backup(id: String, file: String) -> R<()> {
    if !safe_backup_name(&file) {
        bail!("備份檔名不正確");
    }
    paths::discard(&format!("備份 {file}"), &[(backups_dir(&id).join(&file), file.as_str())]).context("刪除備份失敗")?;
    Ok(())
}

#[tauri::command]
pub async fn list_screenshots(id: String) -> Vec<Screenshot> {
    let mut out: Vec<Screenshot> = std::fs::read_dir(paths::game_dir(&id).join("screenshots"))
        .map(|d| d.flatten().collect::<Vec<_>>())
        .unwrap_or_default()
        .into_iter()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("png")))
        .map(|p| Screenshot {
            time: mtime(&p),
            name: p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(),
            path: p.to_string_lossy().to_string(),
        })
        .collect();
    out.sort_by_key(|s| std::cmp::Reverse(s.time));
    out
}

const WATCHED: [&str; 4] = ["mods", "resourcepacks", "shaderpacks", "screenshots"];

#[tauri::command]
pub async fn dir_stamp(subs: Vec<String>, id: Option<String>) -> R<String> {
    if subs.is_empty() || subs.iter().any(|s| !WATCHED.contains(&s.as_str())) {
        bail!("資料夾不正確");
    }
    let ids: Vec<String> = match id {
        Some(id) => vec![crate::instance::get(&id).await?.id],
        None => crate::instance::list().await.into_iter().map(|i| i.id).collect(),
    };
    tokio::task::spawn_blocking(move || {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        for id in &ids {
            for sub in &subs {
                let mut entries: Vec<_> = std::fs::read_dir(paths::game_dir(id).join(sub))
                    .map(|d| d.flatten().filter_map(|e| e.metadata().ok().map(|m| (e.file_name(), m.len(), m.modified().ok()))).collect())
                    .unwrap_or_default();
                entries.sort();
                (id, sub, entries).hash(&mut hasher);
            }
        }
        hasher.finish().to_string()
    })
    .await
    .context("讀取資料夾失敗")
    .map_err(Into::into)
}

#[tauri::command]
pub async fn clean_cache(state: State<'_, AppState>) -> R<u64> {
    if !state.running_ids().await.is_empty() {
        bail!("遊戲執行中無法清理，請先關閉遊戲");
    }
    tokio::task::spawn_blocking(|| {
        let mut freed = 0;
        let natives = paths::shared().join("natives");
        freed += crate::worlds::dir_size(&natives);
        let _ = std::fs::remove_dir_all(&natives);
        let thumbs = crate::insight::thumbs_dir();
        freed += crate::worlds::dir_size(&thumbs);
        let _ = std::fs::remove_dir_all(&thumbs);
        freed += paths::purge_trash(paths::TRASH_DAYS);
        let week_ago = now_secs() - 7 * 86_400;
        for entry in std::fs::read_dir(paths::logs()).map(|d| d.flatten().collect::<Vec<_>>()).unwrap_or_default() {
            if mtime(&entry.path()) < week_ago {
                freed += entry.metadata().map(|m| m.len()).unwrap_or(0);
                let _ = std::fs::remove_file(entry.path());
            }
        }
        freed
    })
    .await
    .context("清理工作中斷")
    .map_err(Into::into)
}

pub const IMAGE_EXTS: [&str; 5] = ["png", "jpg", "jpeg", "webp", "gif"];
pub const COVER_EXTS: [&str; 6] = ["png", "jpg", "jpeg", "webp", "gif", "svg"];
const VIDEO_EXTS: [&str; 2] = ["mp4", "webm"];
const AUDIO_EXTS: [&str; 6] = ["mp3", "ogg", "wav", "m4a", "flac", "opus"];

pub async fn copy_media(src: &Path, exts: &[&str], max_mb: u64, dir: &Path, prefix: &str) -> R<String> {
    store_media(src, exts, max_mb, dir, prefix, true).await
}

async fn store_media(src: &Path, exts: &[&str], max_mb: u64, dir: &Path, prefix: &str, replace: bool) -> R<String> {
    let ext = src
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .filter(|e| exts.contains(&e.as_str()))
        .with_context(|| format!("只支援 {} 格式", exts.join("、").to_uppercase()))?;
    let meta = tokio::fs::metadata(src).await.context("找不到檔案")?;
    if meta.len() > max_mb * 1024 * 1024 {
        bail!("檔案太大了（上限 {max_mb} MB）");
    }
    tokio::fs::create_dir_all(dir).await?;
    if replace {
        for entry in std::fs::read_dir(dir).map(|d| d.flatten().collect::<Vec<_>>()).unwrap_or_default() {
            if entry.file_name().to_string_lossy().starts_with(prefix) {
                let _ = std::fs::remove_file(entry.path());
            }
        }
    }
    let dest = dir.join(if replace { format!("{prefix}{}.{ext}", now_secs()) } else { unique_name(prefix, &ext) });
    tokio::fs::copy(src, &dest).await.context("複製檔案失敗")?;
    Ok(dest.to_string_lossy().to_string())
}

const PICTURE_EXTS: [&str; 6] = ["png", "jpg", "jpeg", "webp", "gif", "svg"];

fn images_dir() -> PathBuf {
    paths::root().join("media").join("images")
}

fn unique_name(prefix: &str, ext: &str) -> String {
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or_default();
    format!("{prefix}{nanos:x}.{ext}")
}

#[tauri::command]
pub async fn import_image(path: String) -> R<String> {
    let src = PathBuf::from(&path);
    let ext = src
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .filter(|e| PICTURE_EXTS.contains(&e.as_str()))
        .with_context(|| format!("只支援 {} 格式", PICTURE_EXTS.join("、").to_uppercase()))?;
    let meta = tokio::fs::metadata(&src).await.context("找不到檔案")?;
    if !meta.is_file() || meta.len() > 15 * 1024 * 1024 {
        bail!("圖片太大了（上限 15 MB）");
    }
    let dir = images_dir();
    tokio::fs::create_dir_all(&dir).await?;
    let day_ago = std::time::SystemTime::now() - std::time::Duration::from_secs(86400);
    for entry in std::fs::read_dir(&dir).map(|d| d.flatten().collect::<Vec<_>>()).unwrap_or_default() {
        let stale = entry.metadata().and_then(|m| m.modified()).map(|t| t < day_ago).unwrap_or(false);
        if stale && entry.file_name().to_string_lossy().starts_with(STAGING) {
            let _ = std::fs::remove_file(entry.path());
        }
    }
    let dest = dir.join(unique_name(STAGING, &ext));
    tokio::fs::copy(&src, &dest).await.context("複製圖片失敗")?;
    Ok(dest.to_string_lossy().to_string())
}

const STAGING: &str = "staging-";

fn kept_path(dir: &Path, path: &Path) -> Option<PathBuf> {
    let name = path.file_name()?.to_string_lossy().to_string();
    let rest = name.strip_prefix(STAGING)?;
    if path.parent()?.canonicalize().ok()? != dir.canonicalize().ok()? {
        return None;
    }
    Some(dir.join(format!("image-{rest}")))
}

#[tauri::command]
pub async fn keep_image(path: String) -> R<String> {
    let src = PathBuf::from(&path);
    let Some(dest) = kept_path(&images_dir(), &src) else {
        return Ok(path);
    };
    tokio::fs::rename(&src, &dest).await.context("保存圖片失敗")?;
    Ok(dest.to_string_lossy().to_string())
}

#[tauri::command]
pub async fn read_svg(path: String) -> R<String> {
    let dir = paths::root().canonicalize().context("找不到啟動器資料夾")?;
    let file = PathBuf::from(&path).canonicalize().context("找不到這張圖片")?;
    if !file.starts_with(&dir) || file.extension().map(|e| e.to_string_lossy().to_lowercase()) != Some("svg".into()) {
        bail!("只能讀取啟動器保存的 SVG 圖片");
    }
    if tokio::fs::metadata(&file).await?.len() > 512 * 1024 {
        bail!("SVG 檔案太大了（上限 512 KB）");
    }
    Ok(tokio::fs::read_to_string(&file).await.context("讀取 SVG 失敗")?)
}

#[tauri::command]
pub async fn save_svg(text: String) -> R<String> {
    let trimmed = text.trim_start();
    if !trimmed.starts_with("<svg") || text.len() > 512 * 1024 || text.contains("<script") || text.contains("<foreignObject") {
        bail!("SVG 內容不正確");
    }
    let dir = images_dir();
    tokio::fs::create_dir_all(&dir).await?;
    let dest = dir.join(unique_name("image-", "svg"));
    tokio::fs::write(&dest, text).await.context("儲存 SVG 失敗")?;
    Ok(dest.to_string_lossy().to_string())
}

const LOOK_MANIFEST: &str = "look.json";
const LOOK_FORMAT: &str = "brilliant-look";
const LOOK_JSON_MAX: u64 = 256 << 10;
const LOOK_BG_MAX: u64 = 500 << 20;
const LOOK_PREVIEW_MAX: u64 = 20 << 20;
const LOOK_TRACK_MAX: u64 = 200 << 20;
const LOOK_TRACKS: usize = 20;

#[derive(Serialize, serde::Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct LookManifest {
    format: String,
    version: u32,
    name: String,
    description: String,
    look: serde_json::Value,
    background: String,
    music: Vec<crate::config::MediaItem>,
}

fn media_ok(ext: &str, head: &[u8]) -> bool {
    match ext {
        "png" => head.starts_with(b"\x89PNG"),
        "jpg" | "jpeg" => head.starts_with(&[0xFF, 0xD8]),
        "webp" => head.len() >= 12 && &head[..4] == b"RIFF" && &head[8..12] == b"WEBP",
        "gif" => head.starts_with(b"GIF8"),
        "mp4" => head.len() >= 8 && &head[4..8] == b"ftyp",
        "webm" => head.starts_with(&[0x1A, 0x45, 0xDF, 0xA3]),
        _ => AUDIO_EXTS.contains(&ext),
    }
}

fn is_background_ext(ext: &str) -> bool {
    IMAGE_EXTS.contains(&ext) || VIDEO_EXTS.contains(&ext)
}

fn inside_launcher(path: &str) -> R<PathBuf> {
    let dir = paths::root().canonicalize().context("找不到啟動器資料夾")?;
    let file = PathBuf::from(path).canonicalize().with_context(|| format!("找不到檔案：{path}"))?;
    if !file.starts_with(&dir) || !file.is_file() {
        bail!("只能匯出啟動器保存的背景與音樂");
    }
    Ok(file)
}

fn ext_of(path: &Path) -> String {
    path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default()
}

fn safe_title(text: &str) -> String {
    let cleaned: String = text.chars().map(|c| if "\\/:*?\"<>|".contains(c) || c.is_control() { '_' } else { c }).collect();
    let title: String = cleaned.trim().chars().take(40).collect();
    if title.is_empty() { "未命名".into() } else { title }
}

fn head_of(path: &Path) -> Vec<u8> {
    let mut head = vec![0u8; 12];
    let read = std::fs::File::open(path).and_then(|mut f| std::io::Read::read(&mut f, &mut head)).unwrap_or(0);
    head.truncate(read);
    head
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportedLook {
    pub path: String,
    pub background: String,
}

#[tauri::command]
pub async fn export_look(name: String, description: String, look: serde_json::Value, background: String, music: Vec<crate::config::MediaItem>) -> R<ExportedLook> {
    if !look.is_object() || look.to_string().len() > 64 << 10 {
        bail!("外觀設定不正確");
    }
    if music.len() > LOOK_TRACKS {
        bail!("背景音樂最多 {LOOK_TRACKS} 首");
    }
    let mut manifest = LookManifest { format: LOOK_FORMAT.into(), version: 1, look, ..Default::default() };
    manifest.name = name.chars().filter(|c| !c.is_control()).take(24).collect::<String>().trim().to_string();
    manifest.description = description.chars().filter(|c| !c.is_control()).take(80).collect::<String>().trim().to_string();
    let mut entries: Vec<(String, PathBuf)> = Vec::new();
    if !background.is_empty() {
        let file = inside_launcher(&background)?;
        let ext = ext_of(&file);
        if !is_background_ext(&ext) || std::fs::metadata(&file)?.len() > LOOK_BG_MAX || !media_ok(&ext, &head_of(&file)) {
            bail!("背景必須是 500 MB 以內的圖片（PNG、JPG、WEBP、GIF）或影片（MP4、WEBM）");
        }
        manifest.background = format!("background.{ext}");
        entries.push((manifest.background.clone(), file));
    }
    for (index, track) in music.iter().enumerate() {
        let file = inside_launcher(&track.path)?;
        let ext = ext_of(&file);
        if !AUDIO_EXTS.contains(&ext.as_str()) || std::fs::metadata(&file)?.len() > LOOK_TRACK_MAX {
            bail!("背景音樂必須是 200 MB 以內的音訊檔");
        }
        let entry = format!("music/{}.{ext}", index + 1);
        manifest.music.push(crate::config::MediaItem { path: entry.clone(), name: track.name.chars().filter(|c| !c.is_control()).take(60).collect() });
        entries.push((entry, file));
    }
    let dir = dirs::download_dir().unwrap_or_else(|| paths::root().join("exports"));
    tokio::fs::create_dir_all(&dir).await?;
    let path = paths::unused(dir.join(format!("輝煌外觀-{}.brlook", safe_title(&manifest.name))));
    let kept = match entries.first().filter(|(name, _)| name.starts_with("background.")) {
        Some((name, file)) => {
            let images = images_dir();
            std::fs::create_dir_all(&images)?;
            let copy = images.join(unique_name("look-bg-", name.trim_start_matches("background.")));
            std::fs::copy(file, &copy).context("保存背景失敗")?;
            copy.to_string_lossy().to_string()
        }
        None => String::new(),
    };
    let target = path.clone();
    let json = serde_json::to_vec_pretty(&manifest)?;
    let written = tokio::task::spawn_blocking(move || -> R<()> {
        let mut zip = zip::ZipWriter::new(std::fs::File::create(&target)?);
        zip.set_comment("Brilliant Launcher look");
        zip.start_file(LOOK_MANIFEST, zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated))?;
        std::io::Write::write_all(&mut zip, &json)?;
        let stored = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored).large_file(true);
        for (name, file) in entries {
            zip.start_file(name, stored)?;
            std::io::copy(&mut std::fs::File::open(&file)?, &mut zip)?;
        }
        zip.finish()?;
        Ok(())
    })
    .await
    .context("匯出外觀中斷")?;
    if let Err(error) = written {
        let _ = std::fs::remove_file(&path);
        return Err(error.0.context("匯出外觀失敗").into());
    }
    Ok(ExportedLook { path: path.to_string_lossy().to_string(), background: kept })
}

fn open_look(path: &str) -> R<(zip::ZipArchive<std::fs::File>, LookManifest)> {
    let file = std::fs::File::open(path).context("找不到這個外觀檔")?;
    let mut archive = zip::ZipArchive::new(file).context("這不是輝煌外觀檔")?;
    let manifest = read_manifest(&mut archive)?;
    Ok((archive, manifest))
}

fn read_manifest<T: std::io::Read + std::io::Seek>(archive: &mut zip::ZipArchive<T>) -> R<LookManifest> {
    let entry = archive.by_name(LOOK_MANIFEST).context("這不是輝煌外觀檔")?;
    if entry.size() > LOOK_JSON_MAX {
        bail!("外觀檔內容不正確");
    }
    let mut text = String::new();
    std::io::Read::read_to_string(&mut std::io::Read::take(entry, LOOK_JSON_MAX), &mut text).context("外觀檔內容不正確")?;
    let manifest: LookManifest = serde_json::from_str(&text).context("外觀檔內容不正確")?;
    if manifest.format != LOOK_FORMAT || !manifest.look.is_object() {
        bail!("這不是輝煌外觀檔");
    }
    let background_ok = manifest.background.is_empty()
        || manifest.background.strip_prefix("background.").is_some_and(is_background_ext);
    let music_ok = manifest.music.len() <= LOOK_TRACKS
        && manifest.music.iter().enumerate().all(|(i, t)| t.path.strip_prefix(&format!("music/{}.", i + 1)).is_some_and(|ext| AUDIO_EXTS.contains(&ext)));
    if !background_ok || !music_ok {
        bail!("外觀檔內容不正確");
    }
    Ok(manifest)
}

fn extract_entry<T: std::io::Read + std::io::Seek>(archive: &mut zip::ZipArchive<T>, name: &str, max: u64, dest: &Path) -> R<()> {
    let entry = archive.by_name(name).with_context(|| format!("外觀檔缺少 {name}"))?;
    if entry.size() > max {
        bail!("外觀檔裡的 {name} 太大了");
    }
    let mut out = std::fs::File::create(dest)?;
    let copied = std::io::copy(&mut std::io::Read::take(entry, max + 1), &mut out)?;
    drop(out);
    let ext = name.rsplit('.').next().unwrap_or_default();
    if copied > max || !media_ok(ext, &head_of(dest)) {
        let _ = std::fs::remove_file(dest);
        bail!("外觀檔裡的 {name} 格式不正確");
    }
    Ok(())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LookInfo {
    pub name: String,
    pub description: String,
    pub look: serde_json::Value,
    pub background: String,
    pub background_kind: String,
    pub music: Vec<String>,
}

#[tauri::command]
pub async fn inspect_look(path: String) -> R<LookInfo> {
    tokio::task::spawn_blocking(move || -> R<LookInfo> {
        use base64::Engine;
        let (mut archive, manifest) = open_look(&path)?;
        let ext = manifest.background.rsplit('.').next().unwrap_or_default().to_string();
        let kind = if manifest.background.is_empty() { "" } else if VIDEO_EXTS.contains(&ext.as_str()) { "video" } else { "image" };
        let mut preview = String::new();
        if kind == "image" {
            if let Ok(entry) = archive.by_name(&manifest.background) {
                if entry.size() <= LOOK_PREVIEW_MAX {
                    let mut bytes = Vec::new();
                    if std::io::Read::read_to_end(&mut std::io::Read::take(entry, LOOK_PREVIEW_MAX), &mut bytes).is_ok() && media_ok(&ext, &bytes) {
                        let mime = if ext == "jpg" { "jpeg" } else { ext.as_str() };
                        preview = format!("data:image/{mime};base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes));
                    }
                }
            }
        }
        Ok(LookInfo {
            name: manifest.name,
            description: manifest.description,
            look: manifest.look,
            background: preview,
            background_kind: kind.into(),
            music: manifest.music.into_iter().map(|t| t.name).collect(),
        })
    })
    .await
    .context("讀取外觀檔中斷")?
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledLook {
    pub background: String,
    pub music: Vec<crate::config::MediaItem>,
}

#[tauri::command]
pub async fn install_look(path: String, background: bool, music: bool) -> R<InstalledLook> {
    tokio::task::spawn_blocking(move || -> R<InstalledLook> {
        let (mut archive, manifest) = open_look(&path)?;
        let mut installed = InstalledLook { background: String::new(), music: Vec::new() };
        if background && !manifest.background.is_empty() {
            let dir = images_dir();
            std::fs::create_dir_all(&dir)?;
            let dest = dir.join(unique_name("look-bg-", manifest.background.rsplit('.').next().unwrap_or("png")));
            extract_entry(&mut archive, &manifest.background, LOOK_BG_MAX, &dest)?;
            installed.background = dest.to_string_lossy().to_string();
        }
        if music {
            let dir = paths::root().join("media");
            std::fs::create_dir_all(&dir)?;
            for track in &manifest.music {
                let dest = dir.join(unique_name("music-", track.path.rsplit('.').next().unwrap_or("mp3")));
                extract_entry(&mut archive, &track.path, LOOK_TRACK_MAX, &dest)?;
                let name = if track.name.trim().is_empty() { "匯入的音樂".to_string() } else { track.name.clone() };
                installed.music.push(crate::config::MediaItem { path: dest.to_string_lossy().to_string(), name });
            }
        }
        Ok(installed)
    })
    .await
    .context("匯入外觀中斷")?
}

#[cfg(test)]
mod look_file_tests {
    use super::*;

    fn pack(manifest: &serde_json::Value, files: &[(&str, &[u8])]) -> zip::ZipArchive<std::io::Cursor<Vec<u8>>> {
        let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        zip.start_file(LOOK_MANIFEST, zip::write::SimpleFileOptions::default()).unwrap();
        std::io::Write::write_all(&mut zip, manifest.to_string().as_bytes()).unwrap();
        for (name, bytes) in files {
            zip.start_file(*name, zip::write::SimpleFileOptions::default()).unwrap();
            std::io::Write::write_all(&mut zip, bytes).unwrap();
        }
        zip::ZipArchive::new(zip.finish().unwrap()).unwrap()
    }

    #[test]
    fn look_archive_is_validated() {
        let good = serde_json::json!({ "format": LOOK_FORMAT, "version": 1, "name": "雨荷", "look": { "accent": "#45B5A5" }, "background": "background.mp4", "music": [{ "path": "music/1.ogg", "name": "雨聲" }] });
        let manifest = read_manifest(&mut pack(&good, &[])).unwrap();
        assert_eq!((manifest.name.as_str(), manifest.background.as_str(), manifest.music[0].name.as_str()), ("雨荷", "background.mp4", "雨聲"));
        for bad in [
            serde_json::json!({ "format": "other", "look": {} }),
            serde_json::json!({ "format": LOOK_FORMAT, "look": {}, "background": "../../evil.exe" }),
            serde_json::json!({ "format": LOOK_FORMAT, "look": {}, "music": [{ "path": "music/1.exe", "name": "x" }] }),
            serde_json::json!({ "format": LOOK_FORMAT, "look": {}, "music": [{ "path": "../music/1.mp3", "name": "x" }] }),
        ] {
            assert!(read_manifest(&mut pack(&bad, &[])).is_err(), "{bad}");
        }
        assert!(media_ok("mp4", b"\0\0\0\x18ftypisom") && media_ok("webm", &[0x1A, 0x45, 0xDF, 0xA3]) && !media_ok("mp4", b"MZ\0\0\0\0\0\0"));
    }

    #[test]
    fn extracted_media_must_match_its_type() {
        let manifest = serde_json::json!({ "format": LOOK_FORMAT, "look": {}, "background": "background.png" });
        let dir = std::env::temp_dir().join(format!("brlook-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut fake = pack(&manifest, &[("background.png", b"MZ not an image")]);
        assert!(extract_entry(&mut fake, "background.png", 1024, &dir.join("a.png")).is_err());
        assert!(!dir.join("a.png").exists(), "rejected files are removed");
        let mut real = pack(&manifest, &[("background.png", b"\x89PNG\r\n\x1a\n....")]);
        extract_entry(&mut real, "background.png", 1024, &dir.join("b.png")).unwrap();
        assert!(extract_entry(&mut real, "background.png", 4, &dir.join("c.png")).is_err(), "size limit applies");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[tauri::command]
pub async fn save_image(data: String) -> R<String> {
    use base64::Engine;
    let (ext, body) = if let Some(rest) = data.strip_prefix("data:image/png;base64,") {
        ("png", rest)
    } else if let Some(rest) = data.strip_prefix("data:image/jpeg;base64,") {
        ("jpg", rest)
    } else {
        bail!("圖片資料不正確");
    };
    let bytes = base64::engine::general_purpose::STANDARD.decode(body).context("圖片資料不正確")?;
    let valid = if ext == "png" { bytes.starts_with(b"\x89PNG") } else { bytes.starts_with(&[0xFF, 0xD8]) };
    if !valid || bytes.len() > 8 << 20 {
        bail!("圖片資料不正確");
    }
    let dir = images_dir();
    tokio::fs::create_dir_all(&dir).await?;
    let dest = dir.join(unique_name("image-", ext));
    tokio::fs::write(&dest, bytes).await.context("儲存圖片失敗")?;
    Ok(dest.to_string_lossy().to_string())
}

fn removable_media(media: &Path, target: &Path) -> bool {
    let name = target.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let parent = target.parent().and_then(|p| p.canonicalize().ok());
    parent.is_some() && parent == media.canonicalize().ok() && (name.starts_with("font-") || name.starts_with("music-"))
}

#[tauri::command]
pub async fn remove_media(path: String) -> R<()> {
    let media = paths::root().join("media");
    let target = PathBuf::from(&path);
    if !target.exists() {
        return Ok(());
    }
    if !removable_media(&media, &target) {
        bail!("只能移除啟動器媒體資料夾裡的字型或音樂");
    }
    let name = target.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    tokio::task::spawn_blocking(move || paths::discard("移除的字型與音樂", &[(&target, name.as_str())]))
        .await?
        .context("移到回收資料夾失敗")?;
    Ok(())
}

fn is_font(head: &[u8; 4]) -> bool {
    matches!(head, [0, 1, 0, 0] | b"OTTO" | b"true" | b"ttcf" | b"wOFF" | b"wOF2")
}

#[tauri::command]
pub async fn set_media(kind: String, path: String) -> R<String> {
    let src = PathBuf::from(&path);
    let media = paths::root().join("media");
    match kind.as_str() {
        "background" => copy_media(&src, &[&IMAGE_EXTS[..], &VIDEO_EXTS[..]].concat(), 500, &media, "background-").await,
        "music" => store_media(&src, &AUDIO_EXTS, 200, &media, "music-", false).await,
        "font" => {
            let mut head = [0u8; 4];
            let read = std::fs::File::open(&src).and_then(|mut f| std::io::Read::read_exact(&mut f, &mut head));
            if read.is_err() || !is_font(&head) {
                bail!("這個檔案不是有效的字型檔（TTF、OTF、WOFF、WOFF2）");
            }
            store_media(&src, &["ttf", "otf", "woff", "woff2"], 50, &media, "font-", false).await
        }
        _ => bail!("不支援的媒體類型"),
    }
}

pub fn set_autostart(on: bool) -> R<()> {
    let exe = std::env::current_exe()?;
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";
        let mut cmd = std::process::Command::new("reg");
        if on {
            cmd.args(["add", KEY, "/v", "BrilliantLauncher", "/t", "REG_SZ", "/f", "/d"])
                .arg(format!("\"{}\"", exe.display()));
        } else {
            cmd.args(["delete", KEY, "/v", "BrilliantLauncher", "/f"]);
        }
        let status = cmd.creation_flags(0x0800_0000).output()?.status;
        if on && !status.success() {
            bail!("無法設定開機自動啟動");
        }
    }
    #[cfg(target_os = "macos")]
    {
        let file = dirs::home_dir().context("找不到使用者資料夾")?.join("Library/LaunchAgents/net.brilliantw.launcher.plist");
        if on {
            std::fs::create_dir_all(file.parent().unwrap_or(&file))?;
            let escaped = exe.to_string_lossy().replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;");
            std::fs::write(&file, format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n<plist version=\"1.0\"><dict><key>Label</key><string>net.brilliantw.launcher</string><key>ProgramArguments</key><array><string>{escaped}</string></array><key>RunAtLoad</key><true/></dict></plist>\n"))?;
        } else {
            let _ = std::fs::remove_file(&file);
        }
    }
    #[cfg(target_os = "linux")]
    {
        if std::env::var_os("FLATPAK_ID").is_some() {
            bail!("Flatpak 版無法自行設定開機啟動，請到系統設定的「自動啟動」加入輝煌啟動器");
        }
        let file = dirs::config_dir().context("找不到設定資料夾")?.join("autostart/brilliant-launcher.desktop");
        if on {
            std::fs::create_dir_all(file.parent().unwrap_or(&file))?;
            let target = std::env::var_os("APPIMAGE").map(std::path::PathBuf::from).unwrap_or(exe);
            std::fs::write(&file, format!("[Desktop Entry]\nType=Application\nName=輝煌啟動器\nExec=\"{}\"\nX-GNOME-Autostart-enabled=true\n", target.display()))?;
        } else {
            let _ = std::fs::remove_file(&file);
        }
    }
    Ok(())
}

pub fn hook_command(script: &str, instance: &crate::instance::Instance) -> tokio::process::Command {
    let mut cmd = if cfg!(target_os = "windows") {
        let mut c = tokio::process::Command::new("cmd");
        c.arg("/C").arg(script);
        c
    } else {
        let mut c = tokio::process::Command::new("sh");
        c.arg("-c").arg(script);
        c
    };
    cmd.current_dir(paths::game_dir(&instance.id))
        .env("INST_ID", &instance.id)
        .env("INST_NAME", &instance.name)
        .env("INST_MC_VER", &instance.mc_version)
        .env("INST_DIR", paths::game_dir(&instance.id))
        .stdin(std::process::Stdio::null());
    crate::mc::java::hide_console(&mut cmd);
    cmd
}

pub fn reveal(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

pub fn setup_tray(app: &tauri::App) -> tauri::Result<()> {
    use tauri::menu::{Menu, MenuItem};
    use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
    use tauri::Emitter;

    let show = MenuItem::with_id(app, "show", "顯示啟動器", true, None::<&str>)?;
    let play = MenuItem::with_id(app, "play", "開始遊戲", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "結束輝煌啟動器", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &play, &quit])?;
    let mut tray = TrayIconBuilder::with_id("main")
        .tooltip("輝煌啟動器")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(move |app, event| match event.id().as_ref() {
            "show" => reveal(app),
            "play" => {
                reveal(app);
                let _ = app.emit("launcher://tray-play", ());
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(move |tray, event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                reveal(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }
    tray.build(app)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn staged_images_become_permanent() {
        let dir = std::env::temp_dir().join(format!("bl-img-{}", now_secs()));
        std::fs::create_dir_all(&dir).unwrap();
        let staged = dir.join("staging-abc.png");
        std::fs::write(&staged, b"x").unwrap();
        assert_eq!(kept_path(&dir, &staged), Some(dir.join("image-abc.png")));
        assert_eq!(kept_path(&dir, &dir.join("image-abc.png")), None);
        assert_eq!(kept_path(&dir, &dir.join("upload-old.png")), None);
        assert_eq!(kept_path(&dir, &std::env::temp_dir().join("staging-abc.png")), None);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn only_launcher_media_is_removable() {
        let media = std::env::temp_dir().join(format!("bl-media-{}", now_secs()));
        std::fs::create_dir_all(&media).unwrap();
        for name in ["font-1.ttf", "music-1.mp3", "background-1.png"] {
            std::fs::write(media.join(name), b"x").unwrap();
        }
        assert!(removable_media(&media, &media.join("font-1.ttf")));
        assert!(removable_media(&media, &media.join("music-1.mp3")));
        assert!(!removable_media(&media, &media.join("background-1.png")));
        assert!(!removable_media(&media, &media.join("sub/../../font-1.ttf")));
        assert!(!removable_media(&media, &std::env::temp_dir().join("font-1.ttf")));
        std::fs::remove_dir_all(&media).unwrap();
    }

    #[test]
    fn font_signatures() {
        let head = |name: &str| {
            let bytes = std::fs::read(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../src/assets").join(name)).unwrap();
            [bytes[0], bytes[1], bytes[2], bytes[3]]
        };
        assert!(is_font(&head("font/Minecraft.otf")));
        assert!(is_font(&head("font/NaikaiFont-Regular.ttf")));
        assert!(!is_font(&head("icon.png")));
        assert!(is_font(b"wOF2") && !is_font(b"PK"));
    }

    #[test]
    fn zip_round_trip_skips_session_lock() {
        let base = std::env::temp_dir().join(format!("bl-zip-{}", now_secs()));
        let src = base.join("saves/World/region");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::write(src.join("r.0.0.mca"), b"chunk").unwrap();
        std::fs::write(base.join("saves/World/session.lock"), b"lock").unwrap();
        #[cfg(windows)]
        assert!(std::process::Command::new("cmd").arg("/C").arg("mklink").arg("/J").arg(base.join("saves").join("World").join("loop")).arg(base.join("saves")).stdout(std::process::Stdio::null()).status().unwrap().success());
        let archive = base.join("out.zip");
        zip_dir(&base.join("saves"), &archive).unwrap();
        let out = base.join("restored");
        zip::ZipArchive::new(std::fs::File::open(&archive).unwrap()).unwrap().extract(&out).unwrap();
        assert_eq!(std::fs::read(out.join("World/region/r.0.0.mca")).unwrap(), b"chunk");
        assert!(!out.join("World/session.lock").exists());
        assert!(safe_backup_name("auto-1.zip") && !safe_backup_name("../x.zip"));
        let _ = std::fs::remove_dir_all(base);
    }
}
