pub const CREATE_TABLES: &str = "
    -- Represents a code entity (file, class, function)
    CREATE TABLE IF NOT EXISTS nodes (
        id TEXT PRIMARY KEY,
        kind TEXT NOT NULL,
        name TEXT NOT NULL,
        file_path TEXT NOT NULL,
        start_line INTEGER,
        end_line INTEGER,
        content_hash TEXT,
        indexed_at TEXT NOT NULL
    );

    -- Represents relationships between graph nodes.
    CREATE TABLE IF NOT EXISTS edges (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        source_id TEXT NOT NULL,
        target_id TEXT NOT NULL,
        relation TEXT NOT NULL,
        FOREIGN KEY(source_id) REFERENCES nodes(id),
        FOREIGN KEY(target_id) REFERENCES nodes(id)
    );

    CREATE INDEX IF NOT EXISTS idx_node_name ON nodes(name);
    CREATE INDEX IF NOT EXISTS idx_node_kind_name_path ON nodes(kind, name, file_path);
    CREATE INDEX IF NOT EXISTS idx_edge_source_relation ON edges(source_id, relation);
    CREATE INDEX IF NOT EXISTS idx_edge_target_relation ON edges(target_id, relation);
";
