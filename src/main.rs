#![feature(file_buffered)]
/// NOTE: this file is vibe coded.
mod client;
mod kadem;
mod pow;

use clap::{CommandFactory, Parser, Subcommand, error::ErrorKind};

#[derive(Parser)]
#[command(name = "dolomedes")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
    config_path: Option<std::path::PathBuf>,
    routing_table_path: Option<std::path::PathBuf>,
}

#[derive(Subcommand)]
enum Commands {
    Setup {
        #[arg(long)]
        datadir: Option<std::path::PathBuf>,
        #[arg(long)]
        port: Option<u16>,
    },
}

enum Mode {
    Serve {
        config_path: std::path::PathBuf,
        routing_table_path: Option<std::path::PathBuf>,
    },
    Setup {
        config_path: std::path::PathBuf,
        datadir: std::path::PathBuf,
        port: u16,
    },
}

fn parse_cli() -> Result<Mode, clap::Error> {
    let cli = Cli::try_parse()?;

    match (cli.command, cli.config_path, cli.routing_table_path) {
        (Some(Commands::Setup { datadir, port }), None, None) => Ok(Mode::Setup {
            config_path: std::path::PathBuf::from(client::DEFAULT_CONFIG_PATH),
            datadir: datadir.unwrap_or_else(|| std::path::PathBuf::from(client::DEFAULT_DATA_DIR)),
            port: port.unwrap_or(client::DEFAULT_PORT),
        }),
        (None, Some(config_path), routing_table_path) => Ok(Mode::Serve {
            config_path,
            routing_table_path,
        }),
        (None, None, _) => Err(Cli::command().error(
            ErrorKind::MissingRequiredArgument,
            "missing required argument <CONFIG_PATH>",
        )),
        (Some(Commands::Setup { .. }), Some(_), _) => Err(Cli::command().error(
            ErrorKind::ArgumentConflict,
            "setup does not accept client config or routing table arguments",
        )),
        (Some(Commands::Setup { .. }), None, Some(_)) => Err(Cli::command().error(
            ErrorKind::ArgumentConflict,
            "setup does not accept a routing table argument",
        )),
    }
}

fn main() -> anyhow::Result<()> {
    let mode = match parse_cli() {
        Ok(mode) => mode,
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

    match mode {
        Mode::Serve {
            config_path,
            routing_table_path,
        } => {
            let never = client::cli::serve(config_path, routing_table_path)?;
            match never {}
        }
        Mode::Setup {
            config_path,
            datadir,
            port,
        } => {
            client::cli::setup_env(config_path, datadir, port)?;
            Ok(())
        }
    }
}
