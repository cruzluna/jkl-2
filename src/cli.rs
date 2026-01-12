use clap::{Args, Parser, Subcommand};

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Tui => crate::tui::run(),
        Commands::Upsert(args) => handle_upsert(args),
    }
}

fn handle_upsert(args: UpsertArgs) -> Result<(), Box<dyn std::error::Error>> {
    let status = match args.status {
        Some(status) => Some(status.parse()?),
        None => None,
    };
    crate::context::upsert_context(args.session_id, args.session_name, status, args.context)
}

#[derive(Parser)]
#[command(name = "jkl", version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Tui,
    Upsert(UpsertArgs),
}

#[derive(Args)]
struct UpsertArgs {
    session_id: String,
    session_name: String,
    #[arg(long)]
    status: Option<String>,
    #[arg(long)]
    context: Option<String>,
}
