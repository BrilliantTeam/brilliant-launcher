use super::rules::Rule;
use crate::err::R;
use crate::{net, paths};
use crate::bail;
use anyhow::Context;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Deserialize, Serialize, Clone, Debug, Default)]
pub struct Artifact {
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub sha1: Option<String>,
    #[serde(default)]
    pub size: u64,
    #[serde(default)]
    pub url: String,
}

#[derive(Deserialize, Serialize, Clone, Debug, Default)]
pub struct LibDownloads {
    #[serde(default)]
    pub artifact: Option<Artifact>,
    #[serde(default)]
    pub classifiers: Option<BTreeMap<String, Artifact>>,
}

#[derive(Deserialize, Serialize, Clone, Debug, Default)]
pub struct Extract {
    #[serde(default)]
    pub exclude: Vec<String>,
}

#[derive(Deserialize, Serialize, Clone, Debug, Default)]
pub struct Library {
    pub name: String,
    #[serde(default)]
    pub downloads: Option<LibDownloads>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub rules: Vec<Rule>,
    #[serde(default)]
    pub natives: Option<BTreeMap<String, String>>,
    #[serde(default)]
    pub extract: Option<Extract>,
}

#[derive(Deserialize, Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct AssetIndexRef {
    pub id: String,
    #[serde(default)]
    pub sha1: Option<String>,
    #[serde(default)]
    pub size: u64,
    #[serde(default)]
    pub total_size: u64,
    pub url: String,
}

#[derive(Deserialize, Serialize, Clone, Debug, Default)]
pub struct ClientDownloads {
    #[serde(default)]
    pub client: Option<Artifact>,
}

#[derive(Deserialize, Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct JavaVersionRef {
    #[serde(default)]
    pub component: String,
    #[serde(default)]
    pub major_version: u32,
}

#[derive(Deserialize, Serialize, Clone, Debug, Default)]
pub struct Arguments {
    #[serde(default)]
    pub game: Vec<Value>,
    #[serde(default)]
    pub jvm: Vec<Value>,
}

#[derive(Deserialize, Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct VersionJson {
    pub id: String,
    pub inherits_from: Option<String>,
    pub main_class: Option<String>,
    pub assets: Option<String>,
    pub asset_index: Option<AssetIndexRef>,
    pub downloads: Option<ClientDownloads>,
    pub libraries: Vec<Library>,
    pub arguments: Option<Arguments>,
    pub minecraft_arguments: Option<String>,
    pub java_version: Option<JavaVersionRef>,
    #[serde(rename = "type")]
    pub kind: Option<String>,
    pub release_time: Option<String>,
}

pub fn maven_path(name: &str) -> String {
    let (coord, ext) = name.split_once('@').unwrap_or((name, "jar"));
    let parts: Vec<&str> = coord.split(':').collect();
    let group = parts.first().copied().unwrap_or("").replace('.', "/");
    let artifact = parts.get(1).copied().unwrap_or("");
    let version = parts.get(2).copied().unwrap_or("");
    let file = match parts.get(3) {
        Some(classifier) => format!("{artifact}-{version}-{classifier}.{ext}"),
        None => format!("{artifact}-{version}.{ext}"),
    };
    format!("{group}/{artifact}/{version}/{file}")
}

pub fn maven_classifier(name: &str) -> Option<&str> {
    let (coord, _) = name.split_once('@').unwrap_or((name, "jar"));
    coord.split(':').nth(3)
}

pub fn maven_key(name: &str) -> String {
    let (coord, _) = name.split_once('@').unwrap_or((name, "jar"));
    let parts: Vec<&str> = coord.split(':').collect();
    let classifier = parts.get(3).copied().unwrap_or("");
    format!(
        "{}:{}:{}",
        parts.first().copied().unwrap_or(""),
        parts.get(1).copied().unwrap_or(""),
        classifier
    )
}

pub async fn read_local(id: &str) -> Option<VersionJson> {
    let bytes = tokio::fs::read(paths::version_json(id)).await.ok()?;
    match serde_json::from_slice::<VersionJson>(&bytes) {
        Ok(v) => Some(v),
        Err(e) => {
            eprintln!("[version] {id}.json 損毀，將重新下載：{e}");
            None
        }
    }
}

