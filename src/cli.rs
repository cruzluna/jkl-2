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
    Update(UpdateArgs),
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

#[derive(Args)]
struct UpdateArgs {
    /// Include pre-release versions.
    #[arg(long)]
    prerelease: bool,
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
        Commands::Update(args) => {
            info!("command=update");
            handle_update(args)?
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

fn handle_update(args: UpdateArgs) -> Result<()> {
    crate::update::run(args.prerelease)
}

fn join_tokens(tokens: Vec<String>) -> String {
    tokens.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::{load_contexts, session_key, AgentStatus};
    use crate::test_utils::EnvGuard;

    #[test]
    fn join_tokens_joins_with_spaces() {
        let joined = join_tokens(vec!["one".to_string(), "two".to_string()]);
        assert_eq!(joined, "one two");
    }

    #[test]
    fn handle_upsert_rejects_invalid_status() {
        let args = UpsertArgs {
            session_name: vec!["Alpha".to_string()],
            session_id: None,
            pane_id: None,
            status: Some("not-a-status".to_string()),
            context: None,
        };

        let err = handle_upsert(args).expect_err("expected invalid status");
        match err {
            ContextError::InvalidStatus(_) => {}
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn handle_upsert_session_persists_values() {
        let mut env = EnvGuard::new("cli-upsert-session");
        env.set_temp_home();

        let args = UpsertArgs {
            session_name: vec!["Alpha".to_string(), "Beta".to_string()],
            session_id: Some("sid".to_string()),
            pane_id: None,
            status: Some("Working".to_string()),
            context: Some(vec!["hello".to_string(), "world".to_string()]),
        };

        handle_upsert(args).expect("handle upsert");

        let contexts = load_contexts().expect("load contexts");
        let entry = contexts
            .get(&session_key("Alpha Beta"))
            .expect("session entry");
        assert_eq!(entry.session_name.as_deref(), Some("Alpha Beta"));
        assert_eq!(entry.session_id.as_deref(), Some("sid"));
        assert_eq!(entry.session_status, Some(AgentStatus::Working));
        assert_eq!(entry.session_context.as_deref(), Some("hello world"));
    }

    #[test]
    fn handle_upsert_pane_updates_pane_context() {
        let mut env = EnvGuard::new("cli-upsert-pane");
        env.set_temp_home();

        let args = UpsertArgs {
            session_name: vec!["Alpha".to_string()],
            session_id: None,
            pane_id: Some("%1".to_string()),
            status: Some("waiting".to_string()),
            context: Some(vec!["pane".to_string(), "ctx".to_string()]),
        };

        handle_upsert(args).expect("handle upsert");

        let contexts = load_contexts().expect("load contexts");
        let session = contexts
            .get(&session_key("Alpha"))
            .expect("session entry");
        let pane = session.panes.get("%1").expect("pane entry");
        assert_eq!(pane.pane_status, Some(AgentStatus::Waiting));
        assert_eq!(pane.pane_context.as_deref(), Some("pane ctx"));
    }

    #[test]
    fn handle_rename_moves_session_context() {
        let mut env = EnvGuard::new("cli-rename");
        env.set_temp_home();

        crate::context::upsert_session(
            "Old".to_string(),
            Some("sid".to_string()),
            None,
            None,
        )
        .expect("seed session");

        let args = RenameArgs {
            session_id: "sid".to_string(),
            session_name: vec!["New".to_string(), "Name".to_string()],
        };

        handle_rename(args).expect("handle rename");

        let contexts = load_contexts().expect("load contexts");
        assert!(contexts.get(&session_key("Old")).is_none());
        let entry = contexts
            .get(&session_key("New Name"))
            .expect("renamed entry");
        assert_eq!(entry.session_id.as_deref(), Some("sid"));
        assert_eq!(entry.session_name.as_deref(), Some("New Name"));
    }
}
