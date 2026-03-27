use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub enum NodeKind {
    File,
    Function,
    Class,
    Interface,
    Module,
}

#[derive(Debug)]
pub struct Symbol {
    pub name: String,
    pub kind: NodeKind,
    pub line_range: (usize, usize),
}

#[derive(Debug, Clone)]
pub struct PendingNode {
    pub id: String,
    pub kind: String,
    pub name: String,
    pub file_path: String,
    pub start_line: Option<i64>,
    pub end_line: Option<i64>,
    pub content_hash: Option<String>,
}

#[derive(Debug, Clone)]
pub struct PendingEdge {
    pub source_id: String,
    pub target_id: String,
    pub relation: String,
}

#[derive(Debug, Clone, Default)]
pub struct ParsedFileGraph {
    pub file_node: Option<PendingNode>,
    pub nodes: Vec<PendingNode>,
    pub edges: Vec<PendingEdge>,
}

#[derive(Debug, Clone, Serialize)]
pub struct StoredNode {
    pub id: String,
    pub kind: String,
    pub name: String,
    pub file_path: String,
    pub start_line: Option<i64>,
    pub end_line: Option<i64>,
    pub content_hash: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct StoredEdge {
    pub source_id: String,
    pub target_id: String,
    pub relation: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct NodeDetails {
    pub node: StoredNode,
    pub outgoing: Vec<StoredEdge>,
    pub incoming: Vec<StoredEdge>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TraceTreeNode {
    pub node: StoredNode,
    pub relation: String,
    pub depth: i64,
    pub children: Vec<TraceTreeNode>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TraceResult {
    pub target: StoredNode,
    pub max_depth: i64,
    pub children: Vec<TraceTreeNode>,
}
