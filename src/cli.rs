use clap::{Args, Parser, Subcommand};
use std::io;

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Tui(args) => handle_tui(args),
        Commands::Upsert(args) => handle_upsert(args),
        Commands::Rename(args) => handle_rename(args),
    }
}

fn handle_tui(args: TuiArgs) -> Result<(), Box<dyn std::error::Error>> {
    if args.pane_state {
        let session_name = args
            .session_name
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "Missing --session-name"))?;
        let pane_id = args
            .pane_id
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "Missing --pane-id"))?;
        return crate::tui::run_pane_selector(session_name, pane_id);
    }
    crate::tui::run()
}

fn handle_upsert(args: UpsertArgs) -> Result<(), Box<dyn std::error::Error>> {
    let status = match args.status {
        Some(status) => Some(status.parse()?),
        None => None,
    };
    if let Some(pane_id) = args.pane_id {
        if args.context.is_some() {
            return Err(Box::new(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Pane context is not supported yet",
            )));
        }
        return crate::context::upsert_pane(&args.session_name, &pane_id, status);
    }
    crate::context::upsert_session(args.session_name, args.session_id, status, args.context)?;
    Ok(())
}

fn handle_rename(args: RenameArgs) -> Result<(), Box<dyn std::error::Error>> {
    crate::context::rename_session(&args.session_id, &args.session_name)
}

#[derive(Parser)]
#[command(name = "jkl", version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Tui(TuiArgs),
    Upsert(UpsertArgs),
    Rename(RenameArgs),
}

#[derive(Args)]
struct TuiArgs {
    #[arg(long)]
    pane_state: bool,
    #[arg(long)]
    session_name: Option<String>,
    #[arg(long)]
    pane_id: Option<String>,
}

#[derive(Args)]
struct UpsertArgs {
    session_name: String,
    #[arg(long)]
    session_id: Option<String>,
    #[arg(long)]
    pane_id: Option<String>,
    #[arg(long)]
    status: Option<String>,
    #[arg(long)]
    context: Option<String>,
}

#[derive(Args)]
struct RenameArgs {
    session_id: String,
    session_name: String,
}
