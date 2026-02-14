use anyhow::{Context, Result, bail};
use self_update::backends::github::ReleaseList;
use self_update::update::{Release, ReleaseAsset};
use std::fs;

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

fn detect_target() -> String {
    self_update::get_target().to_string()
}

fn archive_name(target: &str, identifier: Option<&str>) -> String {
    match identifier {
        Some(identifier) => format!("{BIN_NAME}-{target}-{identifier}.tar.gz"),
        None => format!("{BIN_NAME}-{target}.tar.gz"),
    }
}

fn parse_os_release_value(contents: &str, key: &str) -> Option<String> {
    for line in contents.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((line_key, raw_value)) = line.split_once('=') else {
            continue;
        };
        if line_key != key {
            continue;
        }
        return Some(raw_value.trim().trim_matches('"').to_string());
    }
    None
}

fn is_amazon_linux_2_from_os_release(contents: &str) -> bool {
    let id = parse_os_release_value(contents, "ID");
    let version_id = parse_os_release_value(contents, "VERSION_ID");
    matches!(id.as_deref(), Some("amzn"))
        && version_id
            .as_deref()
            .is_some_and(|version| version == "2" || version.starts_with("2."))
}

fn is_amazon_linux_2() -> bool {
    if let Ok(os_release) = fs::read_to_string("/etc/os-release") {
        if is_amazon_linux_2_from_os_release(&os_release) {
            return true;
        }
    }

    if let Ok(system_release) = fs::read_to_string("/etc/system-release") {
        let lower = system_release.to_ascii_lowercase();
        return lower.contains("amazon linux") && lower.contains("release 2");
    }

    false
}

fn detect_asset_identifier_for(target: &str, amazon_linux_2: bool) -> Option<&'static str> {
    if amazon_linux_2 && target.ends_with("unknown-linux-gnu") {
        return Some("al2");
    }
    None
}

fn detect_asset_identifier(target: &str) -> Option<&'static str> {
    detect_asset_identifier_for(target, is_amazon_linux_2())
}

fn select_channel_tag(releases: &[Release], channel: UpdateChannel) -> Option<String> {
    let fragment = channel.tag_fragment()?;
    releases
        .iter()
        .find(|release| release.version.contains(fragment))
        .map(|release| format!("v{}", release.version))
}

fn filter_releases_for_identifier(
    releases: Vec<Release>,
    target: Option<&str>,
    identifier: Option<&str>,
) -> Vec<Release> {
    if let (Some(target), Some(identifier)) = (target, identifier) {
        return releases
            .into_iter()
            .filter(|release| release.asset_for(target, Some(identifier)).is_some())
            .collect();
    }
    releases
}

fn find_channel_tag(
    config: &UpdateConfig,
    target: Option<&str>,
    identifier: Option<&str>,
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
    let releases = filter_releases_for_identifier(builder.build()?.fetch()?, target, identifier);
    select_channel_tag(&releases, channel).ok_or_else(|| anyhow::anyhow!(channel.not_found_error()))
}

