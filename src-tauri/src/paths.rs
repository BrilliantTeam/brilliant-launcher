use std::path::{Path, PathBuf};

pub fn root() -> PathBuf {
    if let Some(custom) = std::env::var_os("BRILLIANT_LAUNCHER_HOME") {
        return PathBuf::from(custom);
    }
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("BrilliantLauncher")
}

pub fn config_file() -> PathBuf {
    root().join("config.json")
}
pub fn accounts_file() -> PathBuf {
    root().join("accounts.json")
}

pub fn shared() -> PathBuf {
    root().join("shared")
}
pub fn versions() -> PathBuf {
    shared().join("versions")
}
pub fn version_dir(id: &str) -> PathBuf {
    versions().join(id)
}
pub fn version_json(id: &str) -> PathBuf {
    version_dir(id).join(format!("{id}.json"))
}
pub fn version_jar(id: &str) -> PathBuf {
    version_dir(id).join(format!("{id}.jar"))
}
pub fn libraries() -> PathBuf {
    shared().join("libraries")
}
pub fn assets() -> PathBuf {
    shared().join("assets")
}
pub fn natives(version_id: &str) -> PathBuf {
    shared().join("natives").join(version_id)
}
pub fn java_root() -> PathBuf {
    shared().join("java")
}
pub fn logs() -> PathBuf {
    root().join("logs")
}

pub fn instances() -> PathBuf {
    root().join("instances")
}
pub fn trash() -> PathBuf {
    root().join("trash")
}

pub const TRASH_DAYS: u64 = 30;

pub fn discard<P: AsRef<Path>>(context: &str, items: &[(P, &str)]) -> std::io::Result<PathBuf> {
    discard_in(&trash(), context, items)
}

fn discard_in<P: AsRef<Path>>(trash: &Path, context: &str, items: &[(P, &str)]) -> std::io::Result<PathBuf> {
    let label: String = context.chars().map(|c| if "\\/:*?\"<>|".contains(c) || c.is_control() { '_' } else { c }).take(60).collect();
    let base = trash.join(label.trim().trim_end_matches('.'));
    let mut bin = base.clone();
    let mut n = 2;
    while bin.exists() {
        bin = PathBuf::from(format!("{} ({n})", base.display()));
        n += 1;
    }
    for (src, rel) in items {
        let dest = bin.join(rel);
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)?;
        }
        move_path(src.as_ref(), &dest)?;
    }
    Ok(bin)
}

fn move_path(src: &Path, dest: &Path) -> std::io::Result<()> {
    if std::fs::rename(src, dest).is_ok() {
        return Ok(());
    }
    if src.is_dir() {
        copy_tree(src, dest)?;
        std::fs::remove_dir_all(src)
    } else {
        std::fs::copy(src, dest)?;
        std::fs::remove_file(src)
    }
}

fn copy_tree(src: &Path, dest: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dest)?;
    for entry in std::fs::read_dir(src)?.flatten() {
        let to = dest.join(entry.file_name());
        let kind = entry.file_type()?;
        if kind.is_symlink() {
            continue;
        }
        if kind.is_dir() {
            copy_tree(&entry.path(), &to)?;
        } else {
            std::fs::copy(entry.path(), &to)?;
        }
    }
    Ok(())
}

pub fn purge_trash(days: u64) -> u64 {
    purge_in(&trash(), days)
}

fn purge_in(trash: &Path, days: u64) -> u64 {
    let limit = std::time::SystemTime::now() - std::time::Duration::from_secs(days * 86_400);
    let mut freed = 0;
    for entry in std::fs::read_dir(trash).map(|d| d.flatten().collect::<Vec<_>>()).unwrap_or_default() {
        let old = entry.metadata().and_then(|m| m.modified()).map(|t| t < limit).unwrap_or(false);
        if old {
            freed += size_of(&entry.path());
            let path = entry.path();
            let _ = if path.is_dir() { std::fs::remove_dir_all(&path) } else { std::fs::remove_file(&path) };
        }
    }
    freed
}

fn size_of(path: &Path) -> u64 {
    match std::fs::symlink_metadata(path) {
        Ok(m) if m.is_dir() => std::fs::read_dir(path).map(|d| d.flatten().map(|e| size_of(&e.path())).sum()).unwrap_or(0),
        Ok(m) => m.len(),
        Err(_) => 0,
    }
}

pub fn valid_instance_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 80 && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}
pub fn instance_dir(id: &str) -> PathBuf {
    instances().join(if valid_instance_id(id) { id } else { "_invalid" })
}
pub fn instance_json(id: &str) -> PathBuf {
    instance_dir(id).join("instance.json")
}
pub fn game_dir(id: &str) -> PathBuf {
    instance_dir(id).join(".minecraft")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn instance_dir_never_leaves_instances_folder() {
        for id in ["../../evil", "..", "a/b", "a\\b", "", "C:\\x"] {
            assert_eq!(instance_dir(id), instances().join("_invalid"), "{id}");
        }
        assert_eq!(instance_dir("26-3-1792333628"), instances().join("26-3-1792333628"));
    }

    #[test]
    fn discard_moves_into_named_trash_folder() {
        let home = std::env::temp_dir().join(format!("trash-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&home);
        let bin = home.join("trash");
        let src = home.join("mods");
        std::fs::create_dir_all(src.join("sub")).unwrap();
        std::fs::write(src.join("a.jar"), "x").unwrap();
        std::fs::write(src.join("sub/b.txt"), "y").unwrap();
        let first = discard_in(&bin, "模組 a/b:c", &[(src.join("a.jar"), "a.jar"), (src.join("sub"), "config/sub")]).unwrap();
        assert_eq!(first, bin.join("模組 a_b_c"));
        assert!(first.join("a.jar").is_file() && first.join("config/sub/b.txt").is_file());
        assert!(!src.join("a.jar").exists() && !src.join("sub").exists());
        std::fs::write(src.join("a.jar"), "z").unwrap();
        assert_eq!(discard_in(&bin, "模組 a/b:c", &[(src.join("a.jar"), "a.jar")]).unwrap(), bin.join("模組 a_b_c (2)"));
        assert_eq!(purge_in(&bin, 1), 0);
        std::thread::sleep(std::time::Duration::from_millis(20));
        assert_eq!(purge_in(&bin, 0), 3);
        assert!(std::fs::read_dir(&bin).unwrap().next().is_none());
        let _ = std::fs::remove_dir_all(&home);
    }
}
