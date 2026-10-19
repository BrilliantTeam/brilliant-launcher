use crate::config::{self, now_secs, Account};
use crate::err::R;
use crate::state::AppState;
use crate::{auth, bail, net, paths};
use anyhow::Context;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::PathBuf;
use tauri::State;

const API: &str = "https://api.minecraftservices.com/minecraft/profile";
const TEXTURES: &str = "https://textures.minecraft.net/";
const REFRESH_AHEAD_SECS: i64 = 3 * 3600;

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LibrarySkin {
    pub file: String,
    pub name: String,
    pub variant: String,
    pub added: i64,
    #[serde(default)]
    pub path: String,
}

fn library_dir() -> PathBuf {
    paths::root().join("skins")
}

fn library_index() -> PathBuf {
    library_dir().join("library.json")
}

fn load_library() -> Vec<LibrarySkin> {
    std::fs::read(library_index()).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

fn save_library(list: &[LibrarySkin]) -> R<()> {
    std::fs::create_dir_all(library_dir())?;
    std::fs::write(library_index(), serde_json::to_vec_pretty(list)?)?;
    Ok(())
}

pub fn png_size(bytes: &[u8]) -> Option<(u32, u32)> {
    if bytes.len() < 24 || &bytes[..8] != b"\x89PNG\r\n\x1a\n" || &bytes[12..16] != b"IHDR" {
        return None;
    }
    let w = u32::from_be_bytes(bytes[16..20].try_into().ok()?);
    let h = u32::from_be_bytes(bytes[20..24].try_into().ok()?);
    Some((w, h))
}

fn valid_skin(bytes: &[u8]) -> R<()> {
    match png_size(bytes) {
        Some((64, 64)) | Some((64, 32)) => Ok(()),
        Some((w, h)) => bail!("皮膚必須是 64×64 或 64×32 的 PNG，這張是 {w}×{h}"),
        None => bail!("這不是 PNG 圖片"),
    }
}

fn variant(v: &str) -> &'static str {
    if v.eq_ignore_ascii_case("slim") { "slim" } else { "classic" }
}

async fn store_account(updated: &Account) -> R<()> {
    let mut all = config::load_accounts().await;
    if let Some(existing) = all.iter_mut().find(|a| a.id == updated.id) {
        *existing = updated.clone();
    }
    config::save_accounts(&all).await
}

pub async fn fresh(account: Account, ahead: i64) -> R<Account> {
    if account.kind != "msa" || account.expires_at > now_secs() + ahead {
        return Ok(account);
    }
    if account.refresh_token.is_empty() {
        bail!("{} 的登入已過期，請重新登入", account.name);
    }
    match auth::msa::refresh(config::MSA_CLIENT_ID, &account.refresh_token).await {
        Ok(refreshed) => {
            store_account(&refreshed).await?;
            Ok(refreshed)
        }
        Err(e) => {
            let mut marked = account.clone();
            marked.session_error = format!("{:#}", e.0);
            store_account(&marked).await?;
            Err(e)
        }
    }
}

async fn msa_account(id: &str) -> R<Account> {
    let account = config::load_accounts()
        .await
        .into_iter()
        .find(|a| a.id == id)
        .context("找不到這個帳號")?;
    if account.kind != "msa" {
        bail!("離線帳號無法更改皮膚、披風或名稱");
    }
    fresh(account, 60).await
}

async fn send(req: reqwest::RequestBuilder) -> R<Value> {
    let res = req.send().await?;
    let status = res.status();
    let text = res.text().await?;
    if status == reqwest::StatusCode::UNAUTHORIZED {
        bail!("登入已失效，請到帳號頁重新登入");
    }
    if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
        bail!("操作太頻繁了，請稍等一分鐘再試");
    }
    if !status.is_success() {
        let message = serde_json::from_str::<Value>(&text)
            .ok()
            .and_then(|v| v.get("errorMessage").or_else(|| v.get("details")).map(|m| m.to_string()))
            .unwrap_or(text);
        bail!("Minecraft 服務回應錯誤（HTTP {status}）：{message}");
    }
    Ok(if text.trim().is_empty() { Value::Null } else { serde_json::from_str(&text).unwrap_or(Value::Null) })
}

