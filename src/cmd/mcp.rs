use std::io::{self, BufRead, Write};

use anyhow::Context;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::storage::{db, query};

const JSONRPC_VERSION: &str = "2.0";
const SERVER_NAME: &str = "adn";
const SERVER_VERSION: &str = env!("CARGO_PKG_VERSION");
const MCP_PROTOCOL_VERSION: &str = "2024-11-05";
const METHOD_INITIALIZE: &str = "initialize";
const METHOD_INITIALIZED: &str = "notifications/initialized";
const METHOD_TOOLS_LIST: &str = "tools/list";
const METHOD_TOOLS_CALL: &str = "tools/call";
const METHOD_PING: &str = "ping";

pub fn run_serve() -> anyhow::Result<()> {
    let conn = db::init_db().context("failed to open ADN database")?;
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    let mut initialized = false;
    let mut handshake_complete = false;

    for line_result in stdin.lock().lines() {
        let line = match line_result {
            Ok(line) => line,
            Err(error) => {
                eprintln!("failed to read stdin: {error}");
                return Err(error.into());
            }
        };

        if line.trim().is_empty() {
            continue;
        }

        match handle_message(&conn, &mut stdout, &mut initialized, &mut handshake_complete, &line)
        {
            Ok(()) => {}
            Err(error) => {
                eprintln!("fatal MCP server error: {error:#}");
                return Err(error);
            }
        }
    }

    Ok(())
}

fn handle_message(
    conn: &Connection,
    stdout: &mut io::Stdout,
    initialized: &mut bool,
    handshake_complete: &mut bool,
    line: &str,
) -> anyhow::Result<()> {
    let request = match parse_request(line) {
        Ok(request) => request,
        Err(error) => {
            eprintln!("failed to parse request: {error:#}");
            write_error_response(stdout, None, PARSE_ERROR, "Parse error", None)?;
            return Ok(());
        }
    };

    if request.jsonrpc != JSONRPC_VERSION {
        eprintln!("received non-2.0 jsonrpc version: {}", request.jsonrpc);
        write_error_response(
            stdout,
            request.id,
            INVALID_REQUEST,
            "Invalid Request",
            Some(json!("jsonrpc must be \"2.0\"")),
        )?;
        return Ok(());
    }

    let method = request.method.as_str();

    if !*initialized && method != METHOD_INITIALIZE {
        if let Some(id) = request.id {
            write_error_response(
                stdout,
                Some(id),
                SERVER_NOT_INITIALIZED,
                "Server not initialized",
                None,
            )?;
        }
        return Ok(());
    }

    if *initialized && !*handshake_complete && method != METHOD_INITIALIZED && method != METHOD_PING {
        if let Some(id) = request.id {
            write_error_response(
                stdout,
                Some(id),
                SERVER_NOT_INITIALIZED,
                "Server handshake incomplete",
                None,
            )?;
        }
        return Ok(());
    }

    match method {
        METHOD_INITIALIZE => handle_initialize(stdout, initialized, request.id),
        METHOD_INITIALIZED => {
            *handshake_complete = true;
            Ok(())
        }
        METHOD_PING => {
            if let Some(id) = request.id {
                write_success_response(stdout, id, json!({}))?;
            }
            Ok(())
        }
        METHOD_TOOLS_LIST => handle_tools_list(stdout, request.id),
        METHOD_TOOLS_CALL => match handle_tools_call(conn, stdout, request.id.clone(), request.params) {
            Ok(()) => Ok(()),
            Err(error) => {
                eprintln!("tools/call failed: {error:#}");
                if let Some(id) = request.id {
                    write_error_response(
                        stdout,
                        Some(id),
                        INTERNAL_ERROR,
                        "Tool execution failed",
                        Some(json!(error.to_string())),
                    )?;
                }
                Ok(())
            }
        },
        _ => {
            if let Some(id) = request.id {
                write_error_response(stdout, Some(id), METHOD_NOT_FOUND, "Method not found", None)?;
            }
            Ok(())
        }
    }
}

fn parse_request(line: &str) -> anyhow::Result<JsonRpcRequest> {
    serde_json::from_str::<JsonRpcRequest>(line).context("request is not valid JSON-RPC")
}

