use super::rules::{self, Features};
use super::version::{maven_path, Library, Resolved};
use crate::err::R;
use crate::net::DlFile;
use crate::paths;
use anyhow::Context;
use std::path::PathBuf;

const DEFAULT_MAVEN: &str = "https://libraries.minecraft.net/";

pub struct LibPlan {
    pub downloads: Vec<DlFile>,
    pub classpath: Vec<PathBuf>,
    pub natives: Vec<(PathBuf, Vec<String>)>,
}

fn fill_arch(template: &str) -> String {
    let bits = if rules::os_arch() == "x86" { "32" } else { "64" };
    template.replace("${arch}", bits)
}

fn artifact_job(lib: &Library, classifier: Option<&str>) -> Option<(DlFile, PathBuf)> {
    let coord = match classifier {
        Some(c) => format!("{}:{c}", lib.name),
        None => lib.name.clone(),
    };

    let described = lib.downloads.as_ref().and_then(|d| match classifier {
        Some(c) => d.classifiers.as_ref()?.get(c).cloned(),
        None => d.artifact.clone(),
    });

    let rel = described
        .as_ref()
        .and_then(|a| a.path.clone())
        .unwrap_or_else(|| maven_path(&coord));
    let path = paths::libraries().join(&rel);

    let url = match described.as_ref().map(|a| a.url.clone()).filter(|u| !u.is_empty()) {
        Some(u) => u,
        None => format!("{}{rel}", lib.url.as_deref().unwrap_or(DEFAULT_MAVEN)),
    };

    let job = DlFile::new(url, path.clone())
        .sha1(described.as_ref().and_then(|a| a.sha1.clone()))
        .size(described.as_ref().map(|a| a.size).unwrap_or(0));
    Some((job, path))
}

pub fn plan(version: &Resolved, features: &Features) -> LibPlan {
    let mut out = LibPlan { downloads: Vec::new(), classpath: Vec::new(), natives: Vec::new() };

    for lib in &version.libraries {
        if !rules::allowed(&lib.rules, features) {
            continue;
        }

        if let Some(natives) = &lib.natives {
            if let Some(template) = natives.get(rules::os_name()) {
                let classifier = fill_arch(template);
                if let Some((job, path)) = artifact_job(lib, Some(&classifier)) {
                    out.downloads.push(job);
                    let exclude = lib.extract.as_ref().map(|e| e.exclude.clone()).unwrap_or_default();
                    out.natives.push((path, exclude));
                }
            }
            if lib.downloads.as_ref().and_then(|d| d.artifact.as_ref()).is_none() {
                continue;
            }
        }

        if let Some((job, path)) = artifact_job(lib, None) {
            if !out.classpath.contains(&path) {
                out.classpath.push(path);
            }
            out.downloads.push(job);
        }
    }

    out
}

pub fn extract_natives(jar: &PathBuf, dest: &PathBuf, exclude: &[String]) -> R<()> {
    std::fs::create_dir_all(dest)?;
    let file = std::fs::File::open(jar).with_context(|| format!("開啟失敗：{}", jar.display()))?;
    let mut zip = zip::ZipArchive::new(std::io::BufReader::new(file))
        .with_context(|| format!("解壓失敗：{}", jar.display()))?;

    for i in 0..zip.len() {
        let mut entry = zip.by_index(i)?;
        let Some((name, file_name)) = ({
            match entry.enclosed_name() {
                Some(path) => path
                    .file_name()
                    .map(|base| (path.to_string_lossy().replace('\\', "/"), base.to_os_string())),
                None => None,
            }
        }) else {
            continue;
        };
        if entry.is_dir() || name.starts_with("META-INF/") {
            continue;
        }
        if exclude.iter().any(|e| name.starts_with(e.trim_end_matches('/'))) {
            continue;
        }
        let target = dest.join(file_name);
        if std::fs::metadata(&target).map(|m| m.len()).ok() == Some(entry.size()) {
            continue;
        }
        let mut out = std::fs::File::create(&target)
            .with_context(|| format!("寫入失敗：{}", target.display()))?;
        std::io::copy(&mut entry, &mut out)?;
    }
    Ok(())
}
