mod cmd;
mod indexer;
mod models;
mod storage;

use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "adn")]
#[command(about = "Architectural Discovery & Navigation CLI", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Index a directory to build the knowledge graph
    Index {
        /// The path to the directory
        path: PathBuf,
    },
    /// Search symbols by name
    Search {
        /// Symbol name fragment
        query: String,
        /// Emit JSON output
        #[arg(long)]
        json: bool,
    },
    /// Inspect a node and its edges
    Inspect {
        /// Node identifier
        id: String,
        /// Emit JSON output
        #[arg(long)]
        json: bool,
    },
    /// List symbols for a file
    Ls {
        /// Repo-relative file path
        path: PathBuf,
        /// Emit JSON output
        #[arg(long)]
        json: bool,
    },
    /// Trace the upstream impact radius for a node
    Trace {
        /// Node identifier
        id: String,
        /// Emit JSON output
        #[arg(long)]
        json: bool,
    },
    /// MCP server commands
    Mcp {
        #[command(subcommand)]
        command: McpCommands,
    },
}

#[derive(Subcommand)]
enum McpCommands {
    /// Start the MCP server
    Serve,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match &cli.command {
        Commands::Index { path } => {
            println!("Indexing project at: {:?}", path);
            cmd::index::run(path)?;
        }
        Commands::Search { query, json } => {
            cmd::search::run(query, *json)?;
        }
        Commands::Inspect { id, json } => {
            cmd::inspect::run(id, *json)?;
        }
        Commands::Ls { path, json } => {
            cmd::ls::run(path, *json)?;
        }
        Commands::Trace { id, json } => {
            cmd::trace::run(id, *json)?;
        }
        Commands::Mcp { command } => match command {
            McpCommands::Serve => cmd::mcp::run_serve()?,
        },
    }

    Ok(())
}