fn handle_initialize(
    stdout: &mut io::Stdout,
    initialized: &mut bool,
    id: Option<Value>,
) -> anyhow::Result<()> {
    let Some(id) = id else {
        return Ok(());
    };

    *initialized = true;

    let result = InitializeResult {
        protocol_version: MCP_PROTOCOL_VERSION.to_string(),
        capabilities: ServerCapabilities {
            tools: Some(ToolsCapability {
                list_changed: Some(false),
            }),
        },
        server_info: ServerInfo {
            name: SERVER_NAME.to_string(),
            version: SERVER_VERSION.to_string(),
        },
    };

    write_success_response(stdout, id, serde_json::to_value(result)?)
}

fn handle_tools_list(stdout: &mut io::Stdout, id: Option<Value>) -> anyhow::Result<()> {
    let Some(id) = id else {
        return Ok(());
    };

    let result = ToolsListResult {
        tools: vec![
            ToolDefinition {
                name: "search_codebase".to_string(),
                description: "Search symbols in the indexed codebase by name fragment.".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "query": {
                            "type": "string",
                            "description": "Case-insensitive symbol name fragment."
                        }
                    },
                    "required": ["query"],
                    "additionalProperties": false
                }),
            },
            ToolDefinition {
                name: "get_node_details".to_string(),
                description: "Fetch a node plus its incoming and outgoing graph edges.".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "id": {
                            "type": "string",
                            "description": "Exact node identifier."
                        }
                    },
                    "required": ["id"],
                    "additionalProperties": false
                }),
            },
            ToolDefinition {
                name: "list_file_symbols".to_string(),
                description: "List all indexed symbols for a repo-relative file path.".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "Repo-relative file path."
                        }
                    },
                    "required": ["path"],
                    "additionalProperties": false
                }),
            },
            ToolDefinition {
                name: "trace_impact".to_string(),
                description: "Trace upstream callers and references that impact a node.".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "id": {
                            "type": "string",
                            "description": "Exact node identifier."
                        }
                    },
                    "required": ["id"],
                    "additionalProperties": false
                }),
            },
        ],
    };

    write_success_response(stdout, id, serde_json::to_value(result)?)
}

fn handle_tools_call(
    conn: &Connection,
    stdout: &mut io::Stdout,
    id: Option<Value>,
    params: Option<Value>,
) -> anyhow::Result<()> {
    let Some(id) = id else {
        return Ok(());
    };

    let params = params.unwrap_or(Value::Null);
    let request: ToolCallRequest = match serde_json::from_value(params) {
        Ok(request) => request,
        Err(error) => {
            write_error_response(
                stdout,
                Some(id),
                INVALID_PARAMS,
                "Invalid tool call parameters",
                Some(json!(error.to_string())),
            )?;
            return Ok(());
        }
    };

    let output_text = match request.name.as_str() {
        "search_codebase" => {
            let args: SearchCodebaseArgs =
                match parse_tool_arguments(stdout, &request.arguments, id.clone())? {
                    Some(args) => args,
                    None => return Ok(()),
                };
            let results = query::search_symbols(conn, &args.query)?;
            serde_json::to_string_pretty(&results)?
        }
        "get_node_details" => {
            let args: NodeIdArgs = match parse_tool_arguments(stdout, &request.arguments, id.clone())?
            {
                Some(args) => args,
                None => return Ok(()),
            };
            let result = query::get_node_details(conn, &args.id)?;
            serde_json::to_string_pretty(&result)?
        }
        "list_file_symbols" => {
            let args: FilePathArgs =
                match parse_tool_arguments(stdout, &request.arguments, id.clone())? {
                    Some(args) => args,
                    None => return Ok(()),
                };
            let result = query::get_file_symbols(conn, &normalize_path(&args.path))?;
            serde_json::to_string_pretty(&result)?
        }
        "trace_impact" => {
            let args: NodeIdArgs = match parse_tool_arguments(stdout, &request.arguments, id.clone())?
            {
                Some(args) => args,
                None => return Ok(()),
            };
            let result = query::trace_impact(conn, &args.id)?;
            serde_json::to_string_pretty(&result)?
        }
        _ => {
            write_error_response(stdout, Some(id), INVALID_PARAMS, "Unknown tool", None)?;
            return Ok(());
        }
    };

    let result = ToolCallResult {
        content: vec![TextContent {
            content_type: "text".to_string(),
            text: output_text,
        }],
        is_error: Some(false),
    };

    write_success_response(stdout, id, serde_json::to_value(result)?)
}

