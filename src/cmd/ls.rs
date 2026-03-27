use std::path::Path;

use crate::models::StoredNode;
use crate::storage::{db, query};

pub fn run(path: &Path, json: bool) -> anyhow::Result<()> {
    let conn = db::init_db()?;
    let normalized_path = normalize_cli_path(path);
    let symbols = query::get_file_symbols(&conn, &normalized_path)?;

    if json {
        println!("{}", serde_json::to_string_pretty(&symbols)?);
        return Ok(());
    }

    if symbols.is_empty() {
        println!("No symbols found for file: {}", normalized_path);
        return Ok(());
    }

    println!("Symbols in {}", normalized_path);
    for node in &symbols {
        println!("{}", format_file_symbol(node));
    }

    Ok(())
}

fn normalize_cli_path(path: &Path) -> String {
    let raw = path.to_string_lossy().trim().replace('\\', "/");

    raw.trim_start_matches("./").to_string()
}

fn format_file_symbol(node: &StoredNode) -> String {
    let mut parts = vec![format!("[{}]", node.kind), node.name.clone()];

    if let Some(span) = format_line_span(node.start_line, node.end_line) {
        parts.push(span);
    }

    parts.join(" ")
}

fn format_line_span(start_line: Option<i64>, end_line: Option<i64>) -> Option<String> {
    match (start_line, end_line) {
        (Some(start), Some(end)) if start == end => Some(format!("line {}", start)),
        (Some(start), Some(end)) => Some(format!("lines {}-{}", start, end)),
        (Some(start), None) => Some(format!("line {}", start)),
        _ => None,
    }
}
