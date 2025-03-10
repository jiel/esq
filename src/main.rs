mod commands;
mod elasticsearch;
mod utils;

use clap::{Parser, Subcommand};
use commands::alias::{AliasCommands, handle_alias_command};
use commands::cat::{CatArgs, handle_cat_command};
use commands::login::handle_login_command;
use commands::logout::handle_logout_command;
use commands::ls::handle_ls_command;
use utils::*;

#[derive(Parser)]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// List all Elasticsearch indices
    Ls,

    /// Display data from a specific index
    Cat(CatArgs),

    /// Manage aliases for indices used in the cat command
    Alias {
        #[command(subcommand)]
        command: AliasCommands,
    },

    /// Login to Elasticsearch instance
    Login,

    /// Logout from Elasticsearch instance
    Logout,
}

fn main() {
    if let Err(e) = run() {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}

fn run() -> Result<(), ESQError> {
    let cli = Cli::parse();

    // Try to load existing config at startup
    let config_dir = dirs::home_dir()
        .ok_or(ESQError::ConfigError(
            "Could not determine home directory".to_string(),
        ))?
        .join(".esq");
    let config_file = config_dir.join("config.toml");
    let config = load_config(&config_file)?;

    match &cli.command {
        Commands::Ls => handle_ls_command(config),
        Commands::Cat(args) => handle_cat_command(
            config,
            &args.index,
            &args.from,
            &args.to,
            &args.select_clause,
            &args.where_clause,
            args.follow,
            &args.around,
            &args.lines,
        ),
        Commands::Alias { command } => handle_alias_command(command),
        Commands::Login => handle_login_command(config, &config_file),
        Commands::Logout => handle_logout_command(config, &config_file),
    }
}
