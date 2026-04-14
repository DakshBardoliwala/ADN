use serde_json::{json, Value};
mod support;

use support::TestWorkspace;

fn search_for_symbol(workspace: &TestWorkspace, query: &str) -> Vec<Value> {
    let session = workspace.run_mcp_session(
        &[
            json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}),
            json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
            json!({
                "jsonrpc": "2.0",
                "id": 2,
                "method": "tools/call",
                "params": {
                    "name": "search_codebase",
                    "arguments": {
                        "query": query
                    }
                }
            }),
        ],
    );

    assert!(
        session.stderr.is_empty(),
        "expected no stderr for successful session, got:\n{}",
        session.stderr
    );

    let content = session.responses[1]["result"]["content"][0]["text"]
        .as_str()
        .expect("tool result should include text content");

    serde_json::from_str::<Vec<Value>>(content).expect("tool text should contain json array")
}

#[test]
fn initialize_returns_jsonrpc_2_and_matching_id() {
    let workspace = TestWorkspace::new();
    workspace.index_fixture_ok("mcp_sample");

    let session = workspace.run_mcp_session(
        &[json!({"jsonrpc": "2.0", "id": 41, "method": "initialize", "params": {}})],
    );

    assert!(session.stderr.is_empty(), "unexpected stderr:\n{}", session.stderr);
    assert_eq!(session.responses.len(), 1);
    assert_eq!(session.responses[0]["jsonrpc"], "2.0");
    assert_eq!(session.responses[0]["id"], 41);
    assert_eq!(session.responses[0]["result"]["serverInfo"]["name"], "adn");
}

#[test]
fn tools_list_is_rejected_before_initialized_notification() {
    let workspace = TestWorkspace::new();
    workspace.index_fixture_ok("mcp_sample");

    let session = workspace.run_mcp_session(
        &[
            json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}),
            json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}),
        ],
    );

    assert!(session.stderr.is_empty(), "unexpected stderr:\n{}", session.stderr);
    assert_eq!(session.responses.len(), 2);
    assert_eq!(session.responses[1]["jsonrpc"], "2.0");
    assert_eq!(session.responses[1]["id"], 2);
    assert_eq!(session.responses[1]["error"]["code"], -32002);
    assert_eq!(
        session.responses[1]["error"]["message"],
        "Server handshake incomplete"
    );
}

#[test]
fn tools_list_succeeds_after_handshake() {
    let workspace = TestWorkspace::new();
    workspace.index_fixture_ok("mcp_sample");

    let session = workspace.run_mcp_session(
        &[
            json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}),
            json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
            json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}),
        ],
    );

    assert!(session.stderr.is_empty(), "unexpected stderr:\n{}", session.stderr);
    assert_eq!(session.responses.len(), 2);
    assert_eq!(session.responses[1]["jsonrpc"], "2.0");
    assert_eq!(session.responses[1]["id"], 2);

    let tools = session.responses[1]["result"]["tools"]
        .as_array()
        .expect("tools/list should return an array");
    let names = tools
        .iter()
        .filter_map(|tool| tool["name"].as_str())
        .collect::<Vec<_>>();

    assert!(names.contains(&"search_codebase"));
    assert!(names.contains(&"get_node_details"));
    assert!(names.contains(&"list_file_symbols"));
    assert!(names.contains(&"trace_impact"));
}

#[test]
fn tools_call_returns_text_content_and_expected_query_results() {
    let workspace = TestWorkspace::new();
    workspace.index_fixture_ok("mcp_sample");

    let session = workspace.run_mcp_session(
        &[
            json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}),
            json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
            json!({
                "jsonrpc": "2.0",
                "id": 2,
                "method": "tools/call",
                "params": {
                    "name": "search_codebase",
                    "arguments": {
                        "query": "helper"
                    }
                }
            }),
        ],
    );

    assert!(session.stderr.is_empty(), "unexpected stderr:\n{}", session.stderr);
    assert_eq!(session.responses.len(), 2);
    assert_eq!(session.responses[1]["jsonrpc"], "2.0");
    assert_eq!(session.responses[1]["id"], 2);
    assert_eq!(session.responses[1]["result"]["content"][0]["type"], "text");

    let content = session.responses[1]["result"]["content"][0]["text"]
        .as_str()
        .expect("tool result should include text payload");
    let payload = serde_json::from_str::<Vec<Value>>(content).expect("payload should be valid json");

    assert!(
        payload
            .iter()
            .any(|node| node["name"] == "helper" && node["file_path"] == "helpers.py"),
        "expected helper symbol in search results: {payload:?}"
    );
}

#[test]
fn invalid_tool_arguments_return_error_without_killing_server() {
    let workspace = TestWorkspace::new();
    workspace.index_fixture_ok("mcp_sample");

    let session = workspace.run_mcp_session(
        &[
            json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}),
            json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
            json!({
                "jsonrpc": "2.0",
                "id": 2,
                "method": "tools/call",
                "params": {
                    "name": "search_codebase",
                    "arguments": {}
                }
            }),
            json!({"jsonrpc": "2.0", "id": 3, "method": "tools/list"}),
        ],
    );

    assert!(session.stderr.is_empty(), "unexpected stderr:\n{}", session.stderr);
    assert_eq!(session.responses.len(), 3);
    assert_eq!(session.responses[1]["id"], 2);
    assert_eq!(session.responses[1]["error"]["code"], -32602);
    assert_eq!(
        session.responses[1]["error"]["message"],
        "Tool arguments do not match the expected schema"
    );
    assert_eq!(session.responses[2]["id"], 3);
    assert!(session.responses[2]["result"]["tools"].is_array());
}

#[test]
fn trace_impact_returns_text_content_for_imported_symbol() {
    let workspace = TestWorkspace::new();
    workspace.index_fixture_ok("mcp_sample");

    let helper_nodes = search_for_symbol(&workspace, "helper");
    let helper_id = helper_nodes
        .iter()
        .find(|node| node["name"] == "helper" && node["kind"] == "function")
        .and_then(|node| node["id"].as_str())
        .expect("helper function should be indexed")
        .to_string();

    let session = workspace.run_mcp_session(
        &[
            json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}),
            json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
            json!({
                "jsonrpc": "2.0",
                "id": 2,
                "method": "tools/call",
                "params": {
                    "name": "trace_impact",
                    "arguments": {
                        "id": helper_id
                    }
                }
            }),
        ],
    );

    assert!(session.stderr.is_empty(), "unexpected stderr:\n{}", session.stderr);
    assert_eq!(session.responses[1]["result"]["content"][0]["type"], "text");

    let content = session.responses[1]["result"]["content"][0]["text"]
        .as_str()
        .expect("trace result should include text payload");
    let trace = serde_json::from_str::<Value>(content).expect("trace content should be valid json");

    assert_eq!(trace["target"]["name"], "helper");
    assert!(
        trace["children"]
            .as_array()
            .expect("trace children should be an array")
            .iter()
            .any(|child| child["relation"] == "imports"),
        "expected imports edge in trace payload: {trace:?}"
    );
}
