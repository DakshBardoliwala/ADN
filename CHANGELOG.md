# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2026-04-12

### Added
- **Core Indexer:** Recursive AST traversal using Tree-sitter for Python.
- **Symbol Resolution:** Two-pass indexing to resolve `from ... import` statements across file boundaries.
- **Incremental Indexing:** Blake3-based file hashing to skip unchanged files and reduce I/O.
- **Knowledge Graph:** SQLite storage schema for nodes (files, classes, functions) and edges (defines, imports, calls).
- **CLI Suite:** `search`, `inspect`, `ls`, and `trace` commands for human-friendly graph exploration.
- **MCP Server:** Initial Model Context Protocol support via `stdio` transport, enabling AI agent tool use.
- **Impact Analysis:** Recursive CTE-based `trace` command to calculate code "blast radius."

### Changed
- Refactored `src/models.rs` to use dedicated Read-Side (`StoredNode`) and Write-Side (`PendingNode`) models.
- Migrated from absolute system paths to repo-relative paths for cross-environment database stability.

---

## [Unreleased]

### Planned (v0.2.0 "The Ergonomics Update")
- [ ] Pagination (`limit`/`offset`) for `search_codebase` to prevent context overflow.
- [ ] Multi-identifier lookup (Name + File Path) for `get_node_details`.
- [ ] Configurable recursion depth for the `trace_impact` tool.
- [ ] Discovery tools (`list_indexed_files`) to help agents navigate "cold" repositories.
- [ ] Multi-language support starting with Rust (`tree-sitter-rust`).