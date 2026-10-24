use crate::err::R;
use crate::{bail, net, paths};
use anyhow::Context;
use serde::{Deserialize, Serialize};

const META: &str = "https://meta.fabricmc.net/v2";
pub const FABRIC_MAVEN: &str = "https://maven.fabricmc.net";

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct GameVersion {
    pub version: String,
    #[serde(default)]
    pub stable: bool,
}

#[derive(Deserialize, Clone, Debug)]
struct LoaderEntry {
    loader: LoaderInfo,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct LoaderInfo {
    pub version: String,
    #[serde(default)]
    pub stable: bool,
}

pub async fn game_versions(mirror: bool) -> R<Vec<GameVersion>> {
    net::get_json(&format!("{META}/versions/game"), mirror).await
}

pub async fn loaders(game: &str, mirror: bool) -> R<Vec<LoaderInfo>> {
    let list: Vec<LoaderEntry> =
        net::get_json(&format!("{META}/versions/loader/{game}"), mirror).await?;
    Ok(list.into_iter().map(|e| e.loader).collect())
}

pub async fn install(game: &str, loader: &str, mirror: bool) -> R<String> {
    let id = format!("fabric-loader-{loader}-{game}");
    if paths::version_json(&id).is_file() {
        return Ok(id);
    }

    let url = format!("{META}/versions/loader/{game}/{loader}/profile/json");
    let bytes = net::get_bytes(&url, mirror).await?;
    let profile: serde_json::Value =
        serde_json::from_slice(&bytes).with_context(|| format!("解析 Fabric profile 失敗：{url}"))?;

    let real_id = profile
        .get("id")
        .and_then(|v| v.as_str())
        .unwrap_or(&id)
        .to_string();
    if !paths::valid_version_id(&real_id) {
        bail!("Fabric 回傳的版本名稱不正確：{real_id}");
    }
    net::write_file(&paths::version_json(&real_id), &bytes, false).await?;
    Ok(real_id)
}
