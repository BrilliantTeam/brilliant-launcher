use std::path::PathBuf;

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
}
