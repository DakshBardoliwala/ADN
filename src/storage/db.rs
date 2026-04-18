use rusqlite::{params, Connection, OptionalExtension};

use crate::models::ParsedFileGraph;
use crate::storage::schema;

pub fn init_db() -> anyhow::Result<Connection> {
    // Keep the DB in the ADN workspace (current working directory) so that
    // indexing an arbitrary target directory never requires write access there.
    let conn = Connection::open("adn.db")?;

    conn.execute_batch(schema::CREATE_TABLES)?;
    ensure_nodes_indexed_at_column(&conn)?;

    Ok(conn)
}

pub fn persist_file_graph(conn: &mut Connection, graph: &ParsedFileGraph) -> anyhow::Result<()> {
    let tx = conn.transaction()?;

    {
        let mut insert_node = tx.prepare(
            "INSERT INTO nodes (id, kind, name, file_path, start_line, end_line, content_hash, indexed_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        )?;

        if let Some(file_node) = &graph.file_node {
            insert_node.execute((
                &file_node.id,
                &file_node.kind,
                &file_node.name,
                &file_node.file_path,
                file_node.start_line,
                file_node.end_line,
                &file_node.content_hash,
                &file_node.indexed_at,
            ))?;
        }

        for node in &graph.nodes {
            insert_node.execute((
                &node.id,
                &node.kind,
                &node.name,
                &node.file_path,
                node.start_line,
                node.end_line,
                &node.content_hash,
                &node.indexed_at,
            ))?;
        }
    }

    {
        let mut insert_edge = tx.prepare(
            "INSERT INTO edges (source_id, target_id, relation)
             SELECT ?1, ?2, ?3
             WHERE NOT EXISTS (
                 SELECT 1
                 FROM edges
                 WHERE source_id = ?1 AND target_id = ?2 AND relation = ?3
             )",
        )?;

        for edge in &graph.edges {
            insert_edge.execute((&edge.source_id, &edge.target_id, &edge.relation))?;
        }
    }

    tx.commit()?;

    Ok(())
}

pub fn current_timestamp(conn: &Connection) -> anyhow::Result<String> {
    conn.query_row("SELECT strftime('%Y-%m-%dT%H:%M:%fZ', 'now')", [], |row| {
        row.get(0)
    })
    .map_err(Into::into)
}

pub fn insert_edge(
    conn: &Connection,
    source_id: &str,
    target_id: &str,
    relation: &str,
) -> anyhow::Result<()> {
    conn.execute(
        "INSERT INTO edges (source_id, target_id, relation)
         SELECT ?1, ?2, ?3
         WHERE NOT EXISTS (
             SELECT 1
             FROM edges
             WHERE source_id = ?1 AND target_id = ?2 AND relation = ?3
         )",
        params![source_id, target_id, relation],
    )?;

    Ok(())
}

pub fn get_file_content_hash(conn: &Connection, file_path: &str) -> anyhow::Result<Option<String>> {
    conn.query_row(
        "SELECT content_hash
         FROM nodes
         WHERE file_path = ?1 AND kind = 'file'
         LIMIT 1",
        [file_path],
        |row| row.get::<_, Option<String>>(0),
    )
    .optional()
    .map(|value| value.flatten())
    .map_err(Into::into)
}

pub fn delete_file_graph(conn: &mut Connection, file_path: &str) -> anyhow::Result<()> {
    let tx = conn.transaction()?;

    tx.execute(
        "DELETE FROM edges
         WHERE source_id IN (
             SELECT id FROM nodes WHERE file_path = ?1
         )
         OR target_id IN (
             SELECT id FROM nodes WHERE file_path = ?1
         )",
        [file_path],
    )?;

    tx.execute(
        "DELETE FROM nodes
         WHERE file_path = ?1",
        params![file_path],
    )?;

    tx.commit()?;

    Ok(())
}

fn ensure_nodes_indexed_at_column(conn: &Connection) -> anyhow::Result<()> {
    let has_indexed_at = conn
        .prepare("PRAGMA table_info(nodes)")?
        .query_map([], |row| row.get::<_, String>(1))?
        .collect::<Result<Vec<_>, _>>()?
        .iter()
        .any(|column_name| column_name == "indexed_at");

    if !has_indexed_at {
        conn.execute("ALTER TABLE nodes ADD COLUMN indexed_at TEXT", [])?;
        conn.execute(
            "UPDATE nodes
             SET indexed_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             WHERE indexed_at IS NULL",
            [],
        )?;
    }

    Ok(())
}
