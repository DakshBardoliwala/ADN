use crate::models::StoredNode;
use crate::storage::{db, query};

pub fn run(query_text: &str, options: query::SearchOptions, json: bool) -> anyhow::Result<()> {
    let conn = db::init_db()?;
    let results = query::search_symbols(&conn, query_text, &options)?;

    if json {
        println!("{}", serde_json::to_string_pretty(&results)?);
        return Ok(());
    }

    if results.is_empty() {
        println!("No symbols found for query: {}", query_text.trim());
        return Ok(());
    }

    for node in &results {
        println!("{}", format_search_result(node));
    }

    Ok(())
}

fn format_search_result(node: &StoredNode) -> String {
    let mut parts = vec![
        format!("[{}]", node.kind),
        node.name.clone(),
        format!("({})", node.file_path),
    ];

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
