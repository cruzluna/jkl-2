use std::fs;
use std::path::{Path, PathBuf};

const BASIC_INSTRUCTIONS: &str = r#"## Basic JKL Instructions

- Do not invoke the TUI command (`jkl tui`) from automated agents.
- Upsert session metadata: `jkl upsert <session_name...> [--session-id <session_id>] [--status <status>] [--context <text...>]`
- Upsert pane metadata: `jkl upsert <session_name...> --pane-id <pane_id> [--status <status>] [--context <text...>]`
- Rename a session entry: `jkl rename <session_id> <session_name...>`
- Sync persisted metadata with live tmux state: `jkl sync`
"#;

#[derive(Debug, Default, PartialEq, Eq)]
pub struct AppendSummary {
    pub files_updated: usize,
    pub files_skipped: usize,
}

pub fn append_basic_instructions(root: &Path) -> Result<AppendSummary, std::io::Error> {
    let mut summary = AppendSummary::default();
    let mut stack = vec![root.to_path_buf()];

    while let Some(path) = stack.pop() {
        let entries = match fs::read_dir(&path) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error),
        };

        for entry in entries {
            let entry = entry?;
            let entry_path = entry.path();
            let file_type = entry.file_type()?;
            if file_type.is_dir() {
                if file_type.is_symlink() {
                    continue;
                }
                stack.push(entry_path);
                continue;
            }
            if !file_type.is_file() {
                continue;
            }
            if entry.file_name() != "AGENTS.md" {
                continue;
            }
            let updated = append_instructions_to_file(&entry_path)?;
            if updated {
                summary.files_updated += 1;
            } else {
                summary.files_skipped += 1;
            }
        }
    }

    Ok(summary)
}

fn append_instructions_to_file(path: &Path) -> Result<bool, std::io::Error> {
    let contents = fs::read_to_string(path)?;
    if contents.contains("## Basic JKL Instructions") {
        return Ok(false);
    }
    let mut next = String::with_capacity(contents.len() + BASIC_INSTRUCTIONS.len() + 2);
    next.push_str(contents.trim_end());
    next.push_str("\n\n");
    next.push_str(BASIC_INSTRUCTIONS);
    fs::write(path, next)?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(prefix: &str) -> PathBuf {
        let mut dir = std::env::temp_dir();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_else(|_| std::time::Duration::from_nanos(0))
            .as_nanos();
        dir.push(format!("{}-{}", prefix, nanos));
        fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    #[test]
    fn append_basic_instructions_updates_nested_agents() {
        let root = temp_dir("agents-append");
        let nested = root.join("nested");
        fs::create_dir_all(&nested).expect("create nested");
        let root_file = root.join("AGENTS.md");
        let nested_file = nested.join("AGENTS.md");
        fs::write(&root_file, "# Root\n").expect("write root");
        fs::write(&nested_file, "# Nested\n").expect("write nested");

        let summary = append_basic_instructions(&root).expect("append");

        assert_eq!(summary.files_updated, 2);
        let root_contents = fs::read_to_string(&root_file).expect("read root");
        let nested_contents = fs::read_to_string(&nested_file).expect("read nested");
        assert!(root_contents.contains("## Basic JKL Instructions"));
        assert!(nested_contents.contains("## Basic JKL Instructions"));
    }

    #[test]
    fn append_basic_instructions_skips_existing_section() {
        let root = temp_dir("agents-skip");
        let path = root.join("AGENTS.md");
        fs::write(&path, BASIC_INSTRUCTIONS).expect("write instructions");

        let summary = append_basic_instructions(&root).expect("append");

        assert_eq!(summary.files_updated, 0);
        assert_eq!(summary.files_skipped, 1);
    }
}
