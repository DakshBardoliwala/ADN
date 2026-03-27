use crate::models::{StoredNode, TraceResult, TraceTreeNode};
use crate::storage::{db, query};

pub fn run(node_id: &str, json: bool) -> anyhow::Result<()> {
    let conn = db::init_db()?;
    let trace = query::trace_impact(&conn, node_id)?;

    if json {
        println!("{}", serde_json::to_string_pretty(&trace)?);
        return Ok(());
    }

    let Some(trace) = trace else {
        println!("No node found for id: {}", node_id.trim());
        return Ok(());
    };

    println!("{}", format_target_header(&trace));

    if trace.children.is_empty() {
        println!("No upstream dependencies found.");
        return Ok(());
    }

    print_tree(&trace.children, "");

    Ok(())
}

fn format_target_header(trace: &TraceResult) -> String {
    let mut lines = vec![
        format!("Trace Target: {}", format_node_summary(&trace.target)),
        format!("Max Depth: {}", trace.max_depth),
    ];

    if let Some(span) = format_line_span(trace.target.start_line, trace.target.end_line) {
        lines.push(format!("Span: {}", span));
    }

    lines.join("\n")
}

fn print_tree(nodes: &[TraceTreeNode], prefix: &str) {
    for (index, node) in nodes.iter().enumerate() {
        let is_last = index + 1 == nodes.len();
        let connector = if is_last { "└─" } else { "├─" };

        println!(
            "{}{} {}: {}",
            prefix,
            connector,
            node.relation,
            format_node_summary(&node.node)
        );

        let next_prefix = if is_last {
            format!("{prefix}   ")
        } else {
            format!("{prefix}│  ")
        };

        print_tree(&node.children, &next_prefix);
    }
}

fn format_node_summary(node: &StoredNode) -> String {
    let mut parts = vec![
        format!("[{}]", node.kind),
        node.name.clone(),
        format!("({})", node.file_path),
    ];

    if let Some(span) = format_line_span(node.start_line, node.end_line) {
        parts.push(span);
    }

    parts.push(format!("id={}", node.id));

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
