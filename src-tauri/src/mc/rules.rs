use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Deserialize, Serialize, Clone, Debug, Default)]
pub struct OsRule {
    pub name: Option<String>,
    pub arch: Option<String>,
    pub version: Option<String>,
}

#[derive(Deserialize, Serialize, Clone, Debug, Default)]
pub struct Rule {
    pub action: String,
    pub os: Option<OsRule>,
    pub features: Option<BTreeMap<String, bool>>,
}

pub const fn os_name() -> &'static str {
    if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "osx"
    } else {
        "linux"
    }
}

pub const fn os_arch() -> &'static str {
    if cfg!(target_arch = "aarch64") {
        "arm64"
    } else if cfg!(target_arch = "x86") {
        "x86"
    } else {
        "x86_64"
    }
}

pub fn java_platform() -> &'static str {
    match (os_name(), os_arch()) {
        ("windows", "arm64") => "windows-arm64",
        ("windows", "x86") => "windows-x86",
        ("windows", _) => "windows-x64",
        ("osx", "arm64") => "mac-os-arm64",
        ("osx", _) => "mac-os",
        (_, "x86") => "linux-i386",
        _ => "linux",
    }
}

fn version_matches(pattern: &str) -> bool {
    let Some(prefix) = pattern.strip_prefix('^') else {
        return true;
    };
    let prefix = prefix.replace("\\.", ".");
    let actual = os_release();
    actual.starts_with(&prefix)
}

#[cfg(target_os = "windows")]
fn os_release() -> String {
    "10.0".into()
}

#[cfg(not(target_os = "windows"))]
fn os_release() -> String {
    String::new()
}

pub type Features = BTreeMap<String, bool>;

fn rule_applies(rule: &Rule, features: &Features) -> bool {
    if let Some(os) = &rule.os {
        if os.name.as_deref().is_some_and(|n| n != os_name()) {
            return false;
        }
        if os.arch.as_deref().is_some_and(|a| a != os_arch()) {
            return false;
        }
        if let Some(v) = &os.version {
            if !version_matches(v) {
                return false;
            }
        }
    }
    if let Some(want) = &rule.features {
        for (key, expected) in want {
            if features.get(key).copied().unwrap_or(false) != *expected {
                return false;
            }
        }
    }
    true
}

pub fn allowed(rules: &[Rule], features: &Features) -> bool {
    if rules.is_empty() {
        return true;
    }
    let mut allow = false;
    for rule in rules {
        if rule_applies(rule, features) {
            allow = rule.action == "allow";
        }
    }
    allow
}
