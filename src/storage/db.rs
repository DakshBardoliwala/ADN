use rusqlite::Connection;

use crate::models::ParsedFileGraph;
use crate::storage::schema;

pub fn init_db() -> anyhow::Result<Connection> {
    // Keep the DB in the ADN workspace (current working directory) so that
    // indexing an arbitrary target directory never requires write access there.
    let conn = Connection::open("adn.db")?;

    conn.execute_batch(schema::CREATE_TABLES)?;
    
    Ok(conn)
}

pub fn persist_file_graph(conn: &mut Connection, graph: &ParsedFileGraph) -> anyhow::Result<()> {
    let tx = conn.transaction()?;

    {
        let mut insert_node = tx.prepare(
            "INSERT INTO nodes (id, kind, name, file_path, start_line, end_line, content_hash)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
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
            ))?;
        }
    }

    {
        let mut insert_edge = tx.prepare(
            "INSERT INTO edges (source_id, target_id, relation) VALUES (?1, ?2, ?3)",
        )?;

        for edge in &graph.edges {
            insert_edge.execute((&edge.source_id, &edge.target_id, &edge.relation))?;
        }
    }

    tx.commit()?;

    Ok(())
}
