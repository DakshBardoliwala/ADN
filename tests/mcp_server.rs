use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{json, Value};

fn adn_binary() -> &'static str {
    env!("CARGO_BIN_EXE_adn")
}

fn fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("mcp_sample")
}

fn unique_temp_dir() -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_nanos();

    std::env::temp_dir().join(format!("adn-mcp-test-{}-{nanos}", std::process::id()))
}

fn prepare_workdir() -> PathBuf {
    let workdir = unique_temp_dir();
    fs::create_dir_all(&workdir).expect("temporary workdir should be created");
    workdir
}

fn index_fixture(workdir: &Path) {
    let output = Command::new(adn_binary())
        .args(["index", fixture_root().to_str().expect("fixture path should be valid utf-8")])
        .current_dir(workdir)
        .output()
        .expect("index command should run");

    assert!(
        output.status.success(),
        "index command failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn run_mcp_session(workdir: &Path, messages: &[Value]) -> (Vec<Value>, String) {
    let mut child = Command::new(adn_binary())
        .args(["mcp", "serve"])
        .current_dir(workdir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("mcp server should start");

    {
        let stdin = child.stdin.as_mut().expect("stdin should be piped");
        for message in messages {
            serde_json::to_writer(&mut *stdin, message).expect("request should serialize");
            stdin.write_all(b"\n").expect("newline should be written");
            stdin.flush().expect("stdin should flush");
        }
    }

    let output = child
        .wait_with_output()
        .expect("mcp server should exit cleanly after stdin closes");

    assert!(
        output.status.success(),
        "mcp server failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout_text = String::from_utf8(output.stdout).expect("stdout should be utf-8");
    let responses = stdout_text
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str::<Value>(line).expect("response should be valid json"))
        .collect::<Vec<_>>();

    let stderr_text = String::from_utf8(output.stderr).expect("stderr should be utf-8");

    (responses, stderr_text)
}

fn search_for_symbol(workdir: &Path, query: &str) -> Vec<Value> {
    let (responses, stderr_text) = run_mcp_session(
        workdir,
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
        stderr_text.is_empty(),
        "expected no stderr for successful session, got:\n{stderr_text}"
    );

    let content = responses[1]["result"]["content"][0]["text"]
        .as_str()
        .expect("tool result should include text content");

    serde_json::from_str::<Vec<Value>>(content).expect("tool text should contain json array")
}

#[test]
fn initialize_returns_jsonrpc_2_and_matching_id() {
    let workdir = prepare_workdir();
    index_fixture(&workdir);

    let (responses, stderr_text) = run_mcp_session(
        &workdir,
        &[json!({"jsonrpc": "2.0", "id": 41, "method": "initialize", "params": {}})],
    );

    assert!(stderr_text.is_empty(), "unexpected stderr:\n{stderr_text}");
    assert_eq!(responses.len(), 1);
    assert_eq!(responses[0]["jsonrpc"], "2.0");
    assert_eq!(responses[0]["id"], 41);
    assert_eq!(responses[0]["result"]["serverInfo"]["name"], "adn");
}

#[test]
fn tools_list_is_rejected_before_initialized_notification() {
    let workdir = prepare_workdir();
    index_fixture(&workdir);

    let (responses, stderr_text) = run_mcp_session(
        &workdir,
        &[
            json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}),
            json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}),
        ],
    );

    assert!(stderr_text.is_empty(), "unexpected stderr:\n{stderr_text}");
    assert_eq!(responses.len(), 2);
    assert_eq!(responses[1]["jsonrpc"], "2.0");
    assert_eq!(responses[1]["id"], 2);
    assert_eq!(responses[1]["error"]["code"], -32002);
    assert_eq!(responses[1]["error"]["message"], "Server handshake incomplete");
}

#[test]
fn tools_list_succeeds_after_handshake() {
    let workdir = prepare_workdir();
    index_fixture(&workdir);

    let (responses, stderr_text) = run_mcp_session(
        &workdir,
        &[
            json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}),
            json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
            json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}),
        ],
    );

    assert!(stderr_text.is_empty(), "unexpected stderr:\n{stderr_text}");
    assert_eq!(responses.len(), 2);
    assert_eq!(responses[1]["jsonrpc"], "2.0");
    assert_eq!(responses[1]["id"], 2);

    let tools = responses[1]["result"]["tools"]
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
    let workdir = prepare_workdir();
    index_fixture(&workdir);

    let (responses, stderr_text) = run_mcp_session(
        &workdir,
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

    assert!(stderr_text.is_empty(), "unexpected stderr:\n{stderr_text}");
    assert_eq!(responses.len(), 2);
    assert_eq!(responses[1]["jsonrpc"], "2.0");
    assert_eq!(responses[1]["id"], 2);
    assert_eq!(responses[1]["result"]["content"][0]["type"], "text");

    let content = responses[1]["result"]["content"][0]["text"]
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
    let workdir = prepare_workdir();
    index_fixture(&workdir);

    let (responses, stderr_text) = run_mcp_session(
        &workdir,
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

    assert!(stderr_text.is_empty(), "unexpected stderr:\n{stderr_text}");
    assert_eq!(responses.len(), 3);
    assert_eq!(responses[1]["id"], 2);
    assert_eq!(responses[1]["error"]["code"], -32602);
    assert_eq!(
        responses[1]["error"]["message"],
        "Tool arguments do not match the expected schema"
    );
    assert_eq!(responses[2]["id"], 3);
    assert!(responses[2]["result"]["tools"].is_array());
}

#[test]
fn trace_impact_returns_text_content_for_imported_symbol() {
    let workdir = prepare_workdir();
    index_fixture(&workdir);

    let helper_nodes = search_for_symbol(&workdir, "helper");
    let helper_id = helper_nodes
        .iter()
        .find(|node| node["name"] == "helper" && node["kind"] == "function")
        .and_then(|node| node["id"].as_str())
        .expect("helper function should be indexed")
        .to_string();

    let (responses, stderr_text) = run_mcp_session(
        &workdir,
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

    assert!(stderr_text.is_empty(), "unexpected stderr:\n{stderr_text}");
    assert_eq!(responses[1]["result"]["content"][0]["type"], "text");

    let content = responses[1]["result"]["content"][0]["text"]
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
