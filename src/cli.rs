use anyhow::Result;
use clap::{Args, Parser, Subcommand};
use log::{debug, info};

use crate::context::{AgentStatus, ContextError};
use crate::tui::TuiError;

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
    /// The name of the session to open the status selector popup for. Helps for quicker lookup
    /// Dont need to specifiy any other args if you just want to open the status selector popup for a session.
    #[arg(long, num_args = 1..)]
    session_name: Option<Vec<String>>,
    /// Open the pane status selector popup
    #[arg(long, alias = "pane-state")]
    open_pane_state: bool,
    /// The ID of the pane to open the status selector popup for
    #[arg(long)]
    pane_id: Option<String>,
}

#[derive(Args)]
struct UpsertArgs {
    #[arg(num_args = 1..)]
    session_name: Vec<String>,
    #[arg(long)]
    session_id: Option<String>,
    #[arg(long)]
    pane_id: Option<String>,
    #[arg(long)]
    status: Option<String>,
    #[arg(long, num_args = 1..)]
    context: Option<Vec<String>>,
}

#[derive(Args)]
struct RenameArgs {
    session_id: String,
    #[arg(num_args = 1..)]
    session_name: Vec<String>,
}

pub fn run() -> Result<()> {
    let cli = Cli::parse();
    debug!("cli parsed");
    match cli.command {
        Commands::Tui(args) => {
            info!("command=tui");
            handle_tui(args)?
        }
        Commands::Upsert(args) => {
            info!("command=upsert");
            handle_upsert(args)?
        }
        Commands::Rename(args) => {
            info!("command=rename");
            handle_rename(args)?
        }
    };
    Ok(())
}

fn handle_tui(args: TuiArgs) -> Result<(), TuiError> {
    if args.open_pane_state {
        let session_name = args.session_name.map(join_tokens);
        let pane_id = args.pane_id;
        info!(
            "tui pane selector requested session={:?} pane_id={:?}",
            session_name, pane_id
        );
        return crate::tui::run_pane_selector(
            pane_id.unwrap_or("FAILED TO GET PANE ID".to_string()),
            session_name,
        );
    }
    info!("tui requested");
    crate::tui::run()
}

fn handle_upsert(args: UpsertArgs) -> Result<(), ContextError> {
    let status: Option<AgentStatus> = match args.status {
        Some(s) => Some(
            s.parse::<AgentStatus>()
                .map_err(|e| ContextError::InvalidStatus(e.to_string()))?,
        ),
        None => None,
    };
    let session_name = join_tokens(args.session_name);
    let context = args.context.map(join_tokens);
    debug!(
        "upsert requested session_name={} pane_id={:?} status={:?} context_present={}",
        session_name,
        args.pane_id,
        status,
        context.is_some()
    );
    if let Some(pane_id) = args.pane_id {
        return crate::context::upsert_pane(&session_name, &pane_id, None, status, context);
    }
    crate::context::upsert_session(session_name, args.session_id, status, context)?;
    Ok(())
}

fn handle_rename(args: RenameArgs) -> Result<(), ContextError> {
    let session_name = join_tokens(args.session_name);
    info!(
        "rename requested session_id={} session_name={}",
        args.session_id, session_name
    );
    crate::context::rename_session(&args.session_id, &session_name)?;
    Ok(())
}

fn join_tokens(tokens: Vec<String>) -> String {
    tokens.join(" ")
}
