use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct PendingNode {
    pub id: String,
    pub kind: String,
    pub name: String,
    pub file_path: String,
    pub start_line: Option<i64>,
    pub end_line: Option<i64>,
    pub content_hash: Option<String>,
    pub indexed_at: String,
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

#[derive(Debug, Clone)]
pub struct DeferredImport {
    pub source_id: String,
    pub source_file_path: String,
    pub module_name: String,
    pub imported_name: Option<String>,
    pub is_wildcard: bool,
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
    pub indexed_at: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NodeIdentifier {
    pub name: String,
    pub file_path: String,
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
    pub ambiguous: bool,
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
    pub ambiguous: bool,
    pub max_depth: i64,
    pub children: Vec<TraceTreeNode>,
}

#[derive(Debug, Clone, Serialize)]
pub struct IndexedFileEntry {
    pub file_path: String,
    pub last_indexed: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct IndexedFilesStats {
    pub local_symbols: i64,
    pub external_modules: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct IndexedFilesResult {
    pub files: Vec<IndexedFileEntry>,
    pub stats: IndexedFilesStats,
}
