use std::path::PathBuf;

pub fn config_dir() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(PathBuf::from))
            .map(|dir| dir.join("jkl"))
    }

    #[cfg(not(windows))]
    {
        if let Some(dir) = std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from) {
            return Some(dir.join("jkl"));
        }
        std::env::var_os("HOME")
            .map(PathBuf::from)
            .map(|dir| dir.join(".config").join("jkl"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::EnvGuard;

    #[test]
    #[cfg(unix)]
    fn config_dir_respects_xdg_config_home() {
        let mut env = EnvGuard::new("paths-xdg-config");
        let base = env.temp_dir().join("xdg-config");
        std::fs::create_dir_all(&base).expect("create xdg config dir");
        env.set_var("XDG_CONFIG_HOME", &base);

        let dir = config_dir().expect("config dir");
        assert_eq!(dir, base.join("jkl"));
    }

    #[test]
    fn config_dir_defaults_to_home_config() {
        let mut env = EnvGuard::new("paths-home-config");
        let home = env.set_temp_home();
        env.remove_var("XDG_CONFIG_HOME");

        let dir = config_dir().expect("config dir");
        let expected = if cfg!(windows) {
            home.join("AppData").join("Roaming").join("jkl")
        } else {
            home.join(".config").join("jkl")
        };
        assert_eq!(dir, expected);
    }
}
