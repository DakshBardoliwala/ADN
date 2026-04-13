use std::collections::HashSet;
use std::fs;
use std::path::{Component, Path};

use anyhow::{anyhow, bail, Context};
use rusqlite::{Connection, OptionalExtension};
use tree_sitter::{Language, Node, Parser, Query, QueryCursor, StreamingIterator, Tree};
use tree_sitter_python::LANGUAGE;
use uuid::Uuid;

use crate::indexer::languages::python::IMPORT_QUERY_STR;
use crate::models::{DeferredImport, ParsedFileGraph, PendingEdge, PendingNode};
use crate::storage::db;

const MAX_SCOPE_DEPTH: usize = 1024;
const MODULE_FILE_PATH: &str = "<module>";
const LOCAL_SYMBOL_KINDS: [&str; 2] = ["class", "function"];

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
    edge_keys: HashSet<(String, String, String)>,
    deferred_imports: Vec<DeferredImport>,
}

pub fn parse_file(
    path: &Path,
    project_root: &Path,
    conn: &mut Connection,
) -> anyhow::Result<Vec<DeferredImport>> {
    let source_code = fs::read_to_string(path)?;
    let relative_file_path = relative_file_path(path, project_root)?;
    let content_hash = blake3::hash(source_code.as_bytes()).to_hex().to_string();

    if db::get_file_content_hash(conn, &relative_file_path)?.as_deref() == Some(&content_hash) {
        return Ok(Vec::new());
    }

    db::delete_file_graph(conn, &relative_file_path)?;

    let tree = parse_python_tree(&source_code)?;
    let (graph, deferred_imports) =
        extract_symbols_and_edges(&relative_file_path, conn, &source_code, &tree, content_hash)?;

    db::persist_file_graph(conn, &graph)?;

    Ok(deferred_imports)
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
) -> anyhow::Result<(ParsedFileGraph, Vec<DeferredImport>)> {
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

    Ok((state.graph, state.deferred_imports))
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
        let (module_name, imported_name, is_wildcard) = if let Some(import_name) =
            capture_text(query_match, capture_indexes.import_name, source_code)
        {
            (import_name.to_string(), None, false)
        } else if let Some(from_module) =
            capture_text(query_match, capture_indexes.from_module, source_code)
        {
            if has_capture(query_match, capture_indexes.from_wildcard) {
                (from_module.to_string(), None, true)
            } else if let Some(imported_name) =
                capture_text(query_match, capture_indexes.from_name, source_code)
            {
                (
                    from_module.to_string(),
                    Some(imported_name.to_string()),
                    false,
                )
            } else {
                continue;
            }
        } else {
            continue;
        };

        if let Some(target_id) = resolve_local_import_target(
            conn,
            file_path,
            &module_name,
            imported_name.as_deref(),
            is_wildcard,
        )? {
            push_edge(state, file_node_id, &target_id, "imports");
        } else {
            state.deferred_imports.push(DeferredImport {
                source_id: file_node_id.to_string(),
                source_file_path: file_path.to_string(),
                module_name,
                imported_name,
                is_wildcard,
            });
        }
    }

    Ok(())
}

