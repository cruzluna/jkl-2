use log::{debug, info};
use serde::Deserialize;
use std::fs;
use std::path::PathBuf;
use thiserror::Error;

const DEFAULT_CONFIG: &str = r#"# jkl configuration

[features]
# Show the Context column in `jkl tui` when the terminal is wide enough.
tui_context = true
"#;

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct JklConfig {
    pub features: FeatureConfig,
}

impl Default for JklConfig {
    fn default() -> Self {
        Self {
            features: FeatureConfig::default(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct FeatureConfig {
    pub tui_context: bool,
}

impl Default for FeatureConfig {
    fn default() -> Self {
        Self { tui_context: true }
    }
}

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("TOML error: {0}")]
    Toml(#[from] toml::de::Error),
    #[error("Environment variable not found: {0}")]
    EnvVar(#[from] std::env::VarError),
}

pub fn load_config() -> Result<JklConfig, ConfigError> {
    let path = config_path()?;
    debug!("loading config path={}", path.display());
    let contents = match fs::read_to_string(&path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            info!(
                "config file missing, creating default at {}",
                path.display()
            );
            fs::write(&path, DEFAULT_CONFIG)?;
            return Ok(JklConfig::default());
        }
        Err(error) => return Err(ConfigError::Io(error)),
    };

    let config = toml::from_str(&contents)?;
    debug!("loaded config: {:?}", config);
    Ok(config)
}

fn config_path() -> Result<PathBuf, ConfigError> {
    let home = std::env::var("HOME")?;
    let path = PathBuf::from(home)
        .join(".config")
        .join("jkl")
        .join("jkl.toml");
    debug!("resolved config path={}", path.display());
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::EnvGuard;

    #[test]
    fn load_config_creates_missing_file_with_defaults() {
        let mut env = EnvGuard::new("config-load");
        env.set_temp_home();
        let path = config_path().expect("config path");
        assert!(!path.exists());

        let config = load_config().expect("load config");

        assert!(config.features.tui_context);
        assert!(path.exists());
        let contents = fs::read_to_string(path).expect("read config");
        assert!(contents.contains("tui_context = true"));
    }

    #[test]
    fn load_config_reads_tui_context_feature_flag() {
        let mut env = EnvGuard::new("config-tui-context");
        let home = env.set_temp_home();
        let config_dir = home.join(".config").join("jkl");
        fs::create_dir_all(&config_dir).expect("create config dir");
        fs::write(
            config_dir.join("jkl.toml"),
            "[features]\ntui_context = false\n",
        )
        .expect("write config");

        let config = load_config().expect("load config");

        assert!(!config.features.tui_context);
    }

    #[test]
    fn load_config_defaults_missing_feature_values() {
        let mut env = EnvGuard::new("config-defaults");
        let home = env.set_temp_home();
        let config_dir = home.join(".config").join("jkl");
        fs::create_dir_all(&config_dir).expect("create config dir");
        fs::write(config_dir.join("jkl.toml"), "").expect("write config");

        let config = load_config().expect("load config");

        assert!(config.features.tui_context);
    }
}
