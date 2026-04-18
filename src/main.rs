mod cmd;
mod indexer;
mod models;
mod storage;

use anyhow::bail;
use clap::{Parser, Subcommand};
use models::NodeIdentifier;
use std::path::PathBuf;
use storage::query;

#[derive(Parser)]
#[command(name = "adn")]
#[command(about = "Architectural Discovery Navigation CLI", long_about = None)]
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
        /// Maximum number of results to return
        #[arg(long, default_value_t = 50)]
        limit: i64,
        /// Number of matching results to skip before returning rows
        #[arg(long, default_value_t = 0)]
        offset: i64,
        /// Only return symbols from locally indexed files
        #[arg(long)]
        local: bool,
        /// Emit JSON output
        #[arg(long)]
        json: bool,
    },
    /// Inspect a node and its edges
    Inspect {
        /// Exact node identifier (takes precedence over --name/--file)
        id: Option<String>,
        /// Symbol name for file-scoped lookup when no ID is provided
        #[arg(long)]
        name: Option<String>,
        /// Repo-relative file path for file-scoped lookup when no ID is provided
        #[arg(long)]
        file: Option<String>,
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
        /// Exact node identifier (takes precedence over --name/--file)
        id: Option<String>,
        /// Symbol name for file-scoped lookup when no ID is provided
        #[arg(long)]
        name: Option<String>,
        /// Repo-relative file path for file-scoped lookup when no ID is provided
        #[arg(long)]
        file: Option<String>,
        /// Maximum recursive trace depth
        #[arg(long, default_value_t = 2)]
        depth: i64,
        /// Emit JSON output
        #[arg(long)]
        json: bool,
    },
    /// Show indexed files and symbol counts
    Stats {
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
        Commands::Search {
            query,
            limit,
            offset,
            local,
            json,
        } => {
            cmd::search::run(
                query,
                query::SearchOptions {
                    limit: *limit,
                    offset: *offset,
                    local_only: *local,
                },
                *json,
            )?;
        }
        Commands::Inspect {
            id,
            name,
            file,
            json,
        } => {
            let lookup = build_node_lookup(id.as_deref(), name.as_deref(), file.as_deref())?;
            cmd::inspect::run(lookup, *json)?;
        }
        Commands::Ls { path, json } => {
            cmd::ls::run(path, *json)?;
        }
        Commands::Trace {
            id,
            name,
            file,
            depth,
            json,
        } => {
            let lookup = build_node_lookup(id.as_deref(), name.as_deref(), file.as_deref())?;
            cmd::trace::run(lookup, query::TraceOptions { depth: *depth }, *json)?;
        }
        Commands::Stats { json } => {
            cmd::stats::run(*json)?;
        }
        Commands::Mcp { command } => match command {
            McpCommands::Serve => cmd::mcp::run_serve()?,
        },
    }

    Ok(())
}

fn build_node_lookup(
    id: Option<&str>,
    name: Option<&str>,
    file: Option<&str>,
) -> anyhow::Result<query::NodeLookup> {
    if let Some(id) = id {
        return Ok(query::NodeLookup::Id(id.trim().to_string()));
    }

    match (name, file) {
        (Some(name), Some(file_path)) => Ok(query::NodeLookup::Identifier(NodeIdentifier {
            name: name.trim().to_string(),
            file_path: normalize_cli_path(file_path),
        })),
        (None, None) => bail!("provide either an ID or both --name and --file"),
        _ => bail!("both --name and --file are required when ID is omitted"),
    }
}

fn normalize_cli_path(path: &str) -> String {
    path.trim()
        .replace('\\', "/")
        .trim_start_matches("./")
        .to_string()
}
