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
    let Ok(bytes) = std::fs::read(library_index()) else { return Vec::new() };
    serde_json::from_slice(&bytes).unwrap_or_else(|_| {
        let keep = library_index().with_extension("broken.json");
        if !keep.exists() {
            let _ = std::fs::copy(library_index(), keep);
        }
        Vec::new()
    })
}

fn save_library(list: &[LibrarySkin]) -> R<()> {
    std::fs::create_dir_all(library_dir())?;
    paths::write_atomic(&library_index(), &serde_json::to_vec_pretty(list)?).context("儲存衣櫃清單失敗")?;
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

static REFRESHING: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
static LAST_ATTEMPT: std::sync::Mutex<Option<std::time::Instant>> = std::sync::Mutex::new(None);
const RATE_LIMIT_COOLDOWN_SECS: i64 = 60 * 60;
const FAILURE_COOLDOWN_SECS: i64 = 5 * 60;
const ATTEMPT_GAP: std::time::Duration = std::time::Duration::from_secs(5);

fn cooldown_file() -> std::path::PathBuf {
    crate::paths::root().join("auth-cooldown")
}

pub fn shared_cooldown() -> i64 {
    std::fs::read_to_string(cooldown_file()).ok().and_then(|t| t.trim().parse().ok()).unwrap_or(0)
}

fn waiting(account: &Account, ahead: i64, shared: i64) -> Option<R<Account>> {
    let now = now_secs();
    if account.kind != "msa" || account.expires_at > now + ahead {
        return Some(Ok(account.clone()));
    }
    let until = account.retry_at.max(shared);
    if until <= now {
        return None;
    }
    if account.expires_at > now + 60 {
        return Some(Ok(account.clone()));
    }
    let minutes = (until - now + 59) / 60;
    Some(Err(anyhow::anyhow!("{}，約 {minutes} 分鐘後會自動再試；這段時間反覆登入會讓限制更久。", auth::msa::RATE_LIMITED).into()))
}

async fn space_attempts() {
    let wait = LAST_ATTEMPT.lock().ok().and_then(|last| last.map(|t| ATTEMPT_GAP.saturating_sub(t.elapsed())));
    if let Some(wait) = wait.filter(|w| !w.is_zero()) {
        tokio::time::sleep(wait).await;
    }
    if let Ok(mut last) = LAST_ATTEMPT.lock() {
        *last = Some(std::time::Instant::now());
    }
}

pub async fn fresh(account: Account, ahead: i64) -> R<Account> {
    if let Some(done) = waiting(&account, ahead, shared_cooldown()) {
        return done;
    }
    if account.refresh_token.is_empty() {
        bail!("{} 的登入已過期，請重新登入", account.name);
    }
    let _guard = REFRESHING.lock().await;
    let account = config::load_accounts().await.into_iter().find(|a| a.id == account.id).unwrap_or(account);
    if let Some(done) = waiting(&account, ahead, shared_cooldown()) {
        return done;
    }
    space_attempts().await;
    match auth::msa::refresh(config::MSA_CLIENT_ID, &account.refresh_token).await {
        Ok(refreshed) => {
            store_account(&refreshed).await?;
            Ok(refreshed)
        }
        Err(e) if crate::net::is_offline(&e.0) => Ok(account),
        Err(e) => {
            let detail = format!("{:#}", e.0);
            let limited = detail.contains(auth::msa::RATE_LIMITED);
            if limited {
                let _ = crate::net::write_file(&cooldown_file(), (now_secs() + RATE_LIMIT_COOLDOWN_SECS).to_string().as_bytes(), false).await;
            }
            let mut marked = account.clone();
            marked.retry_at = now_secs() + if limited { RATE_LIMIT_COOLDOWN_SECS } else { FAILURE_COOLDOWN_SECS };
            if marked.expires_at > now_secs() + 60 {
                store_account(&marked).await?;
                return Ok(marked);
            }
            marked.session_error = detail;
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

fn name_taken(list: &[LibrarySkin], name: &str, except: &str) -> bool {
    list.iter().any(|s| s.file != except && s.name.trim() == name)
}

fn add_bytes(bytes: &[u8], name: &str, kind: &str, auto_name: bool) -> R<LibrarySkin> {
    valid_skin(bytes)?;
    let file = format!("{}.png", net::sha1_hex(bytes));
    let mut list = load_library();
    let base: String = name.trim().chars().take(28).collect();
    let name = if !name_taken(&list, &base, &file) {
        base
    } else if auto_name {
        (2..).map(|n| format!("{base} ({n})")).find(|n| !name_taken(&list, n, &file)).unwrap_or_default()
    } else {
        bail!("衣櫃裡已經有叫「{base}」的皮膚了，請換一個名稱");
    };
    std::fs::create_dir_all(library_dir())?;
    std::fs::write(library_dir().join(&file), bytes)?;
    list.retain(|s| s.file != file);
    let entry = LibrarySkin {
        file: file.clone(),
        name,
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
    if tokio::fs::metadata(&path).await.context("讀取圖片失敗")?.len() > 1 << 20 {
        bail!("皮膚圖片太大了，Minecraft 皮膚是 64×64 的小 PNG 檔");
    }
    let bytes = tokio::fs::read(&path).await.context("讀取圖片失敗")?;
    let fallback = PathBuf::from(&path).file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    if name.trim().is_empty() {
        add_bytes(&bytes, &fallback, &variant, true)
    } else {
        add_bytes(&bytes, &name, &variant, false)
    }
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
    add_bytes(&bytes, &name, active.get("variant").and_then(Value::as_str).unwrap_or("classic"), true)
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
        let _ = paths::discard(&format!("皮膚 {file}"), &[(library_dir().join(&file), file.as_str())]);
    }
    Ok(())
}

#[tauri::command]
pub async fn skin_rename(file: String, name: String, variant: String) -> R<()> {
    let mut list = load_library();
    let name: String = name.trim().chars().take(32).collect();
    if name_taken(&list, &name, &file) {
        bail!("衣櫃裡已經有叫「{name}」的皮膚了，請換一個名稱");
    }
    let entry = list.iter_mut().find(|s| s.file == file).context("找不到這個皮膚")?;
    entry.name = name;
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
    let selected = config::load_settings().await.selected_account;
    let accounts = config::load_accounts().await;
    let current = accounts.iter().find(|a| a.id == selected).or_else(|| accounts.first()).cloned();
    for account in current.into_iter().filter(|a| a.kind == "msa") {
        let before = account.expires_at;
        let id = account.id.clone();
        out.push(match fresh(account, 60).await {
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

#[tauri::command]
pub async fn share_screenshot(id: String, file: String, description: String) -> R<String> {
    const SITE: &str = "https://www.brilliantw.net";
    if !file.to_ascii_lowercase().ends_with(".png") || file.contains(['/', '\\']) || file.contains("..") {
        bail!("截圖檔名不正確");
    }
    let shot = paths::game_dir(&id).join("screenshots").join(&file);
    if tokio::fs::metadata(&shot).await.context("讀取截圖失敗")?.len() > 10 * 1024 * 1024 {
        bail!("截圖超過 10 MB，無法投稿");
    }
    let bytes = tokio::fs::read(&shot).await.context("讀取截圖失敗")?;
    let settings = config::load_settings().await;
    let preferred = crate::instance::get(&id).await.map(|i| i.account).unwrap_or_default();
    let account = fresh(crate::commands::account_for(&settings, &preferred).await?, 300).await?;
    if account.kind != "msa" {
        bail!("投稿需要微軟正版帳號，離線帳號無法驗證身分");
    }
    #[derive(Deserialize)]
    struct Challenge {
        #[serde(rename = "serverId")]
        server_id: String,
    }
    let challenge: Challenge = net::post_json(&format!("{SITE}/api/launcher/gallery/challenge"), &serde_json::json!({})).await?;
    let joined = net::client()
        .post("https://sessionserver.mojang.com/session/minecraft/join")
        .json(&serde_json::json!({ "accessToken": account.access_token, "selectedProfile": account.uuid.replace('-', ""), "serverId": challenge.server_id }))
        .send()
        .await?;
    if !joined.status().is_success() {
        bail!("Minecraft 帳號驗證失敗，請到「帳號」頁重新登入後再試");
    }
    #[derive(Deserialize)]
    struct Reply {
        message: Option<String>,
        error: Option<String>,
    }
    let reply: Reply = net::client()
        .post(format!("{SITE}/api/launcher/gallery/submit"))
        .header("Content-Type", "application/octet-stream")
        .header("X-Server-Id", &challenge.server_id)
        .header("X-Player", &account.name)
        .header("X-Description", crate::modrinth::encode(description.trim()))
        .body(bytes)
        .send()
        .await?
        .json()
        .await
        .unwrap_or(Reply { message: None, error: Some("官網暫時沒有回應，請稍後再試".into()) });
    match reply.error {
        Some(error) => bail!("{error}"),
        None => Ok(reply.message.unwrap_or_else(|| "投稿成功".into())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refresh_waits_out_cooldown() {
        let now = now_secs();
        let msa = |expires_at, retry_at| Account { kind: "msa".into(), expires_at, retry_at, refresh_token: "r".into(), ..Default::default() };
        assert!(matches!(waiting(&msa(now + 30, now + 600), 3600, 0), Some(Err(_))));
        assert!(matches!(waiting(&msa(now + 7200, now + 600), 3 * 3600, 0), Some(Ok(_))));
        let blocked = waiting(&msa(now - 10, now + 600), 60, 0).unwrap().err().unwrap();
        assert!(format!("{:#}", blocked.0).contains("10 分鐘") && !format!("{:#}", blocked.0).contains("重新登入"));
        assert!(waiting(&msa(now - 10, now - 1), 60, 0).is_none());
        assert!(waiting(&msa(now + 7200, 0), 3 * 3600, 0).is_none());
        assert!(matches!(waiting(&msa(now + 7200, 0), 60, 0), Some(Ok(_))));
        let shared = waiting(&msa(now - 10, 0), 60, now + 3000).unwrap().err().unwrap();
        assert!(format!("{:#}", shared.0).contains("50 分鐘"));
        assert!(matches!(waiting(&msa(now + 7200, 0), 3 * 3600, now + 3000), Some(Ok(_))));
        assert!(waiting(&msa(now - 10, 0), 60, now - 5).is_none());
    }

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
