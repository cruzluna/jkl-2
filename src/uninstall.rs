use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

const FIG_HELPER_NAME: &str = "jkl-sync-fig-autocomplete";

#[derive(Debug)]
struct UninstallSummary {
    binary_path: PathBuf,
    helper_path: PathBuf,
    removed_binary: bool,
    removed_helper: bool,
    removed_config: Option<bool>,
    config_path: Option<PathBuf>,
}

pub fn run(purge_data: bool) -> Result<()> {
    let exe_path = std::env::current_exe().context("resolve current executable path")?;
    let home_dir = if purge_data {
        Some(
            PathBuf::from(std::env::var("HOME").context("read HOME for config cleanup")?),
        )
    } else {
        None
    };

    let summary = uninstall_paths(&exe_path, home_dir.as_deref(), purge_data)?;

    if summary.removed_binary {
        println!("Removed {}", summary.binary_path.display());
    } else {
        println!("Binary already missing: {}", summary.binary_path.display());
    }

    if summary.removed_helper {
        println!("Removed {}", summary.helper_path.display());
    } else {
        println!("Helper not found: {}", summary.helper_path.display());
    }

    if let Some(removed) = summary.removed_config {
        let path = summary
            .config_path
            .as_ref()
            .expect("config_path should exist when removed_config is Some");
        if removed {
            println!("Removed {}", path.display());
        } else {
            println!("Config already missing: {}", path.display());
        }
    }

    Ok(())
}

fn uninstall_paths(
    exe_path: &Path,
    home_dir: Option<&Path>,
    purge_data: bool,
) -> Result<UninstallSummary> {
    let binary_path = exe_path.to_path_buf();
    let parent = exe_path
        .parent()
        .context("resolve executable parent directory")?;
    let helper_path = parent.join(FIG_HELPER_NAME);

    let removed_binary = remove_file_if_exists(&binary_path)
        .with_context(|| format!("remove binary at {}", binary_path.display()))?;
    let removed_helper = remove_file_if_exists(&helper_path)
        .with_context(|| format!("remove helper at {}", helper_path.display()))?;

    let (removed_config, config_path) = if purge_data {
        let home = home_dir.context("missing home directory for config cleanup")?;
        let path = home.join(".config").join("jkl");
        let removed = remove_dir_if_exists(&path)
            .with_context(|| format!("remove config directory at {}", path.display()))?;
        (Some(removed), Some(path))
    } else {
        (None, None)
    };

    Ok(UninstallSummary {
        binary_path,
        helper_path,
        removed_binary,
        removed_helper,
        removed_config,
        config_path,
    })
}

fn remove_file_if_exists(path: &Path) -> Result<bool, std::io::Error> {
    match fs::remove_file(path) {
        Ok(()) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

fn remove_dir_if_exists(path: &Path) -> Result<bool, std::io::Error> {
    match fs::remove_dir_all(path) {
        Ok(()) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::EnvGuard;

    #[test]
    fn uninstall_paths_removes_binary_and_helper() {
        let env = EnvGuard::new("uninstall-remove-files");
        let exe = env.temp_dir().join("jkl");
        let helper = env.temp_dir().join(FIG_HELPER_NAME);

        fs::write(&exe, "bin").expect("write exe");
        fs::write(&helper, "helper").expect("write helper");

        let summary = uninstall_paths(&exe, None, false).expect("uninstall");
        assert!(summary.removed_binary);
        assert!(summary.removed_helper);
        assert!(!exe.exists());
        assert!(!helper.exists());
    }

    #[test]
    fn uninstall_paths_handles_missing_files() {
        let env = EnvGuard::new("uninstall-missing-files");
        let exe = env.temp_dir().join("jkl");

        let summary = uninstall_paths(&exe, None, false).expect("uninstall");
        assert!(!summary.removed_binary);
        assert!(!summary.removed_helper);
    }

    #[test]
    fn uninstall_paths_purges_config_when_requested() {
        let env = EnvGuard::new("uninstall-purge-config");
        let exe = env.temp_dir().join("jkl");
        let home = env.temp_dir().join("home");
        let config_dir = home.join(".config").join("jkl");

        fs::create_dir_all(&config_dir).expect("create config dir");
        fs::write(config_dir.join("session_context.json"), "{}").expect("write config");
        fs::write(&exe, "bin").expect("write exe");

        let summary = uninstall_paths(&exe, Some(&home), true).expect("uninstall");
        assert_eq!(summary.removed_config, Some(true));
        assert!(!config_dir.exists());
    }
}
