use log::{debug, info};
use serde::Deserialize;
use std::fs;
use std::path::PathBuf;
use thiserror::Error;

#[derive(Clone, Debug, Deserialize, Default)]
#[serde(default)]
pub struct Config {
    pub tui: TuiConfig,
}

#[derive(Clone, Debug, Deserialize, Default)]
#[serde(default)]
pub struct TuiConfig {
    pub pane_jump_expanded: bool,
}

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Environment variable not found: {0}")]
    EnvVar(#[from] std::env::VarError),
    #[error("TOML parse error: {0}")]
    Parse(#[from] toml::de::Error),
}

pub fn load() -> Result<Config, ConfigError> {
    let path = config_path()?;
    debug!("loading config path={}", path.display());

    let contents = match fs::read_to_string(&path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            info!(
                "config file missing at {}; using default feature flags",
                path.display()
            );
            return Ok(Config::default());
        }
        Err(error) => return Err(ConfigError::Io(error)),
    };

    if contents.trim().is_empty() {
        return Ok(Config::default());
    }

    let config = toml::from_str::<Config>(&contents)?;
    Ok(config)
}

fn config_path() -> Result<PathBuf, ConfigError> {
    let home = std::env::var("HOME")?;
    Ok(PathBuf::from(home)
        .join(".config")
        .join("jkl")
        .join("jkl.toml"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::EnvGuard;

    #[test]
    fn load_uses_defaults_when_file_is_missing() {
        let mut env = EnvGuard::new("config-create");
        env.set_temp_home();
        let path = config_path().expect("config path");
        assert!(!path.exists());

        let loaded = load().expect("load config");

        assert!(!loaded.tui.pane_jump_expanded);
        assert!(!path.exists());
    }

    #[test]
    fn load_reads_pane_jump_flag_from_file() {
        let mut env = EnvGuard::new("config-read");
        env.set_temp_home();
        let path = config_path().expect("config path");
        fs::create_dir_all(path.parent().expect("config parent")).expect("create config parent");
        fs::write(
            &path,
            "[tui]\n\
             pane_jump_expanded = true\n",
        )
        .expect("write config");

        let loaded = load().expect("load config");

        assert!(loaded.tui.pane_jump_expanded);
    }
}
