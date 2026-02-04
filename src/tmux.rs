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
    pub pane_id: String,
}

pub fn list_sessions() -> Result<Vec<TmuxSession>, io::Error> {
    let output = Command::new("tmux")
        .args(["list-sessions", "-F", "#{session_id}\t#{session_name}"])
        .output()?;
    if !output.status.success() {
        let message = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(io::Error::new(io::ErrorKind::Other, message));
    }
    let raw_output = String::from_utf8_lossy(&output.stdout);
    debug!("tmux list-sessions raw output:\n{}", raw_output);

    let sessions: Vec<TmuxSession> = raw_output
        .lines()
        .filter_map(|line| {
            let mut parts = line.splitn(2, '\t');
            let id = parts.next()?.trim();
            let name = parts.next()?.trim();
            if id.is_empty() || name.is_empty() {
                debug!("skipping invalid session line: {}", line);
                None
            } else {
                let session = TmuxSession {
                    id: id.to_string(),
                    name: name.to_string(),
                };
                debug!("parsed session: {:?}", session);
                Some(session)
            }
        })
        .collect();
    info!("tmux list-sessions returned {} entries", sessions.len());
    Ok(sessions)
}

pub fn list_panes() -> Result<Vec<TmuxPane>, io::Error> {
    let output = Command::new("tmux")
        .args(["list-panes", "-a", "-F", "#{session_name}\t#{pane_id}"])
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
            let mut parts = line.splitn(2, '\t');
            let session_name = parts.next()?.trim();
            let pane_id = parts.next()?.trim();
            if session_name.is_empty() || pane_id.is_empty() {
                debug!("skipping invalid pane line: {}", line);
                None
            } else {
                let pane = TmuxPane {
                    session_name: session_name.to_string(),
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
