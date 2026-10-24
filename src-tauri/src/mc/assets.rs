use super::version::Resolved;
use crate::err::R;
use crate::net::{self, DlFile};
use crate::paths;
use crate::bail;
use anyhow::Context;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use tauri::AppHandle;

const RESOURCES: &str = "https://resources.download.minecraft.net";

#[derive(Deserialize, Default)]
pub struct AssetIndex {
    #[serde(default)]
    pub objects: BTreeMap<String, AssetObject>,
    #[serde(default)]
    pub virtual_: Option<bool>,
    #[serde(default)]
    pub map_to_resources: Option<bool>,
}

#[derive(Deserialize, Clone)]
pub struct AssetObject {
    pub hash: String,
    #[serde(default)]
    pub size: u64,
}

#[derive(Deserialize)]
struct RawIndex {
    #[serde(default)]
    objects: BTreeMap<String, AssetObject>,
    #[serde(default, rename = "virtual")]
    is_virtual: Option<bool>,
    #[serde(default)]
    map_to_resources: Option<bool>,
}

pub fn valid_hash(hash: &str) -> bool {
    hash.len() == 40 && hash.chars().all(|c| c.is_ascii_hexdigit())
}

pub fn object_path(hash: &str) -> PathBuf {
    let hash = if valid_hash(hash) { hash } else { "0000000000000000000000000000000000000000" };
    paths::assets().join("objects").join(&hash[..2]).join(hash)
}

pub async fn ensure_index(version: &Resolved, mirror: bool) -> R<AssetIndex> {
    let Some(index_ref) = &version.asset_index else {
        bail!("{} 缺少 assetIndex", version.id);
    };
    if index_ref.id.is_empty() || index_ref.id.contains(['/', '\\']) || index_ref.id.contains("..") {
        bail!("資源索引名稱不正確：{}", index_ref.id);
    }
    let file = paths::assets().join("indexes").join(format!("{}.json", index_ref.id));

    let bytes = match tokio::fs::read(&file).await {
        Ok(b)
            if index_ref
                .sha1
                .as_deref()
                .is_none_or(|want| net::sha1_hex(&b).eq_ignore_ascii_case(want)) =>
        {
            b
        }
        _ => {
            let b = net::get_bytes(&index_ref.url, mirror).await?;
            net::write_file(&file, &b, false).await?;
            b
        }
    };

    let raw: RawIndex =
        serde_json::from_slice(&bytes).with_context(|| format!("解析 {}.json 失敗", index_ref.id))?;
    Ok(AssetIndex {
        objects: raw.objects,
        virtual_: raw.is_virtual,
        map_to_resources: raw.map_to_resources,
    })
}

pub fn jobs(index: &AssetIndex) -> Vec<DlFile> {
    let mut seen = std::collections::BTreeSet::new();
    index
        .objects
        .values()
        .filter(|o| valid_hash(&o.hash) && seen.insert(o.hash.clone()))
        .map(|o| {
            DlFile::new(format!("{RESOURCES}/{}/{}", &o.hash[..2], o.hash), object_path(&o.hash))
                .sha1(Some(o.hash.clone()))
                .size(o.size)
        })
        .collect()
}

pub async fn materialise_legacy(
    app: &AppHandle,
    index: &AssetIndex,
    index_id: &str,
    game_dir: &Path,
) -> R<Option<PathBuf>> {
    let is_virtual = index.virtual_.unwrap_or(false);
    let to_resources = index.map_to_resources.unwrap_or(false);
    if !is_virtual && !to_resources {
        return Ok(None);
    }

    let dest = if to_resources {
        game_dir.join("resources")
    } else {
        paths::assets().join("virtual").join(index_id)
    };

    net::emit_stage(app, "assets", "整理舊版資源檔");
    for (name, obj) in &index.objects {
        if !valid_hash(&obj.hash) || crate::mrpack::safe_relative(name).is_none() {
            continue;
        }
        let target = dest.join(name);
        if tokio::fs::metadata(&target).await.map(|m| m.len()).ok() == Some(obj.size) {
            continue;
        }
        if let Some(parent) = target.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        tokio::fs::copy(object_path(&obj.hash), &target)
            .await
            .with_context(|| format!("複製資源失敗：{name}"))?;
    }
    Ok(Some(dest))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bad_hashes_never_panic_or_escape() {
        assert!(valid_hash("a94a8fe5ccb19ba61c4c0873d391e987982fbbd3") && !valid_hash("a") && !valid_hash("../../../../../../../../../../../../evil"));
        for bad in ["", "a", "../x"] {
            assert!(object_path(bad).starts_with(paths::assets().join("objects")));
        }
    }
}
