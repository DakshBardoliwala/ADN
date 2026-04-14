mod support;

use serde_json::Value;

use support::{assert_command_success, TestWorkspace};

#[test]
fn cli_search_runs_in_isolated_temp_workspace() {
    let workspace = TestWorkspace::new();
    workspace.index_fixture_ok("mcp_sample");

    let output = workspace.run_cli(&["search", "helper", "--json"]);
    assert_command_success("search command failed", &output);

    let payload =
        serde_json::from_slice::<Vec<Value>>(&output.stdout).expect("search output should be valid json");

    assert!(
        payload
            .iter()
            .any(|node| node["name"] == "helper" && node["file_path"] == "helpers.py"),
        "expected helper symbol in search results: {payload:?}"
    );
}