#[tauri::command]
pub async fn account_profile(id: String) -> R<Value> {
    let account = msa_account(&id).await?;
    send(net::client().get(API).bearer_auth(&account.access_token)).await
}

#[tauri::command]
pub async fn skin_library() -> Vec<LibrarySkin> {
    load_library()
        .into_iter()
        .map(|mut s| {
            s.path = library_dir().join(&s.file).to_string_lossy().to_string();
            s
        })
        .collect()
}

fn add_bytes(bytes: &[u8], name: &str, kind: &str) -> R<LibrarySkin> {
    valid_skin(bytes)?;
    let file = format!("{}.png", net::sha1_hex(bytes));
    std::fs::create_dir_all(library_dir())?;
    std::fs::write(library_dir().join(&file), bytes)?;
    let mut list = load_library();
    list.retain(|s| s.file != file);
    let entry = LibrarySkin {
        file: file.clone(),
        name: name.trim().chars().take(32).collect::<String>(),
        variant: variant(kind).into(),
        added: now_secs(),
        path: library_dir().join(&file).to_string_lossy().to_string(),
    };
    list.insert(0, entry.clone());
    save_library(&list)?;
    Ok(entry)
}

#[tauri::command]
pub async fn skin_add(path: String, name: String, variant: String) -> R<LibrarySkin> {
    let bytes = tokio::fs::read(&path).await.context("讀取圖片失敗")?;
    let fallback = PathBuf::from(&path).file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    add_bytes(&bytes, if name.trim().is_empty() { &fallback } else { &name }, &variant)
}

#[tauri::command]
pub async fn skin_save_current(id: String) -> R<LibrarySkin> {
    let profile = account_profile(id).await?;
    let active = profile
        .get("skins")
        .and_then(Value::as_array)
        .and_then(|s| s.iter().find(|x| x.get("state").and_then(Value::as_str) == Some("ACTIVE")))
        .context("這個帳號目前使用預設皮膚")?;
    let url = active.get("url").and_then(Value::as_str).unwrap_or("").replace("http://", "https://");
    if !url.starts_with(TEXTURES) {
        bail!("皮膚網址不正確");
    }
    let bytes = net::get_bytes(&url, false).await?;
    let name = format!("{} 的皮膚", profile.get("name").and_then(Value::as_str).unwrap_or("目前"));
    add_bytes(&bytes, &name, active.get("variant").and_then(Value::as_str).unwrap_or("classic"))
}

#[tauri::command]
pub async fn skin_data(file: String) -> R<String> {
    use base64::Engine;
    if !file.ends_with(".png") || file.contains(['/', '\\']) || file.contains("..") {
        bail!("皮膚檔名不正確");
    }
    let bytes = tokio::fs::read(library_dir().join(&file)).await.context("皮膚檔案不見了")?;
    Ok(format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes)))
}

#[tauri::command]
pub async fn skin_remove(file: String) -> R<()> {
    let mut list = load_library();
    list.retain(|s| s.file != file);
    save_library(&list)?;
    if file.ends_with(".png") && !file.contains(['/', '\\']) && !file.contains("..") {
        let _ = tokio::fs::remove_file(library_dir().join(file)).await;
    }
    Ok(())
}

#[tauri::command]
pub async fn skin_rename(file: String, name: String, variant: String) -> R<()> {
    let mut list = load_library();
    let entry = list.iter_mut().find(|s| s.file == file).context("找不到這個皮膚")?;
    entry.name = name.trim().chars().take(32).collect();
    entry.variant = self::variant(&variant).into();
    save_library(&list)
}

fn multipart(variant: &str, png: &[u8]) -> (String, Vec<u8>) {
    let boundary = format!("----BrilliantLauncher{}", now_secs());
    let mut body = Vec::new();
    body.extend(format!("--{boundary}\r\nContent-Disposition: form-data; name=\"variant\"\r\n\r\n{variant}\r\n").as_bytes());
    body.extend(format!("--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"skin.png\"\r\nContent-Type: image/png\r\n\r\n").as_bytes());
    body.extend(png);
    body.extend(format!("\r\n--{boundary}--\r\n").as_bytes());
    (boundary, body)
}

