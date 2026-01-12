use std::io;
use std::process::Command;

#[derive(Clone, Debug)]
pub struct TmuxSession {
    pub id: String,
    pub name: String,
}

pub fn list_sessions() -> Result<Vec<TmuxSession>, io::Error> {
    let output = Command::new("tmux")
        .args(["list-sessions", "-F", "#{session_id}\t#{session_name}"])
        .output()?;
    if !output.status.success() {
        let message = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(io::Error::new(io::ErrorKind::Other, message));
    }
    let sessions = String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| {
            let mut parts = line.splitn(2, '\t');
            let id = parts.next()?.trim();
            let name = parts.next()?.trim();
            if id.is_empty() || name.is_empty() {
                None
            } else {
                Some(TmuxSession {
                    id: id.to_string(),
                    name: name.to_string(),
                })
            }
        })
        .collect();
    Ok(sessions)
}

pub fn switch_client(target: &str) -> Result<(), io::Error> {
    let output = Command::new("tmux")
        .args(["switch-client", "-t", target])
        .output()?;
    if !output.status.success() {
        let message = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(io::Error::new(io::ErrorKind::Other, message));
    }
    Ok(())
}
