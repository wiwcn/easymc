//! Host ↔ container path map and volume builders (Slice C).

use easymc_core::{ContainerVolume, Instance, Paths};

/// In-container mount roots (stable contract for launch arg rewrite + Agent).
pub const CTR_MINECRAFT: &str = "/data/.minecraft";
pub const CTR_LIBRARIES: &str = "/data/libraries";
pub const CTR_ASSETS: &str = "/data/assets";
pub const CTR_VERSIONS: &str = "/data/versions";
pub const CTR_NATIVES: &str = "/data/natives";

/// Build the standard volume set for a container-backed instance.
pub fn default_volumes(paths: &Paths, instance: &Instance) -> Vec<ContainerVolume> {
    vec![
        ContainerVolume::rw(&instance.game_dir, CTR_MINECRAFT),
        ContainerVolume::ro(
            paths.libraries_dir().to_string_lossy().as_ref(),
            CTR_LIBRARIES,
        ),
        ContainerVolume::ro(paths.assets_dir().to_string_lossy().as_ref(), CTR_ASSETS),
        ContainerVolume::ro(
            paths.versions_dir().to_string_lossy().as_ref(),
            CTR_VERSIONS,
        ),
        ContainerVolume::ro(paths.natives_dir().to_string_lossy().as_ref(), CTR_NATIVES),
    ]
}

/// Rewrite a host absolute path to the in-container path when it sits under a known mount.
pub fn remap_host_path(host: &str, volumes: &[ContainerVolume]) -> String {
    // Longest host prefix wins (game_dir may nest under data root).
    let mut best: Option<(&ContainerVolume, usize)> = None;
    for v in volumes {
        let prefix = v.host.trim_end_matches(['/', '\\']);
        if host == prefix
            || host.starts_with(&format!("{prefix}/"))
            || host.starts_with(&format!("{prefix}\\"))
        {
            let n = prefix.len();
            if best.map(|(_, len)| n > len).unwrap_or(true) {
                best = Some((v, n));
            }
        }
    }
    let Some((vol, len)) = best else {
        return host.to_string();
    };
    let rest = &host[len..];
    let rest = rest.trim_start_matches(['/', '\\']).replace('\\', "/");
    if rest.is_empty() {
        vol.container.clone()
    } else {
        format!("{}/{}", vol.container.trim_end_matches('/'), rest)
    }
}

/// Remap every path-looking token in a JVM/game arg list.
pub fn remap_launch_args(args: &[String], volumes: &[ContainerVolume]) -> Vec<String> {
    args.iter()
        .map(|a| {
            // Classpath: split on : or ; then remap each entry.
            if a.contains(".jar") && (a.contains(':') || a.contains(';')) {
                let sep = if a.contains(';') { ';' } else { ':' };
                let remapped: Vec<String> = a
                    .split(sep)
                    .map(|p| remap_host_path(p, volumes))
                    .collect();
                // Inside Linux container always use ':'
                remapped.join(":")
            } else if a.starts_with('/') || (a.len() > 2 && a.as_bytes()[1] == b':') {
                remap_host_path(a, volumes)
            } else if let Some(idx) = a.find('=') {
                // -Djava.library.path=/host/...
                let (k, v) = a.split_at(idx);
                let val = &v[1..];
                if val.starts_with('/') || (val.len() > 2 && val.as_bytes()[1] == b':') {
                    format!("{}={}", k, remap_host_path(val, volumes))
                } else {
                    a.clone()
                }
            } else {
                a.clone()
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remap_longest_prefix() {
        let vols = vec![
            ContainerVolume::ro("/data/EasyMC/libraries", CTR_LIBRARIES),
            ContainerVolume::rw("/data/EasyMC/instances/abc/.minecraft", CTR_MINECRAFT),
        ];
        assert_eq!(
            remap_host_path("/data/EasyMC/libraries/com/foo.jar", &vols),
            "/data/libraries/com/foo.jar"
        );
        assert_eq!(
            remap_host_path("/data/EasyMC/instances/abc/.minecraft", &vols),
            CTR_MINECRAFT
        );
        assert_eq!(
            remap_host_path("/data/EasyMC/instances/abc/.minecraft/saves/w", &vols),
            "/data/.minecraft/saves/w"
        );
    }

    #[test]
    fn remap_classpath() {
        let vols = vec![ContainerVolume::ro("/libs", CTR_LIBRARIES)];
        let args = vec!["/libs/a.jar:/libs/b.jar".into()];
        let out = remap_launch_args(&args, &vols);
        assert_eq!(out[0], "/data/libraries/a.jar:/data/libraries/b.jar");
    }
}
