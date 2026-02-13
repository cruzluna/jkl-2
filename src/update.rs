use anyhow::{Context, Result, bail};
use self_update::backends::github::ReleaseList;
use self_update::update::Release;
use std::path::PathBuf;

const REPO_OWNER: &str = "cruzluna";
const REPO_NAME: &str = "jkl-2";
const BIN_NAME: &str = "jkl";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateChannel {
    Stable,
    PreRelease,
    DevPreview,
}

impl UpdateChannel {
    fn tag_fragment(self) -> Option<&'static str> {
        match self {
            Self::Stable => None,
            Self::PreRelease => Some("-rc."),
            Self::DevPreview => Some("-dev."),
        }
    }

    fn not_found_error(self) -> &'static str {
        match self {
            Self::Stable => "stable channel does not use tag filtering",
            Self::PreRelease => "no rc prerelease tags found",
            Self::DevPreview => "no dev preview tags found",
        }
    }

    fn context_label(self) -> &'static str {
        match self {
            Self::Stable => "stable",
            Self::PreRelease => "rc prerelease",
            Self::DevPreview => "dev preview",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct UpdateConfig {
    repo_owner: &'static str,
    repo_name: &'static str,
    bin_name: &'static str,
}

fn update_config() -> UpdateConfig {
    UpdateConfig {
        repo_owner: REPO_OWNER,
        repo_name: REPO_NAME,
        bin_name: BIN_NAME,
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

fn select_channel_tag(releases: &[Release], channel: UpdateChannel) -> Option<String> {
    let fragment = channel.tag_fragment()?;
    releases
        .iter()
        .find(|release| release.version.contains(fragment))
        .map(|release| format!("v{}", release.version))
}

fn find_channel_tag(
    config: &UpdateConfig,
    target: Option<&str>,
    channel: UpdateChannel,
) -> Result<String> {
    if channel == UpdateChannel::Stable {
        bail!("{}", channel.not_found_error());
    }

    let mut builder = ReleaseList::configure();
    builder
        .repo_owner(config.repo_owner)
        .repo_name(config.repo_name);
    if let Some(target) = target {
        builder.with_target(target);
    }
    let releases = builder.build()?.fetch()?;
    select_channel_tag(&releases, channel).ok_or_else(|| anyhow::anyhow!(channel.not_found_error()))
}

fn command_in_path(command: &str) -> bool {
    let Some(path) = std::env::var_os("PATH") else {
        return false;
    };
    std::env::split_paths(&path).any(|dir| {
        let candidate: PathBuf = dir.join(command);
        candidate.is_file()
    })
}

pub fn run(channel: UpdateChannel) -> Result<()> {
    let config = update_config();
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

    if channel != UpdateChannel::Stable {
        let tag = find_channel_tag(&config, target.as_deref(), channel)
            .with_context(|| format!("find {} tag", channel.context_label()))?;
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

    if command_in_path("jkl-sync-fig-autocomplete") {
        println!("To refresh Fig autocomplete, run: jkl-sync-fig-autocomplete");
    } else {
        println!(
            "To refresh Fig autocomplete, run:\n  curl -fsSL https://raw.githubusercontent.com/{REPO_OWNER}/{REPO_NAME}/master/scripts/sync-fig-autocomplete.sh | bash"
        );
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
        let config = update_config();
        assert_eq!(config.repo_owner, REPO_OWNER);
        assert_eq!(config.repo_name, REPO_NAME);
        assert_eq!(config.bin_name, BIN_NAME);
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
    fn select_channel_tag_picks_rc_release() {
        let releases = vec![release("0.1.0"), release("0.2.0-rc.1")];
        let tag = select_channel_tag(&releases, UpdateChannel::PreRelease);
        assert_eq!(tag.as_deref(), Some("v0.2.0-rc.1"));
    }

    #[test]
    fn select_channel_tag_picks_dev_release() {
        let releases = vec![release("0.1.0-rc.1"), release("0.2.0-dev.12.abcd123")];
        let tag = select_channel_tag(&releases, UpdateChannel::DevPreview);
        assert_eq!(tag.as_deref(), Some("v0.2.0-dev.12.abcd123"));
    }

    #[test]
    fn select_channel_tag_returns_none_when_missing() {
        let releases = vec![release("0.1.0"), release("0.2.0")];
        let tag = select_channel_tag(&releases, UpdateChannel::PreRelease);
        assert!(tag.is_none());
    }

    #[test]
    fn select_channel_tag_returns_none_for_stable() {
        let releases = vec![release("0.1.0"), release("0.2.0-rc.1")];
        let tag = select_channel_tag(&releases, UpdateChannel::Stable);
        assert!(tag.is_none());
    }
}
