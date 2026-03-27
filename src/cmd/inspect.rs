use crate::models::{StoredEdge, StoredNode};
use crate::storage::{db, query};

pub fn run(node_id: &str, json: bool) -> anyhow::Result<()> {
    let conn = db::init_db()?;
    let details = query::get_node_details(&conn, node_id)?;

    if json {
        println!("{}", serde_json::to_string_pretty(&details)?);
        return Ok(());
    }

    let Some(details) = details else {
        println!("No node found for id: {}", node_id.trim());
        return Ok(());
    };

    println!("{}", format_node_header(&details.node));
    println!();
    println!("Outgoing:");
    print_edges(&details.outgoing);
    println!();
    println!("Incoming:");
    print_edges(&details.incoming);

    Ok(())
}

fn format_node_header(node: &StoredNode) -> String {
    let mut lines = vec![
        format!("ID: {}", node.id),
        format!("Kind: {}", node.kind),
        format!("Name: {}", node.name),
        format!("Path: {}", node.file_path),
    ];

    if let Some(span) = format_line_span(node.start_line, node.end_line) {
        lines.push(format!("Span: {}", span));
    }

    lines.join("\n")
}

fn print_edges(edges: &[StoredEdge]) {
    if edges.is_empty() {
        println!("  none");
        return;
    }

    for edge in edges {
        println!(
            "  {}: {} -> {}",
            edge.relation, edge.source_id, edge.target_id
        );
    }
}

fn format_line_span(start_line: Option<i64>, end_line: Option<i64>) -> Option<String> {
    match (start_line, end_line) {
        (Some(start), Some(end)) if start == end => Some(format!("line {}", start)),
        (Some(start), Some(end)) => Some(format!("lines {}-{}", start, end)),
        (Some(start), None) => Some(format!("line {}", start)),
        _ => None,
    }
}
