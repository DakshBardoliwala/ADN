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
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match &cli.command {
        Commands::Index { path } => {
            println!("Indexing project at: {:?}", path);
            cmd::index::run(path)?;
        }
    }

    Ok(())
}
