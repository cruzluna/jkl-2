use std::io;
use std::process::Command;

use log::{debug, info};

#[derive(Clone, Debug)]
pub struct TmuxSession {
    pub id: String,
    pub name: String,
}

#[derive(Clone, Debug)]
pub struct TmuxPane {
    pub session_name: String,
    pub window_id: String,
    pub window_name: String,
    pub pane_id: String,
}

fn parse_tmux_timestamp(value: Option<&str>) -> u64 {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(0)
}

pub fn list_sessions() -> Result<Vec<TmuxSession>, io::Error> {
    let output = Command::new("tmux")
        .args([
            "list-sessions",
            "-F",
            "#{session_id}\t#{session_name}\t#{session_last_attached}\t#{session_activity}",
        ])
        .output()?;
    if !output.status.success() {
        let message = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(io::Error::new(io::ErrorKind::Other, message));
    }
    let raw_output = String::from_utf8_lossy(&output.stdout);
    debug!("tmux list-sessions raw output:\n{}", raw_output);

    let mut sessions_with_recency: Vec<(TmuxSession, u64)> = raw_output
        .lines()
        .filter_map(|line| {
            let mut parts = line.splitn(4, '\t');
            let id = parts.next()?.trim();
            let name = parts.next()?.trim();
            let last_attached = parse_tmux_timestamp(parts.next());
            let last_activity = parse_tmux_timestamp(parts.next());
            if id.is_empty() || name.is_empty() {
                debug!("skipping invalid session line: {}", line);
                None
            } else {
                let recency = last_attached.max(last_activity);
                let session = TmuxSession {
                    id: id.to_string(),
                    name: name.to_string(),
                };
                debug!(
                    "parsed session: {:?}, last_attached={}, last_activity={}, recency={}",
                    session, last_attached, last_activity, recency
                );
                Some((session, recency))
            }
        })
        .collect();
    sessions_with_recency.sort_by(|(left, left_recency), (right, right_recency)| {
        right_recency
            .cmp(left_recency)
            .then_with(|| left.name.cmp(&right.name))
            .then_with(|| left.id.cmp(&right.id))
    });

    let sessions: Vec<TmuxSession> = sessions_with_recency
        .into_iter()
        .map(|(session, _)| session)
        .collect();
    info!("tmux list-sessions returned {} entries", sessions.len());
    Ok(sessions)
}

pub fn list_panes() -> Result<Vec<TmuxPane>, io::Error> {
    let output = Command::new("tmux")
        .args([
            "list-panes",
            "-a",
            "-F",
            "#{session_name}\t#{window_id}\t#{window_name}\t#{pane_id}",
        ])
        .output()?;
    if !output.status.success() {
        let message = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(io::Error::new(io::ErrorKind::Other, message));
    }
    let raw_output = String::from_utf8_lossy(&output.stdout);
    debug!("tmux list-panes raw output:\n{}", raw_output);

    let panes: Vec<TmuxPane> = raw_output
        .lines()
        .filter_map(|line| {
            let mut parts = line.splitn(4, '\t');
            let session_name = parts.next()?.trim();
            let window_id = parts.next()?.trim();
            let window_name = parts.next()?.trim();
            let pane_id = parts.next()?.trim();
            if session_name.is_empty()
                || window_id.is_empty()
                || window_name.is_empty()
                || pane_id.is_empty()
            {
                debug!("skipping invalid pane line: {}", line);
                None
            } else {
                let pane = TmuxPane {
                    session_name: session_name.to_string(),
                    window_id: window_id.to_string(),
                    window_name: window_name.to_string(),
                    pane_id: pane_id.to_string(),
                };
                debug!("parsed pane: {:?}", pane);
                Some(pane)
            }
        })
        .collect();
    info!("tmux list-panes returned {} entries", panes.len());
    Ok(panes)
}

pub fn switch_client(target: &str) -> Result<(), io::Error> {
    let output = Command::new("tmux")
        .args(["switch-client", "-t", target])
        .output()?;
    if !output.status.success() {
        let message = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(io::Error::new(io::ErrorKind::Other, message));
    }
    debug!("tmux switch-client target={}", target);
    Ok(())
}

pub fn select_window(target: &str) -> Result<(), io::Error> {
    let output = Command::new("tmux")
        .args(["select-window", "-t", target])
        .output()?;
    if !output.status.success() {
        let message = String::from_utf8_lossy(&output.stderr).trim().to_string();
        debug!(
            "tmux select-window failed target={} stderr={}",
            target, message
        );
        return Err(io::Error::new(io::ErrorKind::Other, message));
    }
    debug!("tmux select-window target={}", target);
    Ok(())
}

