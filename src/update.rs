use anyhow::{Context, Result, bail};
use self_update::backends::github::ReleaseList;
use self_update::update::Release;

const REPO_OWNER: &str = "cruzluna";
const REPO_NAME: &str = "jkl-2";
const BIN_NAME: &str = "jkl";

#[derive(Debug, Clone, PartialEq, Eq)]
struct UpdateConfig {
    repo_owner: &'static str,
    repo_name: &'static str,
    bin_name: &'static str,
    prerelease: bool,
}

fn update_config(prerelease: bool) -> UpdateConfig {
    UpdateConfig {
        repo_owner: REPO_OWNER,
        repo_name: REPO_NAME,
        bin_name: BIN_NAME,
        prerelease,
    }
}

fn target_triple(os: &str, arch: &str) -> Result<&'static str> {
    let os_part = match os {
        "macos" => "apple-darwin",
        "linux" => "unknown-linux-gnu",
        other => bail!("unsupported os: {other}"),
    };

    let arch_part = match arch {
        "x86_64" => "x86_64",
        "aarch64" => "aarch64",
        other => bail!("unsupported arch: {other}"),
    };

    Ok(match (arch_part, os_part) {
        ("x86_64", "apple-darwin") => "x86_64-apple-darwin",
        ("aarch64", "apple-darwin") => "aarch64-apple-darwin",
        ("x86_64", "unknown-linux-gnu") => "x86_64-unknown-linux-gnu",
        ("aarch64", "unknown-linux-gnu") => "aarch64-unknown-linux-gnu",
        _ => bail!("unsupported target combo: {arch_part}-{os_part}"),
    })
}

fn detect_target() -> Result<String> {
    let os = std::env::consts::OS;
    let arch = std::env::consts::ARCH;
    Ok(target_triple(os, arch)?.to_string())
}

fn archive_name(target: &str) -> String {
    format!("{BIN_NAME}-{target}.tar.gz")
}

fn select_prerelease_tag(releases: &[Release]) -> Option<String> {
    releases
        .iter()
        .find(|release| release.version.contains("-rc."))
        .map(|release| format!("v{}", release.version))
}

fn find_prerelease_tag(config: &UpdateConfig, target: Option<&str>) -> Result<String> {
    let mut builder = ReleaseList::configure();
    builder
        .repo_owner(config.repo_owner)
        .repo_name(config.repo_name);
    if let Some(target) = target {
        builder.with_target(target);
    }
    let releases = builder.build()?.fetch()?;
    select_prerelease_tag(&releases).ok_or_else(|| anyhow::anyhow!("no rc prerelease tags found"))
}

pub fn run(prerelease: bool) -> Result<()> {
    let config = update_config(prerelease);
    let current_version = self_update::cargo_crate_version!();
    let target = detect_target().ok();
    let mut builder = self_update::backends::github::Update::configure();
    builder
        .repo_owner(config.repo_owner)
        .repo_name(config.repo_name)
        .bin_name(config.bin_name)
        .show_download_progress(true)
        .current_version(current_version);

    if let Some(target) = target.as_deref() {
        log::info!(
            "self-update target={} expected_asset={}",
            target,
            archive_name(target)
        );
    }

    if config.prerelease {
        let tag =
            find_prerelease_tag(&config, target.as_deref()).context("find rc prerelease tag")?;
        builder.target_version_tag(&tag);
    }

    let status = builder
        .build()
        .context("configure self-update")?
        .update()
        .context("perform self-update")?;

    if status.updated() {
        log::info!("updated to {}", status.version());
    } else {
        log::info!("already up-to-date");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn release(version: &str) -> Release {
        Release {
            name: version.to_string(),
            version: version.to_string(),
            date: "2025-01-01T00:00:00Z".to_string(),
            body: None,
            assets: Vec::new(),
        }
    }

    #[test]
    fn update_config_sets_repo_fields() {
        let config = update_config(false);
        assert_eq!(config.repo_owner, REPO_OWNER);
        assert_eq!(config.repo_name, REPO_NAME);
        assert_eq!(config.bin_name, BIN_NAME);
        assert!(!config.prerelease);
    }

    #[test]
    fn update_config_sets_prerelease_flag() {
        let config = update_config(true);
        assert!(config.prerelease);
    }

    #[test]
    fn target_triple_maps_macos_x86_64() {
        let target = target_triple("macos", "x86_64").expect("target");
        assert_eq!(target, "x86_64-apple-darwin");
    }

    #[test]
    fn target_triple_maps_linux_aarch64() {
        let target = target_triple("linux", "aarch64").expect("target");
        assert_eq!(target, "aarch64-unknown-linux-gnu");
    }

    #[test]
    fn target_triple_rejects_unknown_os() {
        let err = target_triple("windows", "x86_64").expect_err("expected error");
        assert!(err.to_string().contains("unsupported os"));
    }

    #[test]
    fn target_triple_rejects_unknown_arch() {
        let err = target_triple("linux", "mips").expect_err("expected error");
        assert!(err.to_string().contains("unsupported arch"));
    }

    #[test]
    fn archive_name_uses_bin_and_target() {
        let name = archive_name("x86_64-unknown-linux-gnu");
        assert_eq!(name, "jkl-x86_64-unknown-linux-gnu.tar.gz");
    }

    #[test]
    fn select_prerelease_tag_picks_rc_release() {
        let releases = vec![release("0.1.0"), release("0.2.0-rc.1")];
        let tag = select_prerelease_tag(&releases);
        assert_eq!(tag.as_deref(), Some("v0.2.0-rc.1"));
    }

    #[test]
    fn select_prerelease_tag_returns_none_when_missing() {
        let releases = vec![release("0.1.0"), release("0.2.0")];
        let tag = select_prerelease_tag(&releases);
        assert!(tag.is_none());
    }
}
