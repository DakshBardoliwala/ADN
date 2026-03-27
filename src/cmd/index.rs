use crate::indexer::walker;
use crate::storage::db;
use std::path::PathBuf;

pub fn run(path: &PathBuf) -> anyhow::Result<()> {
    // Initialize the database
    let mut conn = db::init_db()?;

    // Defensive: CLI paste can include trailing newlines/spaces.
    let path = PathBuf::from(path.to_string_lossy().trim().to_string());

    // Start Recursive Walk
    println!("Walking directory ...");
    walker::process_directory(&path, &mut conn)?;

    println!("Indexing complete!");
    Ok(())
}
