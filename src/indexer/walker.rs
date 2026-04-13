use crate::indexer::parser;
use crate::models::DeferredImport;
use ignore::WalkBuilder;
use rusqlite::Connection;
use std::path::Path;

pub fn process_directory(
    path: &Path,
    conn: &mut Connection,
) -> anyhow::Result<Vec<DeferredImport>> {
    let walker = WalkBuilder::new(path).build();
    let mut deferred_imports = Vec::new();

    for entry in walker {
        match entry {
            Ok(entry) => {
                let entry_path = entry.path();

                if entry_path.is_file() && is_supported_file(entry_path) {
                    println!(
                        "Indexing file {:?}",
                        entry_path.strip_prefix(path).unwrap_or(entry_path)
                    );

                    match parser::parse_file(entry_path, path, conn) {
                        Ok(file_deferred_imports) => deferred_imports.extend(file_deferred_imports),
                        Err(e) => {
                            eprintln!("Error parsing file {:?}: {}", entry_path, e);
                        }
                    }
                }
            }

            Err(e) => {
                eprintln!("Error walking directory: {}", e);
            }
        }
    }

    Ok(deferred_imports)
}

fn is_supported_file(path: &Path) -> bool {
    path.extension().is_some_and(|ext| ext == "py")
}
