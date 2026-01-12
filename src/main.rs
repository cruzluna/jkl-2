mod cli;
mod context;
mod tmux;
mod tui;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    cli::run()
}