pub fn run(channel: UpdateChannel) -> Result<()> {
    let config = update_config();
    let current_version = self_update::cargo_crate_version!();
    let target = detect_target();
    let asset_identifier = detect_asset_identifier(&target);
    let mut builder = self_update::backends::github::Update::configure();
    builder
        .repo_owner(config.repo_owner)
        .repo_name(config.repo_name)
        .bin_name(config.bin_name)
        .show_download_progress(true)
        .current_version(current_version)
        .target(&target);
    if let Some(identifier) = asset_identifier {
        builder.identifier(identifier);
    }

    if let Some(identifier) = asset_identifier {
        log::info!(
            "self-update target={} identifier={} expected_asset={}",
            target,
            identifier,
            archive_name(&target, Some(identifier))
        );
    } else {
        log::info!(
            "self-update target={} expected_asset={}",
            target,
            archive_name(&target, None)
        );
    }

    if channel != UpdateChannel::Stable {
        let tag = find_channel_tag(&config, Some(target.as_str()), asset_identifier, channel)
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

    println!("To refresh Fig autocomplete, run: jkl init fig-autocomplete");

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

    fn release_with_assets(version: &str, asset_names: &[&str]) -> Release {
        Release {
            name: version.to_string(),
            version: version.to_string(),
            date: "2025-01-01T00:00:00Z".to_string(),
            body: None,
            assets: asset_names
                .iter()
                .map(|name| ReleaseAsset {
                    download_url: format!("https://example.com/{name}"),
                    name: (*name).to_string(),
                })
                .collect(),
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
    fn detect_target_matches_self_update_target() {
        assert_eq!(detect_target(), self_update::get_target());
    }

    #[test]
    fn archive_name_uses_bin_and_target() {
        let name = archive_name("x86_64-unknown-linux-gnu", None);
        assert_eq!(name, "jkl-x86_64-unknown-linux-gnu.tar.gz");
    }

    #[test]
    fn archive_name_includes_identifier() {
        let name = archive_name("x86_64-unknown-linux-gnu", Some("al2"));
        assert_eq!(name, "jkl-x86_64-unknown-linux-gnu-al2.tar.gz");
    }

    #[test]
    fn parse_os_release_value_reads_quoted_values() {
        let os_release = r#"
ID="amzn"
VERSION_ID="2.0.20240101"
"#;
        assert_eq!(
            parse_os_release_value(os_release, "ID").as_deref(),
            Some("amzn")
        );
        assert_eq!(
            parse_os_release_value(os_release, "VERSION_ID").as_deref(),
            Some("2.0.20240101")
        );
    }

    #[test]
    fn parse_os_release_value_returns_none_when_key_missing() {
        let os_release = "ID=ubuntu\nVERSION_ID=24.04\n";
        assert_eq!(parse_os_release_value(os_release, "NAME"), None);
    }

    #[test]
    fn is_amazon_linux_2_from_os_release_matches_amzn2() {
        let os_release = r#"
ID="amzn"
VERSION_ID="2"
"#;
        assert!(is_amazon_linux_2_from_os_release(os_release));
    }

    #[test]
    fn is_amazon_linux_2_from_os_release_matches_amzn2_patch() {
        let os_release = r#"
ID=amzn
VERSION_ID=2.0.20240101
"#;
        assert!(is_amazon_linux_2_from_os_release(os_release));
    }

    #[test]
    fn is_amazon_linux_2_from_os_release_rejects_other_versions() {
        let os_release = r#"
ID=amzn
VERSION_ID=2023
"#;
        assert!(!is_amazon_linux_2_from_os_release(os_release));
    }

    #[test]
    fn detect_asset_identifier_for_linux_gnu_on_amazon_linux_2() {
        let identifier = detect_asset_identifier_for("x86_64-unknown-linux-gnu", true);
        assert_eq!(identifier, Some("al2"));
    }

    #[test]
    fn detect_asset_identifier_for_non_amazon_linux_2_or_non_gnu() {
        assert_eq!(
            detect_asset_identifier_for("x86_64-unknown-linux-gnu", false),
            None
        );
        assert_eq!(
            detect_asset_identifier_for("x86_64-unknown-linux-musl", true),
            None
        );
    }

    #[test]
    fn filter_releases_for_identifier_keeps_only_matching_assets() {
        let releases = vec![
            release_with_assets("0.2.0-rc.1", &["jkl-x86_64-unknown-linux-gnu.tar.gz"]),
            release_with_assets("0.2.0-rc.2", &["jkl-x86_64-unknown-linux-gnu-al2.tar.gz"]),
        ];
        let filtered =
            filter_releases_for_identifier(releases, Some("x86_64-unknown-linux-gnu"), Some("al2"));
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].version, "0.2.0-rc.2");
    }

    #[test]
    fn filter_releases_for_identifier_is_noop_without_identifier() {
        let releases = vec![
            release_with_assets("0.2.0-rc.1", &["jkl-x86_64-unknown-linux-gnu.tar.gz"]),
            release_with_assets("0.2.0-rc.2", &["jkl-x86_64-unknown-linux-gnu-al2.tar.gz"]),
        ];
        let filtered =
            filter_releases_for_identifier(releases, Some("x86_64-unknown-linux-gnu"), None);
        assert_eq!(filtered.len(), 2);
    }

    #[test]
    fn select_channel_tag_with_identifier_filter_picks_matching_release() {
        let releases = vec![
            release_with_assets("0.2.0-rc.1", &["jkl-x86_64-unknown-linux-gnu.tar.gz"]),
            release_with_assets("0.2.0-rc.2", &["jkl-x86_64-unknown-linux-gnu-al2.tar.gz"]),
        ];
        let filtered =
            filter_releases_for_identifier(releases, Some("x86_64-unknown-linux-gnu"), Some("al2"));
        let tag = select_channel_tag(&filtered, UpdateChannel::PreRelease);
        assert_eq!(tag.as_deref(), Some("v0.2.0-rc.2"));
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
