# ADN — MCP Improvement Suggestions

Observations from using the live MCP server against an indexed Python codebase.

---

## 1. Pagination on `search_codebase`
Broad queries (e.g. `"agent"`, `"chat"`) return thousands of results and overflow the MCP transport.

**Fix:** Add `limit` and `offset` parameters to `search_codebase`. A sensible default of `limit=50` would cover most use cases while keeping responses manageable.

---

## 2. Lookup by name + file, not just UUID
`get_node_details` and `trace_impact` require an exact node UUID. In practice this forces a two-step workflow: `search_codebase` to get the ID, then the actual call.

**Fix:** Accept either a UUID *or* a `(name, file_path)` pair as input to `get_node_details` and `trace_impact`.

---

## 3. Configurable depth on `trace_impact`
The trace result exposes `"max_depth": 2`, which is hardcoded. Deep dependency chains get silently truncated.

**Fix:** Add a `depth` parameter (default `2`, max perhaps `10`) so callers can control how far up the graph to walk.

---

## 4. Filter external modules from search results
`search_codebase` returns external dependency references (e.g. `fastapi.Query`, `gql.transport.exceptions.TransportQueryError`) alongside repo-local symbols. These pollute results for most use cases.

**Fix:** Add a `local_only: bool` parameter (default `true`) that excludes nodes with `file_path: "<module>"`.

---

## 5. Index discovery tool
There is no way to know what is indexed without already knowing what to search for. A cold-start AI agent has no entry point.

**Fix:** Add a `list_indexed_files` tool (or a `stats` tool) that returns the set of indexed file paths and a symbol count. This gives agents a map before they start querying.

---

## 6. Multi-language support (starting with Rust)
The indexer currently only parses `.py` files, which means ADN cannot be used on its own source code. Dogfooding would be a useful forcing function.

**Fix:** Add a `tree-sitter-rust` parser following the same pattern as `languages/python.rs`. Prioritise Rust since that is the project's own language.

---

## 7. Symbol-level content hashes
`content_hash` is populated on file nodes but is `null` on function/class nodes. This prevents callers from detecting whether a specific symbol changed between index runs.

**Fix:** Compute and store a blake3 hash of the symbol's source slice (start_line..end_line) during parsing, and persist it on the node row.
