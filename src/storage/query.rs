use std::collections::HashMap;

use rusqlite::{params, Connection};

use crate::models::{
    IndexedFileEntry, IndexedFilesResult, IndexedFilesStats, NodeDetails, NodeIdentifier,
    StoredEdge, StoredNode, TraceResult, TraceTreeNode,
};

const MODULE_FILE_PATH: &str = "<module>";
const DEFAULT_SEARCH_LIMIT: i64 = 50;
const DEFAULT_SEARCH_OFFSET: i64 = 0;
const DEFAULT_TRACE_DEPTH: i64 = 2;
const MAX_TRACE_DEPTH: i64 = 10;

#[derive(Debug, Clone)]
pub struct SearchOptions {
    pub limit: i64,
    pub offset: i64,
    pub local_only: bool,
}

impl Default for SearchOptions {
    fn default() -> Self {
        Self {
            limit: DEFAULT_SEARCH_LIMIT,
            offset: DEFAULT_SEARCH_OFFSET,
            local_only: true,
        }
    }
}

#[derive(Debug, Clone)]
pub enum NodeLookup {
    Id(String),
    Identifier(NodeIdentifier),
}

#[derive(Debug, Clone)]
pub struct TraceOptions {
    pub depth: i64,
}

impl Default for TraceOptions {
    fn default() -> Self {
        Self {
            depth: DEFAULT_TRACE_DEPTH,
        }
    }
}

