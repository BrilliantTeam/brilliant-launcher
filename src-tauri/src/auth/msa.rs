use crate::config::{now_secs, Account};
use crate::err::R;
use crate::net;
use crate::bail;
use anyhow::{anyhow, Context};
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use serde::Deserialize;
use serde_json::json;
use sha2::{Digest, Sha256};

const AUTHORIZE_URL: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0/authorize";
pub const REDIRECT_URI: &str = "https://login.microsoftonline.com/common/oauth2/nativeclient";
const TOKEN_URL: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0/token";
const XBL_URL: &str = "https://user.auth.xboxlive.com/user/authenticate";
const XSTS_URL: &str = "https://xsts.auth.xboxlive.com/xsts/authorize";
const MC_LOGIN_URL: &str = "https://api.minecraftservices.com/authentication/login_with_xbox";
const MC_PROFILE_URL: &str = "https://api.minecraftservices.com/minecraft/profile";
const SCOPE: &str = "XboxLive.signin offline_access";

#[derive(Deserialize)]
struct MsaToken {
    access_token: String,
    #[serde(default)]
    refresh_token: String,
    expires_in: i64,
}

#[derive(Deserialize)]
struct TokenError {
    error: String,
    #[serde(default)]
    error_description: String,
}

#[derive(Deserialize)]
struct XboxAuth {
    #[serde(rename = "Token")]
    token: String,
    #[serde(rename = "DisplayClaims")]
    display_claims: DisplayClaims,
}

#[derive(Deserialize)]
struct DisplayClaims {
    xui: Vec<Xui>,
}

#[derive(Deserialize)]
struct Xui {
    #[serde(default)]
    uhs: String,
    #[serde(default)]
    xid: String,
}

#[derive(Deserialize)]
struct McToken {
    access_token: String,
    #[serde(default)]
    expires_in: i64,
}

#[derive(Deserialize)]
struct McProfile {
    id: String,
    name: String,
}

pub struct Pkce {
    pub verifier: String,
    pub state: String,
    pub redirect: String,
}

pub fn authorize_url(client_id: &str, redirect: &str) -> (String, Pkce) {
    let verifier = random_hex(32);
    let state = random_hex(16);
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    let mut url = tauri::Url::parse(AUTHORIZE_URL).expect("固定網址");
    url.query_pairs_mut()
        .append_pair("client_id", client_id)
        .append_pair("response_type", "code")
        .append_pair("redirect_uri", redirect)
        .append_pair("response_mode", "query")
        .append_pair("scope", SCOPE)
        .append_pair("code_challenge", &challenge)
        .append_pair("code_challenge_method", "S256")
        .append_pair("state", &state)
        .append_pair("prompt", "select_account");
    (url.into(), Pkce { verifier, state, redirect: redirect.to_string() })
}

pub enum Redirect {
    Code(String),
    Denied(String),
    Ignore,
}

pub fn read_redirect(url: &tauri::Url, state: &str, redirect: &str) -> Redirect {
    if !url.as_str().starts_with(redirect) || !url.query().is_some_and(|q| q.contains("state=")) {
        return Redirect::Ignore;
    }
    let query: std::collections::HashMap<_, _> = url.query_pairs().into_owned().collect();
    if query.get("state").map(String::as_str) != Some(state) {
        return Redirect::Denied("登入回應的 state 不符，已中止".into());
    }
    match (query.get("code"), query.get("error")) {
        (Some(code), _) => Redirect::Code(code.clone()),
        (_, Some(err)) if err == "access_denied" => Redirect::Denied("你取消了授權".into()),
        (_, Some(err)) => Redirect::Denied(format!("登入失敗（{err}）：{}", query.get("error_description").cloned().unwrap_or_default())),
        _ => Redirect::Denied("登入回應缺少授權碼".into()),
    }
}

pub async fn exchange(client_id: &str, code: &str, pkce: &Pkce) -> R<Account> {
    let res = net::client()
        .post(TOKEN_URL)
        .form(&[
            ("grant_type", "authorization_code"),
            ("client_id", client_id),
            ("code", code),
            ("redirect_uri", pkce.redirect.as_str()),
            ("code_verifier", pkce.verifier.as_str()),
            ("scope", SCOPE),
        ])
        .send()
        .await?;
    let status = res.status();
    let text = res.text().await?;
    if !status.is_success() {
        let err: TokenError = serde_json::from_str(&text)
            .unwrap_or(TokenError { error: "unknown".into(), error_description: text.clone() });
        bail!("登入失敗（{}）：{}", err.error, err.error_description);
    }
    let token: MsaToken = serde_json::from_str(&text).context("解析權杖回應失敗")?;
    finish(token).await
}

fn random_hex(bytes: usize) -> String {
    let buf: Vec<u8> = (0..bytes).map(|_| rand::random::<u8>()).collect();
    hex::encode(buf)
}

pub async fn refresh(client_id: &str, refresh_token: &str) -> R<Account> {
    let res = net::client()
        .post(TOKEN_URL)
        .form(&[
            ("grant_type", "refresh_token"),
            ("client_id", client_id),
            ("refresh_token", refresh_token),
            ("scope", SCOPE),
        ])
        .send()
        .await?;
    let status = res.status();
    let text = res.text().await?;
    if !status.is_success() {
        bail!("續期失敗（HTTP {status}），請重新登入：{text}");
    }
    let token: MsaToken = serde_json::from_str(&text).context("解析續期回應失敗")?;
    finish(token).await
}