fn parse_tool_arguments<T>(
    stdout: &mut io::Stdout,
    arguments: &Option<Value>,
    id: Value,
) -> anyhow::Result<Option<T>>
where
    T: for<'de> Deserialize<'de>,
{
    let Some(value) = arguments.clone() else {
        write_error_response(
            stdout,
            Some(id),
            INVALID_PARAMS,
            "Tool arguments are required",
            None,
        )?;
        return Ok(None);
    };

    match serde_json::from_value(value) {
        Ok(args) => Ok(Some(args)),
        Err(error) => {
            write_error_response(
                stdout,
                Some(id),
                INVALID_PARAMS,
                "Tool arguments do not match the expected schema",
                Some(json!(error.to_string())),
            )?;
            Ok(None)
        }
    }
}

fn normalize_path(path: &str) -> String {
    path.trim().replace('\\', "/").trim_start_matches("./").to_string()
}

fn write_success_response(stdout: &mut io::Stdout, id: Value, result: Value) -> anyhow::Result<()> {
    let response = JsonRpcSuccess {
        jsonrpc: JSONRPC_VERSION,
        id,
        result,
    };

    write_json(stdout, &response)
}

fn write_error_response(
    stdout: &mut io::Stdout,
    id: Option<Value>,
    code: i64,
    message: &str,
    data: Option<Value>,
) -> anyhow::Result<()> {
    let response = JsonRpcErrorResponse {
        jsonrpc: JSONRPC_VERSION,
        id: id.unwrap_or(Value::Null),
        error: JsonRpcError {
            code,
            message: message.to_string(),
            data,
        },
    };

    write_json(stdout, &response)
}

fn write_json<T>(stdout: &mut io::Stdout, response: &T) -> anyhow::Result<()>
where
    T: Serialize,
{
    serde_json::to_writer(&mut *stdout, response)?;
    stdout.write_all(b"\n")?;
    stdout.flush()?;
    Ok(())
}

const PARSE_ERROR: i64 = -32700;
const INVALID_REQUEST: i64 = -32600;
const METHOD_NOT_FOUND: i64 = -32601;
const INVALID_PARAMS: i64 = -32602;
const INTERNAL_ERROR: i64 = -32603;
const SERVER_NOT_INITIALIZED: i64 = -32002;

#[derive(Debug, Deserialize)]
struct JsonRpcRequest {
    jsonrpc: String,
    id: Option<Value>,
    method: String,
    params: Option<Value>,
}

#[derive(Debug, Serialize)]
struct JsonRpcSuccess<'a> {
    jsonrpc: &'a str,
    id: Value,
    result: Value,
}

#[derive(Debug, Serialize)]
struct JsonRpcErrorResponse<'a> {
    jsonrpc: &'a str,
    id: Value,
    error: JsonRpcError,
}

#[derive(Debug, Serialize)]
struct JsonRpcError {
    code: i64,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<Value>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct InitializeResult {
    protocol_version: String,
    capabilities: ServerCapabilities,
    server_info: ServerInfo,
}

#[derive(Debug, Serialize)]
struct ServerCapabilities {
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<ToolsCapability>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ToolsCapability {
    #[serde(skip_serializing_if = "Option::is_none")]
    list_changed: Option<bool>,
}

#[derive(Debug, Serialize)]
struct ServerInfo {
    name: String,
    version: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ToolsListResult {
    tools: Vec<ToolDefinition>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ToolDefinition {
    name: String,
    description: String,
    input_schema: Value,
}

#[derive(Debug, Deserialize)]
struct ToolCallRequest {
    name: String,
    arguments: Option<Value>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ToolCallResult {
    content: Vec<TextContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    is_error: Option<bool>,
}

#[derive(Debug, Serialize)]
struct TextContent {
    #[serde(rename = "type")]
    content_type: String,
    text: String,
}

#[derive(Debug, Deserialize)]
struct SearchCodebaseArgs {
    query: String,
}

#[derive(Debug, Deserialize)]
struct NodeIdArgs {
    id: String,
}

#[derive(Debug, Deserialize)]
struct FilePathArgs {
    path: String,
}
