use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

use anyhow::{anyhow, bail, Context};
use rusqlite::{Connection, OptionalExtension};
use tree_sitter::{Language, Node, Parser, Query, QueryCursor, StreamingIterator, Tree};
use tree_sitter_python::LANGUAGE;
use uuid::Uuid;

use crate::indexer::languages::python::IMPORT_QUERY_STR;
use crate::models::{ParsedFileGraph, PendingEdge, PendingNode};
use crate::storage::db;

const MAX_SCOPE_DEPTH: usize = 1024;
const MODULE_FILE_PATH: &str = "<module>";

#[derive(Clone)]
struct ScopeContext {
    id: String,
    kind: ScopeKind,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum ScopeKind {
    Class,
    Function,
}

#[derive(Default)]
struct ExtractionState {
    graph: ParsedFileGraph,
    module_ids: HashMap<String, String>,
    edge_keys: HashSet<(String, String, String)>,
}

pub fn parse_file(path: &Path, project_root: &Path, conn: &mut Connection) -> anyhow::Result<()> {
    let source_code = fs::read_to_string(path)?;
    let relative_file_path = relative_file_path(path, project_root)?;
    let content_hash = blake3::hash(source_code.as_bytes()).to_hex().to_string();

    if db::get_file_content_hash(conn, &relative_file_path)?.as_deref() == Some(&content_hash) {
        return Ok(());
    }

    db::delete_file_graph(conn, &relative_file_path)?;

    let tree = parse_python_tree(&source_code)?;
    let graph =
        extract_symbols_and_edges(&relative_file_path, conn, &source_code, &tree, content_hash)?;

    db::persist_file_graph(conn, &graph)
}

fn parse_python_tree(source_code: &str) -> anyhow::Result<Tree> {
    let mut parser = Parser::new();
    let language = Language::new(LANGUAGE);

    parser.set_language(&language)?;

    parser
        .parse(source_code, None)
        .ok_or_else(|| anyhow!("tree-sitter failed to parse file"))
}

fn extract_symbols_and_edges(
    relative_file_path: &str,
    conn: &Connection,
    source_code: &str,
    tree: &Tree,
    content_hash: String,
) -> anyhow::Result<ParsedFileGraph> {
    let file_node = PendingNode {
        id: Uuid::new_v4().to_string(),
        kind: "file".to_string(),
        name: relative_file_path.to_string(),
        file_path: relative_file_path.to_string(),
        start_line: None,
        end_line: None,
        content_hash: Some(content_hash),
    };

    let mut state = ExtractionState {
        graph: ParsedFileGraph {
            file_node: Some(file_node.clone()),
            nodes: Vec::new(),
            edges: Vec::new(),
        },
        ..ExtractionState::default()
    };

    walk_scope(
        tree.root_node(),
        source_code,
        relative_file_path,
        &mut state,
        None,
        0,
    )?;

    extract_imports(
        conn,
        tree.root_node(),
        source_code,
        relative_file_path,
        &file_node.id,
        &mut state,
    )?;

    Ok(state.graph)
}

fn walk_scope(
    node: Node,
    source_code: &str,
    file_path: &str,
    state: &mut ExtractionState,
    parent_scope: Option<ScopeContext>,
    depth: usize,
) -> anyhow::Result<()> {
    if depth > MAX_SCOPE_DEPTH {
        bail!("maximum recursion depth exceeded while walking Python AST");
    }

    match node.kind() {
        "class_definition" => {
            let class_name = child_field_text(node, "name", source_code)?
                .ok_or_else(|| anyhow!("class_definition missing name field"))?;
            let class_node = build_symbol_node("class", class_name, file_path, node);
            let class_id = class_node.id.clone();
            state.graph.nodes.push(class_node);

            if let Some(parent_scope) = parent_scope.as_ref() {
                if parent_scope.kind == ScopeKind::Class {
                    push_edge(state, &parent_scope.id, &class_id, "defines");
                }
            }

            let next_scope = Some(ScopeContext {
                id: class_id,
                kind: ScopeKind::Class,
            });

            if let Some(body) = node.child_by_field_name("body") {
                walk_scope(body, source_code, file_path, state, next_scope, depth + 1)?;
            }

            Ok(())
        }
        "function_definition" => {
            let function_name = child_field_text(node, "name", source_code)?
                .ok_or_else(|| anyhow!("function_definition missing name field"))?;
            let function_node = build_symbol_node("function", function_name, file_path, node);
            let function_id = function_node.id.clone();
            state.graph.nodes.push(function_node);

            if let Some(parent_scope) = parent_scope.as_ref() {
                if parent_scope.kind == ScopeKind::Class {
                    push_edge(state, &parent_scope.id, &function_id, "defines");
                }
            }

            let next_scope = Some(ScopeContext {
                id: function_id,
                kind: ScopeKind::Function,
            });

            if let Some(body) = node.child_by_field_name("body") {
                walk_scope(body, source_code, file_path, state, next_scope, depth + 1)?;
            }

            Ok(())
        }
        _ => {
            let mut cursor = node.walk();
            for child in node.named_children(&mut cursor) {
                walk_scope(
                    child,
                    source_code,
                    file_path,
                    state,
                    parent_scope.clone(),
                    depth + 1,
                )?;
            }

            Ok(())
        }
    }
}

fn extract_imports(
    conn: &Connection,
    root: Node,
    source_code: &str,
    file_path: &str,
    file_node_id: &str,
    state: &mut ExtractionState,
) -> anyhow::Result<()> {
    let language = Language::new(LANGUAGE);
    let query =
        Query::new(&language, IMPORT_QUERY_STR).context("failed to compile Python import query")?;

    let capture_indexes = ImportCaptureIndexes::new(&query)?;
    let mut cursor = QueryCursor::new();
    let mut matches = cursor.matches(&query, root, source_code.as_bytes());

    while let Some(query_match) = matches.next() {
        let module_name = if let Some(import_name) =
            capture_text(&query_match, capture_indexes.import_name, source_code)
        {
            import_name.to_string()
        } else if let Some(from_module) =
            capture_text(&query_match, capture_indexes.from_module, source_code)
        {
            if has_capture(&query_match, capture_indexes.from_wildcard) {
                format!("{from_module}.*")
            } else if let Some(imported_name) =
                capture_text(&query_match, capture_indexes.from_name, source_code)
            {
                format!("{from_module}.{imported_name}")
            } else {
                continue;
            }
        } else {
            continue;
        };

        let module_id = resolve_module_id(conn, state, &module_name)?;
        push_edge(state, file_node_id, &module_id, "imports");
    }

    let _ = file_path;
    Ok(())
}

fn resolve_module_id(
    conn: &Connection,
    state: &mut ExtractionState,
    module_name: &str,
) -> anyhow::Result<String> {
    if let Some(existing_id) = state.module_ids.get(module_name) {
        return Ok(existing_id.clone());
    }

    let existing_db_id = conn
        .query_row(
            "SELECT id FROM nodes WHERE kind = 'module' AND name = ?1 LIMIT 1",
            [module_name],
            |row| row.get::<_, String>(0),
        )
        .optional()?;

    if let Some(existing_db_id) = existing_db_id {
        state
            .module_ids
            .insert(module_name.to_string(), existing_db_id.clone());
        return Ok(existing_db_id);
    }

    let module_id = Uuid::new_v4().to_string();
    state.graph.nodes.push(PendingNode {
        id: module_id.clone(),
        kind: "module".to_string(),
        name: module_name.to_string(),
        file_path: MODULE_FILE_PATH.to_string(),
        start_line: None,
        end_line: None,
        content_hash: None,
    });
    state
        .module_ids
        .insert(module_name.to_string(), module_id.clone());

    Ok(module_id)
}

fn push_edge(state: &mut ExtractionState, source_id: &str, target_id: &str, relation: &str) {
    let edge_key = (
        source_id.to_string(),
        target_id.to_string(),
        relation.to_string(),
    );

    if state.edge_keys.insert(edge_key) {
        state.graph.edges.push(PendingEdge {
            source_id: source_id.to_string(),
            target_id: target_id.to_string(),
            relation: relation.to_string(),
        });
    }
}

fn build_symbol_node(kind: &str, name: &str, file_path: &str, node: Node) -> PendingNode {
    PendingNode {
        id: Uuid::new_v4().to_string(),
        kind: kind.to_string(),
        name: name.to_string(),
        file_path: file_path.to_string(),
        start_line: Some(line_number(node.start_position().row)),
        end_line: Some(line_number(node.end_position().row)),
        content_hash: None,
    }
}

fn child_field_text<'a>(
    node: Node,
    field_name: &str,
    source_code: &'a str,
) -> anyhow::Result<Option<&'a str>> {
    let Some(child) = node.child_by_field_name(field_name) else {
        return Ok(None);
    };

    child
        .utf8_text(source_code.as_bytes())
        .map(Some)
        .map_err(|err| anyhow!("invalid utf-8 in {field_name} field: {err}"))
}

