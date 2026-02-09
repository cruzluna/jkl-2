mod agents;
mod cli;
mod context;
mod paths;
#[cfg(test)]
mod test_utils;
mod tmux;
mod tui;
mod uninstall;
mod update;

fn main() -> Result<(), anyhow::Error> {
    let log_path = init_logging()?;
    log::info!("logging initialized to: {}", log_path.display());
    cli::run()
}

fn init_logging() -> Result<std::path::PathBuf, anyhow::Error> {
    use flexi_logger::{Cleanup, Criterion, FileSpec, Logger, Naming};

    let log_directory =
        crate::paths::config_dir().ok_or_else(|| anyhow::anyhow!("missing config directory"))?;

    // Ensure log directory exists
    std::fs::create_dir_all(&log_directory)?;

    let file_spec = FileSpec::default()
        .directory(&log_directory)
        .basename("jkl")
        .suffix("log");

    Logger::try_with_str("debug")?
        .log_to_file(file_spec)
        .rotate(
            Criterion::Size(50 * 1024 * 1024),
            Naming::Timestamps,
            Cleanup::KeepLogFiles(5),
        )
        .start()?;

    Ok(log_directory.join("jkl.log"))
}