pub fn search_symbols(
    conn: &Connection,
    query: &str,
    options: &SearchOptions,
) -> anyhow::Result<Vec<StoredNode>> {
    let pattern = format!("%{}%", query.trim());
    let mut stmt = conn.prepare(
        "SELECT id, kind, name, file_path, start_line, end_line, content_hash, indexed_at
         FROM nodes
         WHERE LOWER(name) LIKE LOWER(?1)
           AND (?2 = 0 OR file_path != ?3)
         ORDER BY kind ASC, name ASC, file_path ASC, start_line ASC
         LIMIT ?4 OFFSET ?5",
    )?;

    let rows = stmt.query_map(
        params![
            pattern,
            bool_to_sqlite_flag(options.local_only),
            MODULE_FILE_PATH,
            normalize_limit(options.limit),
            normalize_offset(options.offset)
        ],
        map_node_row,
    )?;

    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

pub fn get_node_details(
    conn: &Connection,
    lookup: &NodeLookup,
) -> anyhow::Result<Option<NodeDetails>> {
    let Some(resolved) = resolve_node_lookup(conn, lookup)? else {
        return Ok(None);
    };

    let outgoing = get_edges_by_column(conn, "source_id", &resolved.node.id)?;
    let incoming = get_edges_by_column(conn, "target_id", &resolved.node.id)?;

    Ok(Some(NodeDetails {
        node: resolved.node,
        ambiguous: resolved.ambiguous,
        outgoing,
        incoming,
    }))
}

pub fn get_file_symbols(conn: &Connection, path: &str) -> anyhow::Result<Vec<StoredNode>> {
    let mut stmt = conn.prepare(
        "SELECT id, kind, name, file_path, start_line, end_line, content_hash, indexed_at
         FROM nodes
         WHERE file_path = ?1
         ORDER BY start_line ASC, end_line ASC, name ASC",
    )?;

    let rows = stmt.query_map([path], map_node_row)?;

    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

pub fn trace_impact(
    conn: &Connection,
    lookup: &NodeLookup,
    options: &TraceOptions,
) -> anyhow::Result<Option<TraceResult>> {
    let Some(resolved) = resolve_node_lookup(conn, lookup)? else {
        return Ok(None);
    };
    let depth = normalize_trace_depth(options.depth);

    let mut stmt = conn.prepare(
        "WITH RECURSIVE impact(node_id, child_id, relation, depth, path) AS (
             SELECT
                 edges.source_id,
                 edges.target_id,
                 edges.relation,
                 1,
                 printf('|%s|%s|', edges.target_id, edges.source_id)
             FROM edges
             WHERE edges.target_id = ?1

             UNION ALL

             SELECT
                 edges.source_id,
                 edges.target_id,
                 edges.relation,
                 impact.depth + 1,
                 impact.path || edges.source_id || '|'
             FROM edges
             JOIN impact ON edges.target_id = impact.node_id
             WHERE impact.depth < ?2
               AND instr(impact.path, printf('|%s|', edges.source_id)) = 0
         )
         SELECT
             impact.node_id,
             impact.child_id,
             impact.relation,
             impact.depth,
             nodes.id,
             nodes.kind,
             nodes.name,
             nodes.file_path,
             nodes.start_line,
             nodes.end_line,
             nodes.content_hash,
             nodes.indexed_at
         FROM impact
         JOIN nodes ON nodes.id = impact.node_id
         ORDER BY impact.depth ASC, impact.child_id ASC, impact.relation ASC, nodes.file_path ASC,
                  nodes.start_line ASC, nodes.name ASC",
    )?;

    let rows = stmt.query_map(params![resolved.node.id, depth], |row| {
        Ok(FlatTraceRow {
            node_id: row.get(0)?,
            child_id: row.get(1)?,
            relation: row.get(2)?,
            depth: row.get(3)?,
            node: StoredNode {
                id: row.get(4)?,
                kind: row.get(5)?,
                name: row.get(6)?,
                file_path: row.get(7)?,
                start_line: row.get(8)?,
                end_line: row.get(9)?,
                content_hash: row.get(10)?,
                indexed_at: row.get(11)?,
            },
        })
    })?;

    let flat_rows = rows.collect::<Result<Vec<_>, _>>()?;
    let max_depth = flat_rows.iter().map(|row| row.depth).max().unwrap_or(0);
    let mut rows_by_child: HashMap<String, Vec<FlatTraceRow>> = HashMap::new();

    for row in flat_rows {
        rows_by_child
            .entry(row.child_id.clone())
            .or_default()
            .push(row);
    }

    Ok(Some(TraceResult {
        target: resolved.node.clone(),
        ambiguous: resolved.ambiguous,
        max_depth,
        children: build_trace_children(&resolved.node.id, &rows_by_child),
    }))
}

pub fn list_indexed_files(conn: &Connection) -> anyhow::Result<IndexedFilesResult> {
    let mut files_stmt = conn.prepare(
        "SELECT file_path, MAX(indexed_at) AS last_indexed
         FROM nodes
         WHERE file_path != ?1
         GROUP BY file_path
         ORDER BY file_path ASC",
    )?;
    let files = files_stmt
        .query_map([MODULE_FILE_PATH], |row| {
            Ok(IndexedFileEntry {
                file_path: row.get(0)?,
                last_indexed: row.get(1)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    let stats = conn.query_row(
        "SELECT
             SUM(CASE WHEN file_path != ?1 AND kind != 'file' THEN 1 ELSE 0 END),
             SUM(CASE WHEN file_path = ?1 THEN 1 ELSE 0 END)
         FROM nodes",
        [MODULE_FILE_PATH],
        |row| {
            Ok(IndexedFilesStats {
                local_symbols: row.get(0)?,
                external_modules: row.get(1)?,
            })
        },
    )?;

    Ok(IndexedFilesResult { files, stats })
}

fn get_edges_by_column(
    conn: &Connection,
    column: &str,
    node_id: &str,
) -> anyhow::Result<Vec<StoredEdge>> {
    let sql = format!(
        "SELECT source_id, target_id, relation
         FROM edges
         WHERE {column} = ?1
         ORDER BY relation ASC, source_id ASC, target_id ASC"
    );

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params![node_id], |row| {
        Ok(StoredEdge {
            source_id: row.get(0)?,
            target_id: row.get(1)?,
            relation: row.get(2)?,
        })
    })?;

    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

fn get_node_by_id(conn: &Connection, id: &str) -> anyhow::Result<Option<StoredNode>> {
    let mut stmt = conn.prepare(
        "SELECT id, kind, name, file_path, start_line, end_line, content_hash, indexed_at
         FROM nodes
         WHERE id = ?1",
    )?;

    let mut rows = stmt.query([id])?;
    let Some(row) = rows.next()? else {
        return Ok(None);
    };

    Ok(Some(map_node(row)?))
}

fn resolve_node_lookup(
    conn: &Connection,
    lookup: &NodeLookup,
) -> anyhow::Result<Option<ResolvedNode>> {
    match lookup {
        NodeLookup::Id(id) => Ok(get_node_by_id(conn, id)?.map(|node| ResolvedNode {
            node,
            ambiguous: false,
        })),
        NodeLookup::Identifier(identifier) => resolve_node_by_identifier(conn, identifier),
    }
}

fn resolve_node_by_identifier(
    conn: &Connection,
    identifier: &NodeIdentifier,
) -> anyhow::Result<Option<ResolvedNode>> {
    let mut stmt = conn.prepare(
        "SELECT
             id,
             kind,
             name,
             file_path,
             start_line,
             end_line,
             content_hash,
             indexed_at,
             (
                 SELECT COUNT(*)
                 FROM nodes AS match_nodes
                 WHERE match_nodes.name = ?1 AND match_nodes.file_path = ?2
             ) AS match_count
         FROM nodes
         WHERE name = ?1 AND file_path = ?2
         ORDER BY start_line IS NULL ASC, start_line ASC, end_line IS NULL ASC, end_line ASC, id ASC
         LIMIT 1",
    )?;

    let mut rows = stmt.query(params![identifier.name, identifier.file_path])?;
    let Some(row) = rows.next()? else {
        return Ok(None);
    };

    Ok(Some(ResolvedNode {
        node: map_node(row)?,
        ambiguous: row.get::<_, i64>(8)? > 1,
    }))
}

fn build_trace_children(
    child_id: &str,
    rows_by_child: &HashMap<String, Vec<FlatTraceRow>>,
) -> Vec<TraceTreeNode> {
    rows_by_child
        .get(child_id)
        .map(|rows| {
            rows.iter()
                .map(|row| TraceTreeNode {
                    node: row.node.clone(),
                    relation: row.relation.clone(),
                    depth: row.depth,
                    children: build_trace_children(&row.node_id, rows_by_child),
                })
                .collect()
        })
        .unwrap_or_default()
}

fn map_node_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<StoredNode> {
    map_node(row)
}

fn map_node(row: &rusqlite::Row<'_>) -> rusqlite::Result<StoredNode> {
    Ok(StoredNode {
        id: row.get(0)?,
        kind: row.get(1)?,
        name: row.get(2)?,
        file_path: row.get(3)?,
        start_line: row.get(4)?,
        end_line: row.get(5)?,
        content_hash: row.get(6)?,
        indexed_at: row.get(7)?,
    })
}

fn bool_to_sqlite_flag(value: bool) -> i64 {
    if value {
        1
    } else {
        0
    }
}

fn normalize_limit(limit: i64) -> i64 {
    limit.max(0)
}

fn normalize_offset(offset: i64) -> i64 {
    offset.max(0)
}

fn normalize_trace_depth(depth: i64) -> i64 {
    depth.clamp(0, MAX_TRACE_DEPTH).max(1)
}

#[derive(Debug, Clone)]
struct FlatTraceRow {
    node_id: String,
    child_id: String,
    relation: String,
    depth: i64,
    node: StoredNode,
}

#[derive(Debug, Clone)]
struct ResolvedNode {
    node: StoredNode,
    ambiguous: bool,
}
