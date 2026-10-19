use crate::err::R;
use crate::{net, paths};
use serde::{Deserialize, Serialize};
use std::sync::Mutex;

const URL: &str = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";
const TTL_SECS: i64 = 600;

#[derive(Deserialize, Serialize, Clone, Debug, Default)]
pub struct Latest {
    #[serde(default)]
    pub release: String,
    #[serde(default)]
    pub snapshot: String,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct VersionEntry {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub url: String,
    #[serde(default)]
    pub sha1: Option<String>,
    #[serde(default, rename = "releaseTime")]
    pub release_time: String,
}

#[derive(Deserialize, Serialize, Clone, Debug, Default)]
pub struct Manifest {
    #[serde(default)]
    pub latest: Latest,
    #[serde(default)]
    pub versions: Vec<VersionEntry>,
}

static CACHE: Mutex<Option<(i64, Manifest)>> = Mutex::new(None);

fn cache_file() -> std::path::PathBuf {
    paths::shared().join("version_manifest_v2.json")
}

pub async fn fetch(mirror: bool) -> R<Manifest> {
    let now = crate::config::now_secs();
    if let Ok(guard) = CACHE.lock() {
        if let Some((at, m)) = guard.as_ref() {
            if now - at < TTL_SECS {
                return Ok(m.clone());
            }
        }
    }

    let fetched: R<Manifest> = net::get_json(URL, mirror).await;
    let manifest = match fetched {
        Ok(m) => {
            if let Ok(bytes) = serde_json::to_vec(&m) {
                let _ = net::write_file(&cache_file(), &bytes, false).await;
            }
            m
        }
        Err(e) => match tokio::fs::read(cache_file()).await {
            Ok(bytes) => serde_json::from_slice(&bytes).map_err(|_| e)?,
            Err(_) => return Err(e),
        },
    };

    if let Ok(mut guard) = CACHE.lock() {
        *guard = Some((now, manifest.clone()));
    }
    Ok(manifest)
}
