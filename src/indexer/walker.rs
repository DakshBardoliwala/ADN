use crate::indexer::parser;
use ignore::WalkBuilder;
use rusqlite::Connection;
use std::path::Path;

pub fn process_directory(path: &Path, conn: &mut Connection) -> anyhow::Result<()> {
    let walker = WalkBuilder::new(path).build();

    for entry in walker {
        match entry {
            Ok(entry) => {
                let entry_path = entry.path();

                if entry_path.is_file() && is_supported_file(&entry_path) {
                    println!(
                        "Indexing file {:?}",
                        entry_path.strip_prefix(path).unwrap_or(entry_path)
                    );

                    if let Err(e) = parser::parse_file(entry_path, path, conn) {
                        eprintln!("Error parsing file {:?}: {}", entry_path, e);
                    }
                }
            }

            Err(e) => {
                eprintln!("Error walking directory: {}", e);
            }
        }
    }

    Ok(())
}

fn is_supported_file(path: &Path) -> bool {
    path.extension().map_or(false, |ext| ext == "py")
}
