//! JSON-RPC 2.0 stdio server loop.
//!
//! Wire format: one JSON object per line on stdin; one JSON object per line on
//! stdout. Stderr is reserved for human-readable diagnostics.
//!
//! Supported methods:
//! - `initialize` → server capabilities + info
//! - `notifications/initialized` → no-op (notification, no response)
//! - `tools/list` → enumerate registered tools
//! - `tools/call` → dispatch to tool handler in [`crate::tools`]
//! - any other → error -32601 Method not found

use anyhow::Result;
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

use crate::tools;

/// Server name advertised in `initialize`.
pub const SERVER_NAME: &str = "pixiekit-mcp";
/// Server version advertised in `initialize`.
pub const SERVER_VERSION: &str = env!("CARGO_PKG_VERSION");
/// MCP protocol version this server speaks.
pub const PROTOCOL_VERSION: &str = "2024-11-05";

// --- JSON-RPC standard error codes ---
pub const ERR_PARSE: i32 = -32700;
pub const ERR_INVALID_REQUEST: i32 = -32600;
pub const ERR_METHOD_NOT_FOUND: i32 = -32601;
pub const ERR_INVALID_PARAMS: i32 = -32602;
pub const ERR_INTERNAL: i32 = -32603;

#[derive(Debug, Deserialize)]
pub struct Request {
    pub jsonrpc: String,
    /// `None` indicates a notification (no response expected).
    #[serde(default)]
    pub id: Option<Value>,
    pub method: String,
    #[serde(default)]
    pub params: Option<Value>,
}

/// Build a successful JSON-RPC response.
pub fn ok_response(id: Value, result: Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result,
    })
}

/// Build a JSON-RPC error response.
pub fn err_response(id: Value, code: i32, message: impl Into<String>) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": {
            "code": code,
            "message": message.into(),
        },
    })
}

/// Run the stdio loop until stdin closes.
pub async fn run_stdio() -> Result<()> {
    let stdin = tokio::io::stdin();
    let mut stdout = tokio::io::stdout();
    let mut reader = BufReader::new(stdin).lines();

    while let Some(line) = reader.next_line().await? {
        if line.trim().is_empty() {
            continue;
        }
        if let Some(response) = handle_line(&line) {
            let mut bytes = serde_json::to_vec(&response)?;
            bytes.push(b'\n');
            stdout.write_all(&bytes).await?;
            stdout.flush().await?;
        }
    }
    Ok(())
}

/// Parse a single line and return an optional response (notifications produce
/// `None`). Pure function — easy to unit-test.
pub fn handle_line(line: &str) -> Option<Value> {
    // Parse JSON
    let raw: Value = match serde_json::from_str(line) {
        Ok(v) => v,
        Err(e) => {
            return Some(err_response(
                Value::Null,
                ERR_PARSE,
                format!("Parse error: {e}"),
            ));
        }
    };

    // Pull id early so even malformed requests can echo it back.
    let id = raw.get("id").cloned().unwrap_or(Value::Null);

    let req: Request = match serde_json::from_value(raw) {
        Ok(r) => r,
        Err(e) => {
            return Some(err_response(
                id,
                ERR_INVALID_REQUEST,
                format!("Invalid Request: {e}"),
            ));
        }
    };

    if req.jsonrpc != "2.0" {
        return Some(err_response(
            req.id.unwrap_or(Value::Null),
            ERR_INVALID_REQUEST,
            "jsonrpc must be \"2.0\"",
        ));
    }

    let is_notification = req.id.is_none();
    let response = dispatch(&req);

    if is_notification {
        // JSON-RPC: notifications must not receive a response.
        None
    } else {
        Some(response)
    }
}

fn dispatch(req: &Request) -> Value {
    let id = req.id.clone().unwrap_or(Value::Null);

    match req.method.as_str() {
        "initialize" => ok_response(id, initialize_result()),
        "notifications/initialized" | "initialized" => {
            // No-op ack. Caller in handle_line drops responses for notifications.
            ok_response(id, json!({}))
        }
        "tools/list" => ok_response(id, json!({ "tools": tools::list_tools() })),
        "tools/call" => match tools::call(req.params.as_ref()) {
            Ok(result) => ok_response(id, result),
            Err(tool_err) => json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": {
                    "code": tool_err.code,
                    "message": tool_err.message,
                }
            }),
        },
        // Optional ping for transport keepalive — common in MCP clients.
        "ping" => ok_response(id, json!({})),
        other => err_response(
            id,
            ERR_METHOD_NOT_FOUND,
            format!("Method not found: {other}"),
        ),
    }
}

fn initialize_result() -> Value {
    json!({
        "protocolVersion": PROTOCOL_VERSION,
        "capabilities": {
            "tools": {}
        },
        "serverInfo": {
            "name": SERVER_NAME,
            "version": SERVER_VERSION,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initialize_returns_capabilities_and_server_info() {
        let line = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#;
        let resp = handle_line(line).expect("response expected");
        assert_eq!(resp["jsonrpc"], "2.0");
        assert_eq!(resp["id"], 1);
        let result = &resp["result"];
        assert_eq!(result["protocolVersion"], PROTOCOL_VERSION);
        assert!(result["capabilities"]["tools"].is_object());
        assert_eq!(result["serverInfo"]["name"], SERVER_NAME);
    }

    #[test]
    fn notifications_initialized_returns_no_response() {
        // No id => notification, must not return a response.
        let line = r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#;
        assert!(handle_line(line).is_none());
    }

    #[test]
    fn unknown_method_returns_method_not_found() {
        let line = r#"{"jsonrpc":"2.0","id":42,"method":"does_not_exist"}"#;
        let resp = handle_line(line).expect("response expected");
        assert_eq!(resp["error"]["code"], ERR_METHOD_NOT_FOUND);
        assert_eq!(resp["id"], 42);
    }

    #[test]
    fn parse_error_returns_negative_32700_with_null_id() {
        let line = "{not json";
        let resp = handle_line(line).expect("response expected");
        assert_eq!(resp["error"]["code"], ERR_PARSE);
        assert_eq!(resp["id"], Value::Null);
    }

    #[test]
    fn wrong_jsonrpc_version_rejected() {
        let line = r#"{"jsonrpc":"1.0","id":1,"method":"ping"}"#;
        let resp = handle_line(line).expect("response expected");
        assert_eq!(resp["error"]["code"], ERR_INVALID_REQUEST);
    }

    #[test]
    fn ping_succeeds() {
        let line = r#"{"jsonrpc":"2.0","id":7,"method":"ping"}"#;
        let resp = handle_line(line).expect("response expected");
        assert!(resp["result"].is_object());
        assert_eq!(resp["id"], 7);
    }
}