async fn finish(msa: MsaToken) -> R<Account> {
    let xbl: XboxAuth = net::post_json(
        XBL_URL,
        &json!({
            "Properties": {
                "AuthMethod": "RPS",
                "SiteName": "user.auth.xboxlive.com",
                "RpsTicket": format!("d={}", msa.access_token),
            },
            "RelyingParty": "http://auth.xboxlive.com",
            "TokenType": "JWT",
        }),
    )
    .await
    .map_err(|e| anyhow!("Xbox Live 驗證失敗：{:#}", e.0))?;

    let uhs = xbl
        .display_claims
        .xui
        .first()
        .map(|x| x.uhs.clone())
        .filter(|s| !s.is_empty())
        .context("Xbox Live 沒有回傳使用者雜湊")?;

    let xsts: XboxAuth = match net::post_json(
        XSTS_URL,
        &json!({
            "Properties": { "SandboxId": "RETAIL", "UserTokens": [xbl.token] },
            "RelyingParty": "rp://api.minecraftservices.com/",
            "TokenType": "JWT",
        }),
    )
    .await
    {
        Ok(v) => v,
        Err(e) => bail!("{}", xsts_hint(&format!("{:#}", e.0))),
    };

    let xuid = xsts.display_claims.xui.first().map(|x| x.xid.clone()).unwrap_or_default();

    let mc: McToken = net::post_json(
        MC_LOGIN_URL,
        &json!({ "identityToken": format!("XBL3.0 x={uhs};{}", xsts.token) }),
    )
    .await
    .map_err(|e| {
        let detail = format!("{:#}", e.0);
        if detail.contains("Invalid app registration") {
            anyhow!("輝煌啟動器的正版登入正在等待 Mojang 審核開通，目前暫時無法使用正版帳號。
請先使用離線帳號；開通後不需要更新啟動器，直接重新登入即可。")
        } else {
            anyhow!("Minecraft 服務驗證失敗：{detail}")
        }
    })?;

    let res = net::client()
        .get(MC_PROFILE_URL)
        .bearer_auth(&mc.access_token)
        .send()
        .await?;
    if res.status() == reqwest::StatusCode::NOT_FOUND {
        bail!("這個微軟帳號沒有 Minecraft: Java Edition，無法用正版登入");
    }
    let status = res.status();
    let text = res.text().await?;
    if !status.is_success() {
        bail!("取得遊戲檔案失敗（HTTP {status}）：{text}");
    }
    let profile: McProfile = serde_json::from_str(&text).context("解析遊戲檔案失敗")?;

    Ok(Account {
        id: profile.id.clone(),
        kind: "msa".into(),
        name: profile.name,
        uuid: super::dash_uuid(&profile.id),
        access_token: mc.access_token,
        refresh_token: msa.refresh_token,
        xuid,
        expires_at: now_secs() + mc.expires_in.max(msa.expires_in).max(3600),
        session_error: String::new(),
    })
}

fn xsts_hint(raw: &str) -> String {
    let hint = if raw.contains("2148916233") {
        "這個微軟帳號沒有 Xbox 個人資料，請先到 xbox.com 建立後再登入"
    } else if raw.contains("2148916238") {
        "未成年帳號需要先加入家庭群組才能登入"
    } else if raw.contains("2148916235") {
        "你所在的地區未提供 Xbox Live 服務"
    } else {
        "XSTS 授權失敗"
    };
    format!("{hint}\n（原始回應：{raw}）")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redirect_is_parsed_and_state_checked() {
        let (url, pkce) = authorize_url("cid", REDIRECT_URI);
        assert!(url.contains("code_challenge_method=S256") && url.contains(&pkce.state));
        let ok = tauri::Url::parse(&format!("{REDIRECT_URI}?code=abc&state={}", pkce.state)).unwrap();
        assert!(matches!(read_redirect(&ok, &pkce.state, REDIRECT_URI), Redirect::Code(c) if c == "abc"));
        let forged = tauri::Url::parse(&format!("{REDIRECT_URI}?code=abc&state=evil")).unwrap();
        assert!(matches!(read_redirect(&forged, &pkce.state, REDIRECT_URI), Redirect::Denied(_)));
        let other = tauri::Url::parse("https://login.live.com/ppsecure/post.srf").unwrap();
        assert!(matches!(read_redirect(&other, &pkce.state, REDIRECT_URI), Redirect::Ignore));
        let local = "http://localhost:51234";
        let (_, pkce) = authorize_url("cid", local);
        let favicon = tauri::Url::parse(&format!("{local}/favicon.ico")).unwrap();
        assert!(matches!(read_redirect(&favicon, &pkce.state, local), Redirect::Ignore));
        let back = tauri::Url::parse(&format!("{local}/?code=xyz&state={}", pkce.state)).unwrap();
        assert!(matches!(read_redirect(&back, &pkce.state, local), Redirect::Code(c) if c == "xyz"));
    }
}
