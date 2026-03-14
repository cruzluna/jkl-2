use std::fs;
use std::io::{self, Write};
use std::path::Path;
use std::thread;
use std::time::Duration;

use thiserror::Error;

const DEFAULT_FOLLOW_INTERVAL_MS: u64 = 120;
const CLEAR_SCREEN: &str = "\x1b[2J\x1b[H";

#[derive(Error, Debug)]
pub enum PreviewError {
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),
}

pub fn run_follow(target_file: &Path, lines: usize) -> Result<(), PreviewError> {
    let lines = lines.max(1);
    let mut previous_frame = String::new();

    loop {
        let target = read_target(target_file);
        let frame = match target {
            Some(pane_id) => render_pane_frame(&pane_id, lines),
            None => "Waiting for pane selection from jkl list view...\n".to_string(),
        };

        if frame != previous_frame {
            print!("{CLEAR_SCREEN}{frame}");
            io::stdout().flush()?;
            previous_frame = frame;
        }

        thread::sleep(Duration::from_millis(DEFAULT_FOLLOW_INTERVAL_MS));
    }
}

fn read_target(target_file: &Path) -> Option<String> {
    let contents = match fs::read_to_string(target_file) {
        Ok(contents) => contents,
        Err(_) => return None,
    };

    let pane_id = contents.trim();
    if pane_id.is_empty() {
        None
    } else {
        Some(pane_id.to_string())
    }
}

fn render_pane_frame(pane_id: &str, lines: usize) -> String {
    let header = format!("Live pane preview: {pane_id}\n\n");
    match crate::tmux::capture_pane(pane_id, lines) {
        Ok(capture) => {
            if capture.trim().is_empty() {
                format!("{header}(pane is empty)\n")
            } else {
                format!("{header}{capture}")
            }
        }
        Err(error) => format!("{header}Unable to capture pane.\n{error}\n"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn read_target_returns_none_for_missing_file() {
        let dir = TempDir::new().expect("temp dir");
        let path = dir.path().join("target.txt");
        assert_eq!(read_target(&path), None);
    }

    #[test]
    fn read_target_trims_whitespace() {
        let dir = TempDir::new().expect("temp dir");
        let path = dir.path().join("target.txt");
        fs::write(&path, "  %9  \n").expect("write target");
        assert_eq!(read_target(&path), Some("%9".to_string()));
    }

    #[test]
    fn read_target_returns_none_for_empty_file() {
        let dir = TempDir::new().expect("temp dir");
        let path = dir.path().join("target.txt");
        fs::write(&path, "   \n").expect("write target");
        assert_eq!(read_target(&path), None);
    }
}
