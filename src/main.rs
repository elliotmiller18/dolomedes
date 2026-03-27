mod client;
mod kadem;
mod proto;
mod terminal;

use clap::{CommandFactory, Parser, error::ErrorKind};

#[derive(Parser)]
#[command(name = "dolomedes")]
struct Cli {
    config_path: std::path::PathBuf,
    routing_table_path: Option<std::path::PathBuf>,
}

fn main() -> anyhow::Result<()> {
    //note: this is vibe coded and subject to change
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(err) => match err.kind() {
            ErrorKind::DisplayHelp | ErrorKind::DisplayVersion => {
                err.print()?;
                return Ok(());
            }
            _ => {
                err.print()?;
                eprintln!();
                Cli::command().print_help()?;
                eprintln!();
                std::process::exit(2);
            }
        },
    };

    let never = client::serve(cli.config_path, cli.routing_table_path)?;
    match never {}
}
