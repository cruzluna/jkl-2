use std::collections::HashMap;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};
use std::time::{SystemTime, UNIX_EPOCH};

static ENV_LOCK: Mutex<()> = Mutex::new(());

pub(crate) struct EnvGuard {
    _lock: MutexGuard<'static, ()>,
    temp_dir: PathBuf,
    old_vars: HashMap<String, Option<OsString>>,
}

impl EnvGuard {
    pub(crate) fn new(prefix: &str) -> Self {
        let lock = ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let temp_dir = unique_temp_dir(prefix);
        fs::create_dir_all(&temp_dir).expect("create temp dir");
        Self {
            _lock: lock,
            temp_dir,
            old_vars: HashMap::new(),
        }
    }

    pub(crate) fn temp_dir(&self) -> &Path {
        &self.temp_dir
    }

    pub(crate) fn set_var(&mut self, key: &str, value: impl AsRef<OsStr>) {
        self.track_var(key);
        unsafe {
            std::env::set_var(key, value);
        }
    }

    pub(crate) fn remove_var(&mut self, key: &str) {
        self.track_var(key);
        unsafe {
            std::env::remove_var(key);
        }
    }

    pub(crate) fn set_temp_home(&mut self) -> PathBuf {
        let home = self.temp_dir.join("home");
        fs::create_dir_all(&home).expect("create temp home");
        self.set_var("HOME", &home);
        home
    }

    fn track_var(&mut self, key: &str) {
        if !self.old_vars.contains_key(key) {
            self.old_vars.insert(key.to_string(), std::env::var_os(key));
        }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        for (key, value) in self.old_vars.drain() {
            match value {
                Some(val) => unsafe {
                    std::env::set_var(&key, val);
                },
                None => unsafe {
                    std::env::remove_var(&key);
                },
            }
        }
        let _ = fs::remove_dir_all(&self.temp_dir);
    }
}

fn unique_temp_dir(prefix: &str) -> PathBuf {
    let mut dir = std::env::temp_dir();
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_else(|_| std::time::Duration::from_nanos(0))
        .as_nanos();
    dir.push(format!("{}-{}", prefix, nanos));
    dir
}