pub fn select_pane(target: &str) -> Result<(), io::Error> {
    let output = Command::new("tmux")
        .args(["select-pane", "-t", target])
        .output()?;
    if !output.status.success() {
        let message = String::from_utf8_lossy(&output.stderr).trim().to_string();
        debug!(
            "tmux select-pane failed target={} stderr={}",
            target, message
        );
        return Err(io::Error::new(io::ErrorKind::Other, message));
    }
    debug!("tmux select-pane target={}", target);
    Ok(())
}

pub fn kill_session(target: &str) -> Result<(), io::Error> {
    let output = Command::new("tmux")
        .args(["kill-session", "-t", target])
        .output()?;
    if !output.status.success() {
        let message = String::from_utf8_lossy(&output.stderr).trim().to_string();
        debug!(
            "tmux kill-session failed target={} stderr={}",
            target, message
        );
        return Err(io::Error::new(io::ErrorKind::Other, message));
    }
    debug!("tmux kill-session target={}", target);
    Ok(())
}

pub fn kill_window(target: &str) -> Result<(), io::Error> {
    let output = Command::new("tmux")
        .args(["kill-window", "-t", target])
        .output()?;
    if !output.status.success() {
        let message = String::from_utf8_lossy(&output.stderr).trim().to_string();
        debug!(
            "tmux kill-window failed target={} stderr={}",
            target, message
        );
        return Err(io::Error::new(io::ErrorKind::Other, message));
    }
    debug!("tmux kill-window target={}", target);
    Ok(())
}

pub fn kill_pane(target: &str) -> Result<(), io::Error> {
    let output = Command::new("tmux")
        .args(["kill-pane", "-t", target])
        .output()?;
    if !output.status.success() {
        let message = String::from_utf8_lossy(&output.stderr).trim().to_string();
        debug!("tmux kill-pane failed target={} stderr={}", target, message);
        return Err(io::Error::new(io::ErrorKind::Other, message));
    }
    debug!("tmux kill-pane target={}", target);
    Ok(())
}

#[cfg(test)]
#[cfg(unix)]
mod tests {
    use super::*;
    use crate::test_utils::EnvGuard;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;

    fn setup_fake_tmux(env: &mut EnvGuard) -> PathBuf {
        let bin_dir = env.temp_dir().join("bin");
        fs::create_dir_all(&bin_dir).expect("create bin dir");
        let script_path = bin_dir.join("tmux");
        let script = r#"#!/bin/sh
case "$1" in
  list-sessions)
    if [ "${TMUX_LIST_SESSIONS_EXIT:-0}" -ne 0 ]; then
      echo "${TMUX_LIST_SESSIONS_ERR:-error}" 1>&2
      exit "${TMUX_LIST_SESSIONS_EXIT}"
    fi
    printf "%s" "${TMUX_LIST_SESSIONS:-}"
    exit 0
    ;;
  list-panes)
    if [ "${TMUX_LIST_PANES_EXIT:-0}" -ne 0 ]; then
      echo "${TMUX_LIST_PANES_ERR:-error}" 1>&2
      exit "${TMUX_LIST_PANES_EXIT}"
    fi
    printf "%s" "${TMUX_LIST_PANES:-}"
    exit 0
    ;;
  switch-client)
    if [ "${TMUX_SWITCH_FAIL:-0}" -ne 0 ]; then
      echo "${TMUX_SWITCH_ERR:-error}" 1>&2
      exit 1
    fi
    exit 0
    ;;
  select-window)
    if [ "${TMUX_SELECT_WINDOW_FAIL:-0}" -ne 0 ]; then
      echo "${TMUX_SELECT_WINDOW_ERR:-error}" 1>&2
      exit 1
    fi
    exit 0
    ;;
  select-pane)
    if [ "${TMUX_SELECT_PANE_FAIL:-0}" -ne 0 ]; then
      echo "${TMUX_SELECT_PANE_ERR:-error}" 1>&2
      exit 1
    fi
    exit 0
    ;;
  *)
    echo "unsupported" 1>&2
    exit 1
    ;;
