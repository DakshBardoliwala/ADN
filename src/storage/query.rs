use std::collections::HashMap;

use rusqlite::{params, Connection};

use crate::models::{NodeDetails, StoredEdge, StoredNode, TraceResult, TraceTreeNode};

pub fn search_symbols(conn: &Connection, query: &str) -> anyhow::Result<Vec<StoredNode>> {
    let pattern = format!("%{}%", query.trim());
    let mut stmt = conn.prepare(
        "SELECT id, kind, name, file_path, start_line, end_line, content_hash
         FROM nodes
         WHERE LOWER(name) LIKE LOWER(?1)
         ORDER BY kind ASC, name ASC, file_path ASC, start_line ASC",
    )?;

    let rows = stmt.query_map([pattern], map_node_row)?;

    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

pub fn get_node_details(conn: &Connection, id: &str) -> anyhow::Result<Option<NodeDetails>> {
    let mut node_stmt = conn.prepare(
        "SELECT id, kind, name, file_path, start_line, end_line, content_hash
         FROM nodes
         WHERE id = ?1",
    )?;

    let mut node_rows = node_stmt.query([id])?;
    let Some(row) = node_rows.next()? else {
        return Ok(None);
    };

    let node = map_node(row)?;
    let outgoing = get_edges_by_column(conn, "source_id", id)?;
    let incoming = get_edges_by_column(conn, "target_id", id)?;

    Ok(Some(NodeDetails {
        node,
        outgoing,
        incoming,
    }))
}

pub fn get_file_symbols(conn: &Connection, path: &str) -> anyhow::Result<Vec<StoredNode>> {
    let mut stmt = conn.prepare(
        "SELECT id, kind, name, file_path, start_line, end_line, content_hash
         FROM nodes
         WHERE file_path = ?1
         ORDER BY start_line ASC, end_line ASC, name ASC",
    )?;

    let rows = stmt.query_map([path], map_node_row)?;

    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

pub fn trace_impact(conn: &Connection, id: &str) -> anyhow::Result<Option<TraceResult>> {
    let target = get_node_by_id(conn, id)?;
    let Some(target) = target else {
        return Ok(None);
    };

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
             nodes.content_hash
         FROM impact
         JOIN nodes ON nodes.id = impact.node_id
         ORDER BY impact.depth ASC, impact.child_id ASC, impact.relation ASC, nodes.file_path ASC,
                  nodes.start_line ASC, nodes.name ASC",
    )?;

    let rows = stmt.query_map(params![id, 5_i64], |row| {
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
        target: target.clone(),
        max_depth,
        children: build_trace_children(&target.id, &rows_by_child),
    }))
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
        "SELECT id, kind, name, file_path, start_line, end_line, content_hash
         FROM nodes
         WHERE id = ?1",
    )?;

    let mut rows = stmt.query([id])?;
    let Some(row) = rows.next()? else {
        return Ok(None);
    };

    Ok(Some(map_node(row)?))
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
    })
}

#[derive(Debug, Clone)]
struct FlatTraceRow {
    node_id: String,
    child_id: String,
    relation: String,
    depth: i64,
    node: StoredNode,
}
