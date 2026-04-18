use crate::models::IndexedFileEntry;
use crate::storage::{db, query};

pub fn run(json: bool) -> anyhow::Result<()> {
    let conn = db::init_db()?;
    let result = query::list_indexed_files(&conn)?;

    if json {
        println!("{}", serde_json::to_string_pretty(&result)?);
        return Ok(());
    }

    print_file_table(&result.files);
    println!();
    println!("Local Symbols: {}", result.stats.local_symbols);
    println!("External Modules: {}", result.stats.external_modules);

    Ok(())
}

fn print_file_table(files: &[IndexedFileEntry]) {
    if files.is_empty() {
        println!("No indexed files found.");
        return;
    }

    let path_width = files
        .iter()
        .map(|entry| entry.file_path.len())
        .max()
        .unwrap_or("File Path".len())
        .max("File Path".len());
    let timestamp_width = files
        .iter()
        .map(|entry| entry.last_indexed.len())
        .max()
        .unwrap_or("Last Indexed".len())
        .max("Last Indexed".len());

    println!(
        "{:<path_width$}  {:<timestamp_width$}",
        "File Path",
        "Last Indexed",
        path_width = path_width,
        timestamp_width = timestamp_width
    );
    println!(
        "{:-<path_width$}  {:-<timestamp_width$}",
        "",
        "",
        path_width = path_width,
        timestamp_width = timestamp_width
    );

    for entry in files {
        println!(
            "{:<path_width$}  {:<timestamp_width$}",
            entry.file_path,
            entry.last_indexed,
            path_width = path_width,
            timestamp_width = timestamp_width
        );
    }
}
