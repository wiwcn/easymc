//! Download source / mirror URL remapping (Official + BMCLAPI).
//!
//! BMCLAPI (`https://bmclapi2.bangbang93.com`) is the standard China-friendly
//! remap used by HMCL / PCL-class launchers. Historical MCBBS
//! (`download.mcbbs.net`) pointed at the same backend but is dead (verified
//! 2026-10) — we do not hardcode it.

use easymc_core::DownloadSource;

/// BMCLAPI v2 base (current public endpoint).
pub const BMCLAPI_BASE: &str = "https://bmclapi2.bangbang93.com";

/// Rewrite an official Mojang / loader URL for the chosen source.
/// Unknown hosts are left unchanged (caller may still hit them officially).
pub fn rewrite_url(source: DownloadSource, url: &str) -> String {
    match source {
        DownloadSource::Official => url.to_string(),
        DownloadSource::Bmclapi => rewrite_bmclapi(url),
    }
}

fn rewrite_bmclapi(url: &str) -> String {
    // Host-only remaps (path preserved).
    let host_map: &[(&str, &str)] = &[
        ("https://piston-meta.mojang.com", BMCLAPI_BASE),
        ("https://launchermeta.mojang.com", BMCLAPI_BASE),
        ("https://piston-data.mojang.com", BMCLAPI_BASE),
        ("https://launcher.mojang.com", BMCLAPI_BASE),
        (
            "https://meta.fabricmc.net",
            "https://bmclapi2.bangbang93.com/fabric-meta",
        ),
        (
            "https://meta.quiltmc.org",
            "https://bmclapi2.bangbang93.com/quilt-meta",
        ),
    ];
    for (from, to) in host_map {
        if let Some(rest) = url.strip_prefix(from) {
            return format!("{to}{rest}");
        }
    }

    // Path-prefix remaps onto BMCL `/maven` or `/assets`.
    let path_map: &[(&str, &str)] = &[
        (
            "https://libraries.minecraft.net",
            "https://bmclapi2.bangbang93.com/maven",
        ),
        (
            "https://resources.download.minecraft.net",
            "https://bmclapi2.bangbang93.com/assets",
        ),
        (
            "https://maven.fabricmc.net",
            "https://bmclapi2.bangbang93.com/maven",
        ),
        (
            "https://maven.minecraftforge.net",
            "https://bmclapi2.bangbang93.com/maven",
        ),
        // NeoForge releases maven (API under /api is not remapped — leave official).
        (
            "https://maven.neoforged.net/releases",
            "https://bmclapi2.bangbang93.com/maven",
        ),
        (
            "https://maven.quiltmc.org/repository/release",
            "https://bmclapi2.bangbang93.com/maven",
        ),
        (
            "https://maven.quiltmc.org",
            "https://bmclapi2.bangbang93.com/maven",
        ),
    ];
    for (from, to) in path_map {
        if let Some(rest) = url.strip_prefix(from) {
            return format!("{to}{rest}");
        }
    }

    url.to_string()
}

/// True when rewriting actually changed the URL.
pub fn is_remapped(source: DownloadSource, url: &str) -> bool {
    rewrite_url(source, url) != url
}

#[cfg(test)]
mod tests {
    use super::*;
    use easymc_core::DownloadSource;

    #[test]
    fn official_is_identity() {
        let u = "https://libraries.minecraft.net/com/mojang/logging/1.1.1/logging-1.1.1.jar";
        assert_eq!(rewrite_url(DownloadSource::Official, u), u);
    }

    #[test]
    fn bmcl_mojang_meta_and_data() {
        assert_eq!(
            rewrite_url(
                DownloadSource::Bmclapi,
                "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json"
            ),
            "https://bmclapi2.bangbang93.com/mc/game/version_manifest_v2.json"
        );
        assert_eq!(
            rewrite_url(
                DownloadSource::Bmclapi,
                "https://launchermeta.mojang.com/mc/game/version_manifest_v2.json"
            ),
            "https://bmclapi2.bangbang93.com/mc/game/version_manifest_v2.json"
        );
        assert_eq!(
            rewrite_url(
                DownloadSource::Bmclapi,
                "https://piston-data.mojang.com/v1/objects/abc/client.jar"
            ),
            "https://bmclapi2.bangbang93.com/v1/objects/abc/client.jar"
        );
    }