fn relative_file_path(path: &Path, project_root: &Path) -> anyhow::Result<String> {
    let relative_path = path.strip_prefix(project_root).with_context(|| {
        format!(
            "failed to compute relative path for {} against {}",
            path.display(),
            project_root.display()
        )
    })?;

    let relative = relative_path.to_string_lossy().replace('\\', "/");
    if relative.is_empty() {
        bail!("computed empty relative file path for {}", path.display());
    }

    Ok(relative)
}

fn line_number(row: usize) -> i64 {
    row as i64 + 1
}

fn capture_text<'a>(
    query_match: &'a tree_sitter::QueryMatch<'a, 'a>,
    capture_index: u32,
    source_code: &'a str,
) -> Option<&'a str> {
    query_match
        .nodes_for_capture_index(capture_index)
        .next()
        .and_then(|node| node.utf8_text(source_code.as_bytes()).ok())
}

fn has_capture(query_match: &tree_sitter::QueryMatch<'_, '_>, capture_index: u32) -> bool {
    query_match
        .nodes_for_capture_index(capture_index)
        .next()
        .is_some()
}

struct ImportCaptureIndexes {
    import_name: u32,
    from_module: u32,
    from_name: u32,
    from_wildcard: u32,
}

impl ImportCaptureIndexes {
    fn new(query: &Query) -> anyhow::Result<Self> {
        Ok(Self {
            import_name: query
                .capture_index_for_name("import.name")
                .ok_or_else(|| anyhow!("missing @import.name capture"))?,
            from_module: query
                .capture_index_for_name("import.from.module")
                .ok_or_else(|| anyhow!("missing @import.from.module capture"))?,
            from_name: query
                .capture_index_for_name("import.from.name")
                .ok_or_else(|| anyhow!("missing @import.from.name capture"))?,
            from_wildcard: query
                .capture_index_for_name("import.from.wildcard")
                .ok_or_else(|| anyhow!("missing @import.from.wildcard capture"))?,
        })
    }
}
