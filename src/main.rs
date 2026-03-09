mod cli;
mod context;
mod init;
#[cfg(test)]
mod test_utils;
mod tmux;
mod tui;
mod uninstall;
mod update;

const LOG_SYMLINK_FILENAME: &str = "jkl.log";
const ISSUE_INSTRUCTIONS: &str =
    "Submit issues to github.com/cruzluna/jkl-2/issues and attach logs.";
const UNKNOWN_ERROR_MESSAGE: &str = "unknown error (see log for details)";

fn main() {
    if let Err(error) = run_with_logging() {
        eprintln!("Error: {}", error.user_message);
        if let Some(log_path) = &error.log_path {
            eprintln!("Details written to: {}", log_path.display());
        } else {
            eprintln!("Detailed logging unavailable: logging could not be initialized.");
        }
        eprintln!("{ISSUE_INSTRUCTIONS}");
        std::process::exit(1);
    }
}

fn run_with_logging() -> Result<(), MainError> {
    // Keep the handle alive until the command finishes so file logging stays active.
    let (log_path, _logger_handle) = init_logging().map_err(MainError::logging_init)?;
    log::info!("logging initialized to: {}", log_path.display());
    match cli::run() {
        Ok(()) => Ok(()),
        Err(source) => {
            let messages = normalized_error_messages(&source);
            let user_message = display_message(&messages, true);
            log_command_failure(&source, &messages, &user_message, &log_path);
            Err(MainError::command_failed(user_message, log_path))
        }
    }
}

fn init_logging() -> Result<(std::path::PathBuf, flexi_logger::LoggerHandle), anyhow::Error> {
    use flexi_logger::{Cleanup, Criterion, FileSpec, Logger, Naming};

    let home_dir = std::env::var("HOME")?;
    let log_directory = std::path::PathBuf::from(home_dir)
        .join(".config")
        .join("jkl");

    // Ensure log directory exists
    std::fs::create_dir_all(&log_directory)?;

    let file_spec = FileSpec::default()
        .directory(&log_directory)
        .basename("jkl")
        .suffix("log");

    let logger_handle = Logger::try_with_str("debug")?
        .log_to_file(file_spec)
        .append()
        // flexi_logger keeps its rotated "current" file under an internal infix;
        // expose a stable user-facing path alongside it.
        .create_symlink(log_directory.join(LOG_SYMLINK_FILENAME))
        .rotate(
            Criterion::Size(50 * 1024 * 1024),
            Naming::Timestamps,
            Cleanup::KeepLogFiles(5),
        )
        .start()?;

    Ok((stable_log_path(&log_directory), logger_handle))
}

fn stable_log_path(log_directory: &std::path::Path) -> std::path::PathBuf {
    log_directory.join(LOG_SYMLINK_FILENAME)
}

fn normalized_error_messages(error: &anyhow::Error) -> Vec<String> {
    error
        .chain()
        .map(|cause| cause.to_string())
        .map(|message| message.trim().to_string())
        .filter(|message| !message.is_empty())
        .collect()
}

fn display_message(messages: &[String], has_log_path: bool) -> String {
    messages.first().cloned().unwrap_or_else(|| {
        if has_log_path {
            UNKNOWN_ERROR_MESSAGE.to_string()
        } else {
            "unknown error".to_string()
        }
    })
}

fn log_command_failure(
    error: &anyhow::Error,
    messages: &[String],
    user_message: &str,
    log_path: &std::path::Path,
) {
    log::error!(
        "command failed user_message={} log_file={}",
        user_message,
        log_path.display()
    );

    if messages.is_empty() {
        log::error!("cause[0]: {UNKNOWN_ERROR_MESSAGE}");
        log::debug!("command error chain={UNKNOWN_ERROR_MESSAGE}");
    } else {
        for (index, message) in messages.iter().enumerate() {
            log::error!("cause[{index}]: {message}");
        }
        log::debug!("command error chain={}", messages.join(" -> "));
    }

    log::debug!("command error debug={:?}", error);
}

struct MainError {
    user_message: String,
    log_path: Option<std::path::PathBuf>,
}

impl MainError {
    fn logging_init(source: anyhow::Error) -> Self {
        let messages = normalized_error_messages(&source);
        Self {
            user_message: display_message(&messages, false),
            log_path: None,
        }
    }

    fn command_failed(user_message: String, log_path: std::path::PathBuf) -> Self {
        Self {
            user_message,
            log_path: Some(log_path),
        }
    }
}

impl std::fmt::Display for MainError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.user_message)
    }
}

#[cfg(test)]
mod tests {
    use super::{MainError, UNKNOWN_ERROR_MESSAGE, display_message, normalized_error_messages};
    use anyhow::Context;

    #[test]
    fn normalized_error_messages_skip_blank_context_and_trim_output() {
        let source = Err::<(), _>(anyhow::anyhow!("boom"))
            .context("   ")
            .unwrap_err();
        let messages = normalized_error_messages(&source);

        assert_eq!(messages, vec!["boom".to_string()]);
        assert_eq!(display_message(&messages, true), "boom");
    }

    #[test]
    fn normalized_error_messages_preserve_order_for_display() {
        let source = Err::<(), _>(anyhow::anyhow!("boom"))
            .context("  outer failure  ")
            .unwrap_err();
        let messages = normalized_error_messages(&source);

        assert_eq!(
            messages,
            vec!["outer failure".to_string(), "boom".to_string()]
        );
        assert_eq!(display_message(&messages, true), "outer failure");
    }

    #[test]
    fn display_message_uses_log_aware_fallbacks() {
        let messages = Vec::new();
        assert_eq!(display_message(&messages, true), UNKNOWN_ERROR_MESSAGE);
        assert_eq!(display_message(&messages, false), "unknown error");
    }

    #[test]
    fn logging_init_uses_plain_fallback_without_log_path() {
        let error = MainError::logging_init(anyhow::anyhow!("   "));
        assert_eq!(error.user_message, "unknown error");
        assert!(error.log_path.is_none());
    }
}
