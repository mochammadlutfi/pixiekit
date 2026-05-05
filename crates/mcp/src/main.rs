//! Pixiekit MCP server — stdio transport.
//!
//! Reads JSON-RPC 2.0 requests line-by-line from stdin and writes responses to
//! stdout. Logs go to stderr. Exposes Pixiekit core tools (bg_remove,
//! video_to_sprite) plus stubs for vectorize and list_presets.

mod server;
mod tools;

use std::process::ExitCode;

#[tokio::main]
async fn main() -> ExitCode {
    eprintln!("pixiekit-mcp starting (stdio transport)");
    match server::run_stdio().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("pixiekit-mcp fatal: {err:#}");
            ExitCode::FAILURE
        }
    }
}