pub fn resolve_deferred_imports(
    conn: &mut Connection,
    deferred_imports: &[DeferredImport],
) -> anyhow::Result<()> {
    let mut edge_keys = HashSet::new();

    for deferred in deferred_imports {
        let target_id = if let Some(target_id) = resolve_local_import_target(
            conn,
            &deferred.source_file_path,
            &deferred.module_name,
            deferred.imported_name.as_deref(),
            deferred.is_wildcard,
        )? {
            target_id
        } else {
            get_or_create_synthetic_module_id(
                conn,
                &synthetic_module_name(&deferred.module_name, deferred.imported_name.as_deref()),
            )?
        };

        let edge_key = (
            deferred.source_id.clone(),
            target_id.clone(),
            "imports".to_string(),
        );

        if edge_keys.insert(edge_key) {
            db::insert_edge(conn, &deferred.source_id, &target_id, "imports")?;
        }
    }

    Ok(())
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

fn resolve_local_import_target(
    conn: &Connection,
    source_file_path: &str,
    module_name: &str,
    imported_name: Option<&str>,
    is_wildcard: bool,
) -> anyhow::Result<Option<String>> {
    let Some(resolved_module_name) = resolve_module_name(source_file_path, module_name) else {
        return Ok(None);
    };

    let candidate_paths = local_module_candidate_paths(&resolved_module_name);

    if imported_name.is_none() || is_wildcard {
        for candidate_path in &candidate_paths {
            if let Some(file_id) = find_file_node_id(conn, candidate_path)? {
                return Ok(Some(file_id));
            }
        }

        return Ok(None);
    }

    let imported_name = imported_name.unwrap_or_default();
    for candidate_path in &candidate_paths {
        if find_file_node_id(conn, candidate_path)?.is_some() {
            if let Some(symbol_id) = find_symbol_id_in_file(conn, candidate_path, imported_name)? {
                return Ok(Some(symbol_id));
            }
        }
    }

    Ok(None)
}

fn resolve_module_name(source_file_path: &str, module_name: &str) -> Option<String> {
    if let Some(stripped) = module_name.strip_prefix('.') {
        return resolve_relative_module_name(source_file_path, stripped, module_name);
    }

    Some(module_name.to_string())
}

fn resolve_relative_module_name(
    source_file_path: &str,
    stripped_module_name: &str,
    full_module_name: &str,
) -> Option<String> {
    let leading_dots = full_module_name
        .chars()
        .take_while(|character| *character == '.')
        .count();

    if leading_dots == 0 {
        return Some(full_module_name.to_string());
    }

    let mut package_parts = Path::new(source_file_path)
        .parent()
        .map(|path| {
            path.components()
                .filter_map(component_to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let levels_up = leading_dots.saturating_sub(1);
    if levels_up > package_parts.len() {
        return None;
    }

    for _ in 0..levels_up {
        package_parts.pop();
    }

    if !stripped_module_name.is_empty() {
        package_parts.extend(
            stripped_module_name
                .split('.')
                .filter(|segment| !segment.is_empty())
                .map(ToString::to_string),
        );
    }

    if package_parts.is_empty() {
        return None;
    }

    Some(package_parts.join("."))
}

fn local_module_candidate_paths(module_name: &str) -> Vec<String> {
    let normalized = module_name.replace('.', "/");

    vec![
        format!("{normalized}.py"),
        format!("{normalized}/__init__.py"),
    ]
}

fn component_to_string(component: Component<'_>) -> Option<String> {
    match component {
        Component::Normal(value) => Some(value.to_string_lossy().to_string()),
        _ => None,
    }
}

fn find_file_node_id(conn: &Connection, file_path: &str) -> anyhow::Result<Option<String>> {
    conn.query_row(
        "SELECT id
         FROM nodes
         WHERE kind = 'file' AND file_path = ?1
         LIMIT 1",
        [file_path],
        |row| row.get::<_, String>(0),
    )
    .optional()
    .map_err(Into::into)
}

fn find_symbol_id_in_file(
    conn: &Connection,
    file_path: &str,
    symbol_name: &str,
) -> anyhow::Result<Option<String>> {
    conn.query_row(
        "SELECT id
         FROM nodes
         WHERE file_path = ?1
           AND name = ?2
           AND kind IN (?3, ?4)
         ORDER BY start_line ASC
         LIMIT 1",
        [
            file_path,
            symbol_name,
            LOCAL_SYMBOL_KINDS[0],
            LOCAL_SYMBOL_KINDS[1],
        ],
        |row| row.get::<_, String>(0),
    )
    .optional()
    .map_err(Into::into)
}

fn get_or_create_synthetic_module_id(
    conn: &mut Connection,
    module_name: &str,
) -> anyhow::Result<String> {
    let existing_id = conn
        .query_row(
            "SELECT id
             FROM nodes
             WHERE kind = 'module' AND name = ?1 AND file_path = ?2
             LIMIT 1",
            [module_name, MODULE_FILE_PATH],
            |row| row.get::<_, String>(0),
        )
        .optional()?;

    if let Some(existing_id) = existing_id {
        return Ok(existing_id);
    }

    let module_id = Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO nodes (id, kind, name, file_path, start_line, end_line, content_hash)
         VALUES (?1, 'module', ?2, ?3, NULL, NULL, NULL)",
        [&module_id, module_name, MODULE_FILE_PATH],
    )?;

    Ok(module_id)
}

fn synthetic_module_name(module_name: &str, imported_name: Option<&str>) -> String {
    match imported_name {
        Some(imported_name) => format!("{module_name}.{imported_name}"),
        None => module_name.to_string(),
    }
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
