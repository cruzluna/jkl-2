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
    Sync,
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
    pane_name: Option<String>,
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
        Commands::Sync => {
            info!("command=sync");
            handle_sync()?
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
        "upsert requested session_name={} pane_id={:?} pane_name={:?} status={:?} context_present={}",
        session_name,
        args.pane_id,
        args.pane_name,
        status,
        context.is_some()
    );
    if let Some(pane_id) = args.pane_id {
        return crate::context::upsert_pane(
            &session_name,
            &pane_id,
            args.pane_name,
            status,
            context,
        );
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

fn handle_sync() -> Result<()> {
    let sessions = crate::tmux::list_sessions()?;
    let panes = crate::tmux::list_panes()?;
    let summary = crate::context::sync_with_tmux(&sessions, &panes)?;
    info!(
        "sync finished removed_sessions={} removed_panes={} renamed_sessions={}",
        summary.removed_sessions, summary.removed_panes, summary.renamed_sessions
    );
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
    use crate::context::{AgentStatus, load_contexts, session_key};
    use crate::test_utils::EnvGuard;
    #[cfg(unix)]
    use std::fs;
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;
    #[cfg(unix)]
    use std::path::PathBuf;

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
            pane_name: None,
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
            pane_name: None,
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
            pane_name: Some("planner".to_string()),
            status: Some("waiting".to_string()),
            context: Some(vec!["pane".to_string(), "ctx".to_string()]),
        };

        handle_upsert(args).expect("handle upsert");

        let contexts = load_contexts().expect("load contexts");
        let session = contexts.get(&session_key("Alpha")).expect("session entry");
        let pane = session.panes.get("%1").expect("pane entry");
        assert_eq!(pane.pane_name.as_deref(), Some("planner"));
        assert_eq!(pane.pane_status, Some(AgentStatus::Waiting));
        assert_eq!(pane.pane_context.as_deref(), Some("pane ctx"));
    }

    #[test]
    fn handle_rename_moves_session_context() {
        let mut env = EnvGuard::new("cli-rename");
        env.set_temp_home();

        crate::context::upsert_session("Old".to_string(), Some("sid".to_string()), None, None)
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

    #[cfg(unix)]
    fn setup_fake_tmux(env: &mut EnvGuard) -> PathBuf {
        let bin_dir = env.temp_dir().join("bin");
        fs::create_dir_all(&bin_dir).expect("create bin dir");
        let script_path = bin_dir.join("tmux");
        let script = r#"#!/bin/sh
case "$1" in
  list-sessions)
    printf "%s" "${TMUX_LIST_SESSIONS:-}"
    exit 0
    ;;
  list-panes)
    printf "%s" "${TMUX_LIST_PANES:-}"
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
        env.set_var("PATH", format!("{}:{old_path}", bin_dir.display()));
        script_path
    }

    #[test]
    #[cfg(unix)]
    fn handle_sync_prunes_stale_and_rekeys_renamed_session() {
        let mut env = EnvGuard::new("cli-sync");
        env.set_temp_home();
        setup_fake_tmux(&mut env);

        crate::context::upsert_session(
            "old-name".to_string(),
            Some("@1".to_string()),
            Some(AgentStatus::Working),
            Some("ctx".to_string()),
        )
        .expect("seed renamed session");
        crate::context::upsert_pane("old-name", "%1", None, None, None).expect("pane keep");
        crate::context::upsert_pane("old-name", "%9", None, None, None).expect("pane stale");
        crate::context::upsert_session("gone".to_string(), Some("@9".to_string()), None, None)
            .expect("seed stale session");

        env.set_var("TMUX_LIST_SESSIONS", "@1\tnew-name\n");
        env.set_var("TMUX_LIST_PANES", "new-name\t%1\n");

        handle_sync().expect("sync");

        let contexts = load_contexts().expect("load contexts");
        assert!(contexts.get(&session_key("old-name")).is_none());
        assert!(contexts.get(&session_key("gone")).is_none());
        let renamed = contexts
            .get(&session_key("new-name"))
            .expect("renamed session");
        assert_eq!(renamed.session_name.as_deref(), Some("new-name"));
        assert_eq!(renamed.session_id.as_deref(), Some("@1"));
        assert!(renamed.panes.contains_key("%1"));
        assert!(!renamed.panes.contains_key("%9"));
    }
}
