mod support;

use serde_json::{json, Value};

use support::TestWorkspace;

fn tool_result_text(response: &Value) -> &str {
    response["result"]["content"][0]["text"]
        .as_str()
        .expect("tool result should include text content")
}

#[test]
fn search_codebase_supports_pagination_and_local_only_filter() {
    let workspace = TestWorkspace::new();
    workspace.index_fixture_ok("query_sample");

    let session = workspace.run_mcp_session(&[
        json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}),
        json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
        json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/call",
            "params": {
                "name": "search_codebase",
                "arguments": {
                    "query": "alpha_",
                    "limit": 1,
                    "offset": 1
                }
            }
        }),
        json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "tools/call",
            "params": {
                "name": "search_codebase",
                "arguments": {
                    "query": "requests"
                }
            }
        }),
        json!({
            "jsonrpc": "2.0",
            "id": 4,
            "method": "tools/call",
            "params": {
                "name": "search_codebase",
                "arguments": {
                    "query": "requests",
                    "local_only": false
                }
            }
        }),
    ]);

    assert!(
        session.stderr.is_empty(),
        "unexpected stderr:\n{}",
        session.stderr
    );

    let page = serde_json::from_str::<Vec<Value>>(tool_result_text(&session.responses[1]))
        .expect("paginated search should be valid json");
    assert_eq!(page.len(), 1);
    assert_eq!(page[0]["name"], "alpha_b");

    let local_only = serde_json::from_str::<Vec<Value>>(tool_result_text(&session.responses[2]))
        .expect("local-only search should be valid json");
    assert!(
        local_only.is_empty(),
        "expected external module to be hidden"
    );

    let include_external =
        serde_json::from_str::<Vec<Value>>(tool_result_text(&session.responses[3]))
            .expect("external search should be valid json");
    assert_eq!(include_external.len(), 1);
    assert_eq!(include_external[0]["file_path"], "<module>");
}

#[test]
fn get_node_details_accepts_identifier_and_reports_ambiguity() {
    let workspace = TestWorkspace::new();
    workspace.index_fixture_ok("query_sample");

    let session = workspace.run_mcp_session(&[
        json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}),
        json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
        json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/call",
            "params": {
                "name": "get_node_details",
                "arguments": {
                    "identifier": {
                        "name": "duplicate",
                        "file_path": "core.py"
                    }
                }
            }
        }),
    ]);

    assert!(
        session.stderr.is_empty(),
        "unexpected stderr:\n{}",
        session.stderr
    );

    let details = serde_json::from_str::<Value>(tool_result_text(&session.responses[1]))
        .expect("details should be valid json");
    assert_eq!(details["node"]["name"], "duplicate");
    assert_eq!(details["node"]["start_line"], 5);
    assert_eq!(details["ambiguous"], true);
}

#[test]
fn trace_impact_accepts_identifier_and_respects_depth() {
    let workspace = TestWorkspace::new();
    workspace.index_fixture_ok("query_sample");

    let session = workspace.run_mcp_session(&[
        json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}),
        json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
        json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/call",
            "params": {
                "name": "trace_impact",
                "arguments": {
                    "identifier": {
                        "name": "target",
                        "file_path": "core.py"
                    },
                    "depth": 1
                }
            }
        }),
        json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "tools/call",
            "params": {
                "name": "trace_impact",
                "arguments": {
                    "identifier": {
                        "name": "target",
                        "file_path": "core.py"
                    },
                    "depth": 2
                }
            }
        }),
    ]);

    assert!(
        session.stderr.is_empty(),
        "unexpected stderr:\n{}",
        session.stderr
    );

    let shallow = serde_json::from_str::<Value>(tool_result_text(&session.responses[1]))
        .expect("shallow trace should be valid json");
    assert_eq!(shallow["ambiguous"], false);
    assert_eq!(shallow["children"].as_array().map(Vec::len), Some(1));
    assert_eq!(shallow["children"][0]["node"]["file_path"], "mid.py");
    assert!(
        shallow["children"][0]["children"]
            .as_array()
            .expect("children should be an array")
            .is_empty(),
        "depth=1 should not recurse"
    );

    let deep = serde_json::from_str::<Value>(tool_result_text(&session.responses[2]))
        .expect("deep trace should be valid json");
    assert_eq!(deep["children"][0]["node"]["file_path"], "mid.py");
    assert_eq!(
        deep["children"][0]["children"][0]["node"]["file_path"],
        "top.py"
    );
}

#[test]
fn list_indexed_files_returns_paths_timestamps_and_stats() {
    let workspace = TestWorkspace::new();
    workspace.index_fixture_ok("query_sample");

    let session = workspace.run_mcp_session(&[
        json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}),
        json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
        json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/call",
            "params": {
                "name": "list_indexed_files",
                "arguments": {}
            }
        }),
    ]);

    assert!(
        session.stderr.is_empty(),
        "unexpected stderr:\n{}",
        session.stderr
    );

    let result = serde_json::from_str::<Value>(tool_result_text(&session.responses[1]))
        .expect("list_indexed_files should be valid json");
    let files = result["files"]
        .as_array()
        .expect("files should be an array");

    assert!(files.iter().all(|entry| entry["file_path"] != "<module>"));
    assert!(files.iter().any(|entry| entry["file_path"] == "core.py"));
    assert!(files.iter().all(|entry| {
        entry["last_indexed"]
            .as_str()
            .is_some_and(|value| !value.is_empty())
    }));
    assert!(
        result["stats"]["local_symbols"]
            .as_i64()
            .is_some_and(|count| count >= 5),
        "expected local symbol count in stats: {result:?}"
    );
    assert_eq!(result["stats"]["external_modules"], 1);
}