pub async fn ensure(id: &str, mirror: bool) -> R<VersionJson> {
    if let Some(v) = read_local(id).await {
        return Ok(v);
    }
    let manifest = super::manifest::fetch(mirror).await?;
    let entry = manifest
        .versions
        .iter()
        .find(|v| v.id == id)
        .with_context(|| format!("版本清單裡沒有 {id}，若是 Fabric 版本請先安裝"))?;
    let bytes = net::get_bytes(&entry.url, mirror).await?;
    if let Some(want) = &entry.sha1 {
        let got = net::sha1_hex(&bytes);
        if !got.eq_ignore_ascii_case(want) {
            bail!("{id}.json 校驗失敗：預期 {want}，實得 {got}");
        }
    }
    net::write_file(&paths::version_json(id), &bytes, false).await?;
    Ok(serde_json::from_slice(&bytes).with_context(|| format!("解析 {id}.json 失敗"))?)
}

#[derive(Clone, Debug)]
pub struct Resolved {
    pub id: String,
    pub base_id: String,
    pub main_class: String,
    pub assets: String,
    pub asset_index: Option<AssetIndexRef>,
    pub client: Option<Artifact>,
    pub libraries: Vec<Library>,
    pub arguments: Arguments,
    pub java_version: JavaVersionRef,
    pub kind: String,
}

const MAX_DEPTH: usize = 8;

pub async fn resolve(id: &str, mirror: bool) -> R<Resolved> {
    let mut chain = Vec::new();
    let mut current = id.to_string();
    for _ in 0..MAX_DEPTH {
        let v = ensure(&current, mirror).await?;
        let parent = v.inherits_from.clone();
        chain.push(v);
        match parent {
            Some(p) => current = p,
            None => break,
        }
    }
    if chain.len() >= MAX_DEPTH && chain.last().and_then(|v| v.inherits_from.clone()).is_some() {
        bail!("{id} 的 inheritsFrom 繼承過深，可能有循環");
    }

    let base = chain.last().expect("chain 至少有一項");
    let base_id = base.id.clone();

    let mut main_class = String::new();
    let mut assets = String::new();
    let mut asset_index = None;
    let mut client = None;
    let mut java_version = JavaVersionRef { component: "jre-legacy".into(), major_version: 8 };
    let mut kind = String::from("release");
    let mut game_args: Vec<Value> = Vec::new();
    let mut jvm_args: Vec<Value> = Vec::new();

    for v in chain.iter().rev() {
        if let Some(m) = &v.main_class {
            main_class = m.clone();
        }
        if let Some(a) = &v.assets {
            assets = a.clone();
        }
        if v.asset_index.is_some() {
            asset_index = v.asset_index.clone();
        }
        if let Some(d) = &v.downloads {
            if d.client.is_some() {
                client = d.client.clone();
            }
        }
        if let Some(j) = &v.java_version {
            java_version = j.clone();
        }
        if let Some(t) = &v.kind {
            kind = t.clone();
        }
        match (&v.arguments, &v.minecraft_arguments) {
            (Some(a), _) => {
                game_args.extend(a.game.iter().cloned());
                jvm_args.extend(a.jvm.iter().cloned());
            }
            (None, Some(legacy)) => {
                game_args.extend(legacy.split_whitespace().map(|s| Value::String(s.into())));
                if jvm_args.is_empty() {
                    jvm_args.extend(
                        LEGACY_JVM_ARGS.iter().map(|s| Value::String((*s).into())),
                    );
                }
            }
            (None, None) => {}
        }
    }

    let mut libraries: Vec<Library> = Vec::new();
    let mut seen: BTreeMap<String, usize> = BTreeMap::new();
    for v in chain.iter() {
        for lib in &v.libraries {
            let key = maven_key(&lib.name);
            if seen.contains_key(&key) {
                continue;
            }
            seen.insert(key, libraries.len());
            libraries.push(lib.clone());
        }
    }

    if main_class.is_empty() {
        bail!("{id} 缺少 mainClass");
    }
    if assets.is_empty() {
        assets = "legacy".into();
    }

    Ok(Resolved {
        id: id.to_string(),
        base_id,
        main_class,
        assets,
        asset_index,
        client,
        libraries,
        arguments: Arguments { game: game_args, jvm: jvm_args },
        java_version,
        kind,
    })
}

const LEGACY_JVM_ARGS: &[&str] = &[
    "-Djava.library.path=${natives_directory}",
    "-Dminecraft.launcher.brand=${launcher_name}",
    "-Dminecraft.launcher.version=${launcher_version}",
    "-cp",
    "${classpath}",
];
