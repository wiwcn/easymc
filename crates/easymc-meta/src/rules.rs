use crate::types::Rule;

pub fn current_os_name() -> &'static str {
    if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "osx"
    } else {
        "linux"
    }
}

pub fn current_arch() -> &'static str {
    if cfg!(target_arch = "x86_64") {
        "x86_64"
    } else if cfg!(target_arch = "x86") {
        "x86"
    } else if cfg!(target_arch = "aarch64") {
        "arm64"
    } else {
        "unknown"
    }
}

/// Evaluate Mojang library / argument rules for this host.
pub fn rules_allow(rules: Option<&[Rule]>, features: &[(&str, bool)]) -> bool {
    let Some(rules) = rules else {
        return true;
    };
    if rules.is_empty() {
        return true;
    }
    let mut allow = false;
    for rule in rules {
        let os_ok = match &rule.os {
            None => true,
            Some(os) => {
                let name_ok = os.name.as_deref().map(|n| n == current_os_name()).unwrap_or(true);
                let arch_ok = os
                    .arch
                    .as_deref()
                    .map(|a| a == current_arch() || (a == "x86" && cfg!(target_arch = "x86")))
                    .unwrap_or(true);
                name_ok && arch_ok
            }
        };
        let features_ok = match &rule.features {
            None => true,
            Some(map) => map.iter().all(|(k, want)| {
                features
                    .iter()
                    .find(|(fk, _)| *fk == k.as_str())
                    .map(|(_, v)| v)
                    == Some(want)
            }),
        };
        if os_ok && features_ok {
            allow = rule.action == "allow";
        }
    }
    allow
}

pub fn is_native_library(name: &str, path: Option<&str>) -> bool {
    let p = path.unwrap_or("");
    name.contains(":natives-") || p.contains("natives-")
}