    #[test]
    fn bmcl_libraries_and_assets() {
        assert_eq!(
            rewrite_url(
                DownloadSource::Bmclapi,
                "https://libraries.minecraft.net/com/mojang/logging/1.1.1/logging-1.1.1.jar"
            ),
            "https://bmclapi2.bangbang93.com/maven/com/mojang/logging/1.1.1/logging-1.1.1.jar"
        );
        assert_eq!(
            rewrite_url(
                DownloadSource::Bmclapi,
                "https://resources.download.minecraft.net/1f/1f5dd088c7cc1f449ec2a4ae81cc6a2b8b7c57d0"
            ),
            "https://bmclapi2.bangbang93.com/assets/1f/1f5dd088c7cc1f449ec2a4ae81cc6a2b8b7c57d0"
        );
    }

    #[test]
    fn bmcl_fabric_quilt_forge() {
        assert_eq!(
            rewrite_url(
                DownloadSource::Bmclapi,
                "https://meta.fabricmc.net/v2/versions/loader/1.21.1"
            ),
            "https://bmclapi2.bangbang93.com/fabric-meta/v2/versions/loader/1.21.1"
        );
        assert_eq!(
            rewrite_url(
                DownloadSource::Bmclapi,
                "https://maven.fabricmc.net/net/fabricmc/fabric-loader/0.16.9/fabric-loader-0.16.9.jar"
            ),
            "https://bmclapi2.bangbang93.com/maven/net/fabricmc/fabric-loader/0.16.9/fabric-loader-0.16.9.jar"
        );
        assert_eq!(
            rewrite_url(
                DownloadSource::Bmclapi,
                "https://meta.quiltmc.org/v3/versions/loader/1.21.1"
            ),
            "https://bmclapi2.bangbang93.com/quilt-meta/v3/versions/loader/1.21.1"
        );
        assert_eq!(
            rewrite_url(
                DownloadSource::Bmclapi,
                "https://maven.quiltmc.org/repository/release/org/quiltmc/quilt-loader/0.26.0/quilt-loader-0.26.0.jar"
            ),
            "https://bmclapi2.bangbang93.com/maven/org/quiltmc/quilt-loader/0.26.0/quilt-loader-0.26.0.jar"
        );
        assert_eq!(
            rewrite_url(
                DownloadSource::Bmclapi,
                "https://maven.minecraftforge.net/net/minecraftforge/forge/1.21.1-52.1.20/forge-1.21.1-52.1.20-installer.jar"
            ),
            "https://bmclapi2.bangbang93.com/maven/net/minecraftforge/forge/1.21.1-52.1.20/forge-1.21.1-52.1.20-installer.jar"
        );
        assert_eq!(
            rewrite_url(
                DownloadSource::Bmclapi,
                "https://maven.neoforged.net/releases/net/neoforged/neoforge/21.1.80/neoforge-21.1.80-installer.jar"
            ),
            "https://bmclapi2.bangbang93.com/maven/net/neoforged/neoforge/21.1.80/neoforge-21.1.80-installer.jar"
        );
    }

    #[test]
    fn neoforge_api_not_remapped() {
        let u = "https://maven.neoforged.net/api/maven/versions/releases/net/neoforged/neoforge";
        assert_eq!(rewrite_url(DownloadSource::Bmclapi, u), u);
        assert!(!is_remapped(DownloadSource::Bmclapi, u));
    }

    #[test]
    fn unknown_host_unchanged() {
        let u = "https://cdn.modrinth.com/data/foo.jar";
        assert_eq!(rewrite_url(DownloadSource::Bmclapi, u), u);
    }
}