#[tauri::command]
pub async fn skin_apply(id: String, file: String) -> R<Value> {
    let entry = load_library().into_iter().find(|s| s.file == file).context("找不到這個皮膚")?;
    let bytes = tokio::fs::read(library_dir().join(&entry.file)).await.context("皮膚檔案不見了")?;
    valid_skin(&bytes)?;
    let account = msa_account(&id).await?;
    let (boundary, body) = multipart(variant(&entry.variant), &bytes);
    send(
        net::client()
            .post(format!("{API}/skins"))
            .bearer_auth(&account.access_token)
            .header("Content-Type", format!("multipart/form-data; boundary={boundary}"))
            .body(body),
    )
    .await
}

#[tauri::command]
pub async fn skin_reset(id: String) -> R<Value> {
    let account = msa_account(&id).await?;
    send(net::client().delete(format!("{API}/skins/active")).bearer_auth(&account.access_token)).await
}

#[tauri::command]
pub async fn cape_set(id: String, cape: Option<String>) -> R<Value> {
    let account = msa_account(&id).await?;
    let req = match cape.filter(|c| !c.is_empty()) {
        Some(cape_id) => net::client().put(format!("{API}/capes/active")).json(&serde_json::json!({ "capeId": cape_id })),
        None => net::client().delete(format!("{API}/capes/active")),
    };
    send(req.bearer_auth(&account.access_token)).await
}

fn valid_name(name: &str) -> bool {
    (3..=16).contains(&name.len()) && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

#[tauri::command]
pub async fn name_status(id: String, name: Option<String>) -> R<Value> {
    let account = msa_account(&id).await?;
    let info = send(net::client().get(format!("{API}/namechange")).bearer_auth(&account.access_token)).await?;
    let available = match name.filter(|n| !n.is_empty()) {
        Some(n) if valid_name(&n) => send(net::client().get(format!("{API}/name/{n}/available")).bearer_auth(&account.access_token))
            .await?
            .get("status")
            .and_then(Value::as_str)
            .map(str::to_string),
        Some(_) => Some("INVALID".into()),
        None => None,
    };
    Ok(serde_json::json!({ "info": info, "available": available }))
}

#[tauri::command]
pub async fn name_change(state: State<'_, AppState>, id: String, name: String) -> R<Value> {
    if !valid_name(&name) {
        bail!("名稱只能用 3～16 個英數字或底線");
    }
    let account = msa_account(&id).await?;
    let profile = send(net::client().put(format!("{API}/name/{name}")).bearer_auth(&account.access_token)).await?;
    let _guard = state.accounts_lock.lock().await;
    let mut updated = account.clone();
    updated.name = name;
    store_account(&updated).await?;
    Ok(profile)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionStatus {
    pub id: String,
    pub ok: bool,
    pub refreshed: bool,
    pub error: String,
}

#[tauri::command]
pub async fn refresh_sessions(state: State<'_, AppState>) -> R<Vec<SessionStatus>> {
    let _guard = state.accounts_lock.lock().await;
    let mut out = Vec::new();
    for account in config::load_accounts().await.into_iter().filter(|a| a.kind == "msa") {
        let before = account.expires_at;
        let id = account.id.clone();
        out.push(match fresh(account, REFRESH_AHEAD_SECS).await {
            Ok(a) => {
                if !a.session_error.is_empty() {
                    let mut cleared = a.clone();
                    cleared.session_error.clear();
                    store_account(&cleared).await?;
                }
                SessionStatus { id, ok: true, refreshed: a.expires_at != before, error: String::new() }
            }
            Err(e) => SessionStatus { id, ok: false, refreshed: false, error: format!("{:#}", e.0) },
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skin_png_checks() {
        let mut png = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
        png.extend(64u32.to_be_bytes());
        png.extend(64u32.to_be_bytes());
        assert_eq!(png_size(&png), Some((64, 64)));
        assert!(valid_skin(&png).is_ok());
        assert!(valid_skin(b"GIF89a").is_err());
        let (boundary, body) = multipart("slim", &png);
        let text = String::from_utf8_lossy(&body);
        assert!(text.starts_with(&format!("--{boundary}")) && text.contains("name=\"variant\"\r\n\r\nslim") && text.ends_with(&format!("--{boundary}--\r\n")));
        assert!(valid_name("Rice_07") && !valid_name("a b") && !valid_name("xx"));
    }
}