esac
"#;
        fs::write(&script_path, script).expect("write tmux script");
        let mut perms = fs::metadata(&script_path).expect("metadata").permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&script_path, perms).expect("chmod");

        let old_path = std::env::var("PATH").unwrap_or_default();
        let new_path = format!("{}:{}", bin_dir.display(), old_path);
        env.set_var("PATH", new_path);
        script_path
    }

    #[test]
    fn list_sessions_parses_output() {
        let mut env = EnvGuard::new("tmux-list-sessions");
        setup_fake_tmux(&mut env);
        env.set_var("TMUX_LIST_SESSIONS", "@1\tone\n@2\ttwo\n");
        env.remove_var("TMUX_LIST_SESSIONS_EXIT");

        let sessions = list_sessions().expect("list sessions");
        assert_eq!(sessions.len(), 2);
        assert_eq!(sessions[0].id, "@1");
        assert_eq!(sessions[0].name, "one");
        assert_eq!(sessions[1].id, "@2");
        assert_eq!(sessions[1].name, "two");
    }

    #[test]
    fn list_sessions_sorts_by_recent_usage_descending() {
        let mut env = EnvGuard::new("tmux-list-sessions-recent");
        setup_fake_tmux(&mut env);
        env.set_var(
            "TMUX_LIST_SESSIONS",
            "@1\tone\t100\t100\n@2\ttwo\t200\t10\n@3\tthree\t0\t150\n@4\tfour\t\t\n",
        );
        env.remove_var("TMUX_LIST_SESSIONS_EXIT");

        let sessions = list_sessions().expect("list sessions");
        assert_eq!(sessions.len(), 4);
        assert_eq!(sessions[0].id, "@2");
        assert_eq!(sessions[1].id, "@3");
        assert_eq!(sessions[2].id, "@1");
        assert_eq!(sessions[3].id, "@4");
    }

    #[test]
    fn list_sessions_skips_invalid_lines() {
        let mut env = EnvGuard::new("tmux-list-sessions-invalid");
        setup_fake_tmux(&mut env);
        env.set_var(
            "TMUX_LIST_SESSIONS",
            " \n@1\tone\n\tmissing_id\n@2\t\n@3\tthree\n",
        );
        env.remove_var("TMUX_LIST_SESSIONS_EXIT");

        let sessions = list_sessions().expect("list sessions");
        assert_eq!(sessions.len(), 2);
        assert_eq!(sessions[0].id, "@1");
        assert_eq!(sessions[1].id, "@3");
    }

    #[test]
    fn list_sessions_returns_error_on_failure() {
        let mut env = EnvGuard::new("tmux-list-sessions-error");
        setup_fake_tmux(&mut env);
        env.set_var("TMUX_LIST_SESSIONS_EXIT", "1");
        env.set_var("TMUX_LIST_SESSIONS_ERR", "boom");

        let err = list_sessions().expect_err("expected error");
        assert_eq!(err.kind(), io::ErrorKind::Other);
        assert_eq!(err.to_string(), "boom");
    }

    #[test]
    fn list_panes_parses_output() {
        let mut env = EnvGuard::new("tmux-list-panes");
        setup_fake_tmux(&mut env);
        env.set_var(
            "TMUX_LIST_PANES",
            "alpha\t@10\teditor\t%1\nbeta\t@20\tserver\t%2\n",
        );
        env.remove_var("TMUX_LIST_PANES_EXIT");

        let panes = list_panes().expect("list panes");
        assert_eq!(panes.len(), 2);
        assert_eq!(panes[0].session_name, "alpha");
        assert_eq!(panes[0].window_id, "@10");
        assert_eq!(panes[0].window_name, "editor");
        assert_eq!(panes[0].pane_id, "%1");
        assert_eq!(panes[1].session_name, "beta");
        assert_eq!(panes[1].window_id, "@20");
        assert_eq!(panes[1].window_name, "server");
        assert_eq!(panes[1].pane_id, "%2");
    }

    #[test]
    fn list_panes_returns_error_on_failure() {
        let mut env = EnvGuard::new("tmux-list-panes-error");
        setup_fake_tmux(&mut env);
        env.set_var("TMUX_LIST_PANES_EXIT", "1");
        env.set_var("TMUX_LIST_PANES_ERR", "no panes");

        let err = list_panes().expect_err("expected error");
        assert_eq!(err.kind(), io::ErrorKind::Other);
        assert_eq!(err.to_string(), "no panes");
    }

    #[test]
    fn switch_client_success() {
        let mut env = EnvGuard::new("tmux-switch-ok");
        setup_fake_tmux(&mut env);
        env.remove_var("TMUX_SWITCH_FAIL");

        switch_client("@1").expect("switch client");
    }

    #[test]
    fn switch_client_returns_error_on_failure() {
        let mut env = EnvGuard::new("tmux-switch-error");
        setup_fake_tmux(&mut env);
        env.set_var("TMUX_SWITCH_FAIL", "1");
        env.set_var("TMUX_SWITCH_ERR", "no client");

        let err = switch_client("@1").expect_err("expected error");
        assert_eq!(err.kind(), io::ErrorKind::Other);
        assert_eq!(err.to_string(), "no client");
    }

    #[test]
    fn select_window_success() {
        let mut env = EnvGuard::new("tmux-select-window-ok");
        setup_fake_tmux(&mut env);
        env.remove_var("TMUX_SELECT_WINDOW_FAIL");

        select_window("@10").expect("select window");
    }

    #[test]
    fn select_window_returns_error_on_failure() {
        let mut env = EnvGuard::new("tmux-select-window-error");
        setup_fake_tmux(&mut env);
        env.set_var("TMUX_SELECT_WINDOW_FAIL", "1");
        env.set_var("TMUX_SELECT_WINDOW_ERR", "no window");

        let err = select_window("@10").expect_err("expected error");
        assert_eq!(err.kind(), io::ErrorKind::Other);
        assert_eq!(err.to_string(), "no window");
    }
}
