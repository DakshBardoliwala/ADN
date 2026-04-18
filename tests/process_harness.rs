mod support;

use rusqlite::Connection;
use serde_json::Value;

use support::{assert_command_success, TestWorkspace};

#[test]
fn cli_search_runs_in_isolated_temp_workspace() {
    let workspace = TestWorkspace::new();
    workspace.index_fixture_ok("mcp_sample");

    let output = workspace.run_cli(&["search", "helper", "--json"]);
    assert_command_success("search command failed", &output);

    let payload = serde_json::from_slice::<Vec<Value>>(&output.stdout)
        .expect("search output should be valid json");

    assert!(
        payload
            .iter()
            .any(|node| node["name"] == "helper" && node["file_path"] == "helpers.py"),
        "expected helper symbol in search results: {payload:?}"
    );

    assert!(
        payload.iter().all(|node| node["indexed_at"]
            .as_str()
            .is_some_and(|value| !value.is_empty())),
        "expected indexed_at on all search results: {payload:?}"
    );
}

#[test]
fn indexing_populates_indexed_at_for_persisted_nodes() {
    let workspace = TestWorkspace::new();
    workspace.index_fixture_ok("mcp_sample");

    let conn = Connection::open(workspace.path().join("adn.db")).expect("database should open");
    let indexed_count = conn
        .query_row(
            "SELECT COUNT(*)
             FROM nodes
             WHERE indexed_at IS NOT NULL AND indexed_at != ''",
            [],
            |row| row.get::<_, i64>(0),
        )
        .expect("count query should succeed");

    assert!(indexed_count > 0, "expected indexed nodes with timestamps");
}

#[test]
fn init_db_migrates_existing_nodes_table_with_indexed_at() {
    let workspace = TestWorkspace::new();
    let conn = Connection::open(workspace.path().join("adn.db")).expect("database should open");

    conn.execute_batch(
        "
        CREATE TABLE nodes (
            id TEXT PRIMARY KEY,
            kind TEXT NOT NULL,
            name TEXT NOT NULL,
            file_path TEXT NOT NULL,
            start_line INTEGER,
            end_line INTEGER,
            content_hash TEXT
        );
        CREATE TABLE edges (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            source_id TEXT NOT NULL,
            target_id TEXT NOT NULL,
            relation TEXT NOT NULL
        );
        INSERT INTO nodes (id, kind, name, file_path, start_line, end_line, content_hash)
        VALUES ('legacy-id', 'function', 'legacy_symbol', 'legacy.py', 1, 1, NULL);
        ",
    )
    .expect("legacy schema should be created");
    drop(conn);

    let output = workspace.run_cli(&["search", "legacy_symbol", "--json"]);
    assert_command_success("search command failed after migration", &output);

    let payload = serde_json::from_slice::<Vec<Value>>(&output.stdout)
        .expect("search output should be valid json");
    assert_eq!(payload.len(), 1, "expected one legacy search result");
    assert_eq!(payload[0]["name"], "legacy_symbol");
    assert!(
        payload[0]["indexed_at"]
            .as_str()
            .is_some_and(|value| !value.is_empty()),
        "expected migrated node to have indexed_at: {payload:?}"
    );
}

#[test]
fn cli_search_supports_limit_offset_and_local_flag() {
    let workspace = TestWorkspace::new();
    workspace.index_fixture_ok("query_sample");

    let paged_output = workspace.run_cli(&[
        "search", "alpha_", "--limit", "1", "--offset", "1", "--json",
    ]);
    assert_command_success("paged search command failed", &paged_output);

    let paged_payload = serde_json::from_slice::<Vec<Value>>(&paged_output.stdout)
        .expect("search output should be valid json");
    assert_eq!(paged_payload.len(), 1);
    assert_eq!(paged_payload[0]["name"], "alpha_b");

    let local_output = workspace.run_cli(&["search", "requests", "--local", "--json"]);
    assert_command_success("local-only search command failed", &local_output);

    let local_payload = serde_json::from_slice::<Vec<Value>>(&local_output.stdout)
        .expect("search output should be valid json");
    assert!(
        local_payload.is_empty(),
        "expected external module to be filtered"
    );
}

#[test]
fn cli_inspect_and_trace_support_name_based_lookup() {
    let workspace = TestWorkspace::new();
    workspace.index_fixture_ok("query_sample");

    let inspect_output = workspace.run_cli(&[
        "inspect",
        "--name",
        "duplicate",
        "--file",
        "core.py",
        "--json",
    ]);
    assert_command_success("inspect command failed", &inspect_output);

    let inspect_payload = serde_json::from_slice::<Value>(&inspect_output.stdout)
        .expect("inspect output should be valid json");
    assert_eq!(inspect_payload["node"]["name"], "duplicate");
    assert_eq!(inspect_payload["node"]["start_line"], 5);
    assert_eq!(inspect_payload["ambiguous"], true);

    let search_output = workspace.run_cli(&["search", "target", "--json"]);
    assert_command_success("search command failed", &search_output);
    let search_payload = serde_json::from_slice::<Vec<Value>>(&search_output.stdout)
        .expect("search output should be valid json");
    let target_id = search_payload
        .iter()
        .find(|node| node["name"] == "target" && node["file_path"] == "core.py")
        .and_then(|node| node["id"].as_str())
        .expect("target node should be indexed");

    let trace_by_id = workspace.run_cli(&["trace", target_id, "--depth", "2", "--json"]);
    assert_command_success("trace by id command failed", &trace_by_id);
    let trace_by_name = workspace.run_cli(&[
        "trace", "--name", "target", "--file", "core.py", "--depth", "2", "--json",
    ]);
    assert_command_success("trace by name command failed", &trace_by_name);

    let trace_by_id_payload = serde_json::from_slice::<Value>(&trace_by_id.stdout)
        .expect("trace output should be valid json");
    let trace_by_name_payload = serde_json::from_slice::<Value>(&trace_by_name.stdout)
        .expect("trace output should be valid json");

    assert_eq!(
        trace_by_name_payload["target"]["id"],
        trace_by_id_payload["target"]["id"]
    );
    assert_eq!(
        trace_by_name_payload["children"],
        trace_by_id_payload["children"]
    );
}

#[test]
fn cli_stats_reports_indexed_files_and_counts() {
    let workspace = TestWorkspace::new();
    workspace.index_fixture_ok("query_sample");

    let json_output = workspace.run_cli(&["stats", "--json"]);
    assert_command_success("stats json command failed", &json_output);

    let payload = serde_json::from_slice::<Value>(&json_output.stdout)
        .expect("stats output should be valid json");
    assert!(payload["files"].is_array());
    assert!(payload["files"]
        .as_array()
        .is_some_and(|files| files.iter().any(|entry| entry["file_path"] == "core.py")));
    assert_eq!(payload["stats"]["external_modules"], 1);

    let text_output = workspace.run_cli(&["stats"]);
    assert_command_success("stats text command failed", &text_output);
    let text = String::from_utf8(text_output.stdout).expect("stats output should be utf-8");
    assert!(text.contains("File Path"));
    assert!(text.contains("core.py"));
    assert!(text.contains("Local Symbols:"));
    assert!(text.contains("External Modules:"));
}
