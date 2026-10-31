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

fn looks_like_abs_path(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }
    s.starts_with('/') || (s.len() > 2 && s.as_bytes()[1] == b':' && s.as_bytes()[0].is_ascii_alphabetic())
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

/// Remap a classpath-style or path-list string (split on `:` / `;`).
///
/// Drive-letter paths (`C:\…`) keep their colon; list separators use `;` on Windows-style
/// lists and `:` otherwise. Result always joins with `:` for the Linux container.
pub fn remap_path_list(list: &str, volumes: &[ContainerVolume]) -> String {
    if list.is_empty() {
        return String::new();
    }
    let parts = split_path_list(list);
    parts
        .into_iter()
        .map(|p| remap_host_path(&p, volumes))
        .collect::<Vec<_>>()
        .join(":")
}

fn split_path_list(list: &str) -> Vec<String> {
    // Prefer `;` when present (Windows classpath). Otherwise split on `:` but keep `C:` drives.
    if list.contains(';') {
        return list
            .split(';')
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .collect();
    }
    let mut out = Vec::new();
    let bytes = list.as_bytes();
    let mut start = 0usize;
    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i] == b':' {
            // Windows drive letter: X: at start of this segment
            let seg_start = start;
            let is_drive = i == seg_start + 1
                && bytes[seg_start].is_ascii_alphabetic()
                && (i + 1 >= bytes.len()
                    || bytes[i + 1] == b'\\'
                    || bytes[i + 1] == b'/');
            if is_drive {
                i += 1;
                continue;
            }
            if i > start {
                out.push(list[start..i].to_string());
            }
            start = i + 1;
        }
        i += 1;
    }
    if start < list.len() {
        out.push(list[start..].to_string());
    }
    out
}

fn remap_one_arg(a: &str, volumes: &[ContainerVolume]) -> String {
    // Classpath / multi-jar list
    if a.contains(".jar") && (a.contains(':') || a.contains(';')) {
        return remap_path_list(a, volumes);
    }
    if looks_like_abs_path(a) {
        return remap_host_path(a, volumes);
    }
    if let Some(idx) = a.find('=') {
        // -Djava.library.path=/host/... or multi-path after =
        let (k, v) = a.split_at(idx);
        let val = &v[1..];
        if val.is_empty() {
            return a.to_string();
        }
        if looks_like_abs_path(val) || val.contains(';') || (val.contains(':') && val.contains('/'))
        {
            return format!("{}={}", k, remap_path_list(val, volumes));
        }
    }
    a.to_string()
}

/// Remap every path-looking token in a JVM/game arg list.
///
/// Also rewrites the argument immediately after `-cp` / `-classpath`.
pub fn remap_launch_args(args: &[String], volumes: &[ContainerVolume]) -> Vec<String> {
    let mut out = Vec::with_capacity(args.len());
    let mut remap_next = false;
    for a in args {
        if remap_next {
            out.push(remap_one_arg(a, volumes));
            remap_next = false;
            continue;
        }
        let lower = a.to_ascii_lowercase();
        if lower == "-cp" || lower == "-classpath" {
            out.push(a.clone());
            remap_next = true;
            continue;
        }
        out.push(remap_one_arg(a, volumes));
    }
    out
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

    #[test]
    fn remap_cp_flag_next_arg() {
        let vols = vec![ContainerVolume::ro("/libs", CTR_LIBRARIES)];
        let args = vec![
            "-cp".into(),
            "/libs/a.jar;/libs/b.jar".into(),
            "net.minecraft.client.main.Main".into(),
        ];
        let out = remap_launch_args(&args, &vols);
        assert_eq!(out[1], "/data/libraries/a.jar:/data/libraries/b.jar");
        assert_eq!(out[2], "net.minecraft.client.main.Main");
    }

    #[test]
    fn remap_single_jar_abs() {
        let vols = vec![ContainerVolume::ro("/libs", CTR_LIBRARIES)];
        let out = remap_launch_args(&["/libs/alone.jar".into()], &vols);
        assert_eq!(out[0], "/data/libraries/alone.jar");
    }

    #[test]
    fn remap_library_path_list() {
        let vols = vec![
            ContainerVolume::ro("/natives", CTR_NATIVES),
            ContainerVolume::ro("/libs", CTR_LIBRARIES),
        ];
        let out = remap_launch_args(
            &["-Djava.library.path=/natives/1.21:/libs/extra".into()],
            &vols,
        );
        assert_eq!(
            out[0],
            "-Djava.library.path=/data/natives/1.21:/data/libraries/extra"
        );
    }

    #[test]
    fn remap_windows_drive_in_list() {
        let vols = vec![ContainerVolume::ro(r"C:\EasyMC\libraries", CTR_LIBRARIES)];
        let list = r"C:\EasyMC\libraries\a.jar;C:\EasyMC\libraries\b.jar";
        assert_eq!(
            remap_path_list(list, &vols),
            "/data/libraries/a.jar:/data/libraries/b.jar"
        );
    }

    #[test]
    fn split_keeps_drive_letter() {
        let parts = split_path_list(r"C:\libs\a.jar:C:\libs\b.jar");
        // Without `;`, colon after drive is kept; second `C:` starts new? 
        // Actually C:\libs\a.jar:C:\libs\b.jar — after jar the `:` before second C splits.
        assert_eq!(parts.len(), 2);
        assert!(parts[0].starts_with("C:"));
        assert!(parts[1].starts_with("C:"));
    }
}
