use rusqlite::{params, Connection};

use crate::models::{NodeDetails, StoredEdge, StoredNode};

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
