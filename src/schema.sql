-- Nodes represent the "entities" in your codebase
CREATE TABLE IF NOT EXISTS nodes (
    id TEXT PRIMARY KEY,
    kind TEXT NOT NULL, -- e.g., 'file', 'function', 'class'
    name TEXT NOT NULL,
    file_path TEXT NOT NULL,
    start_line INTEGER,
    start_column INTEGER,
    end_line INTEGER,
    end_column INTEGER,
    content_hash TEXT    -- To detect if we need to re-index
);

-- Edges represent the relationships/dependencies
CREATE TABLE IF NOT EXISTS edges (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    source_id TEXT NOT NULL,
    target_id TEXT NOT NULL,
    relation_type TEXT NOT NULL, -- e.g., 'defines', 'calls', 'references'
    FOREIGN KEY(source_id) REFERENCES nodes(id),
    FOREIGN KEY(target_id) REFERENCES nodes(id)
);

-- Indexing for fast RAG/Search
CREATE INDEX IF NOT EXISTS idx_node_name ON nodes(name);
CREATE INDEX IF NOT EXISTS idx_edge_source ON edges(source_id);