//! A deliberately awkward MCP server for the gateway's tests (P3-M006). It paginates its tool
//! list, sends its own `ping` with an id that collides with the client's, emits notifications
//! before answers, and has tools that echo, leak, stall, fail, raise JSON-RPC errors, and crash.
//! `--exit-at-start` exits at once; `--silent` never answers anything.

use serde_json::{Value, json};
use std::io::{BufRead, Write};

fn main() {
    let arguments = std::env::args().skip(1).collect::<Vec<_>>();
    if arguments
        .iter()
        .any(|argument| argument == "--exit-at-start")
    {
        std::process::exit(7);
    }
    let silent = arguments.iter().any(|argument| argument == "--silent");
    // The initialize answer waits until the client has answered our own ping.
    let mut pending_initialize = None::<Value>;
    let mut initialized = false;
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    let mut send = |message: Value| {
        let _ = writeln!(stdout, "{message}");
        let _ = stdout.flush();
    };
    for line in stdin.lock().lines() {
        let Ok(line) = line else { return };
        let Ok(message) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        if silent {
            continue;
        }
        let Some(method) = message["method"].as_str() else {
            // The client's answer to our ping: only now answer its initialize.
            if message["id"] == 1 && message.get("result").is_some() {
                if let Some(initialize) = pending_initialize.take() {
                    initialized = true;
                    send(
                        json!({ "jsonrpc": "2.0", "id": initialize["id"], "result": {
                        "protocolVersion": initialize["params"]["protocolVersion"],
                        "capabilities": { "tools": {} },
                        "serverInfo": { "name": "fixture", "version": "1" },
                    } }),
                    );
                }
            }
            continue;
        };
        let id = message["id"].clone();
        let reply = |result: Value| json!({ "jsonrpc": "2.0", "id": id, "result": result });
        match method {
            "initialize" => {
                // Our own request, with an id that collides with the client's first request; the
                // initialize answer waits for the client's reply to it.
                pending_initialize = Some(message.clone());
                send(json!({ "jsonrpc": "2.0", "id": 1, "method": "ping" }));
            }
            "tools/list" | "tools/call" if !initialized => {
                // Like a real server: nothing is served before initialize completes.
                send(json!({ "jsonrpc": "2.0", "id": id,
                             "error": { "code": -32_002, "message": "not initialized" } }));
            }
            "tools/list" => {
                let tool = |name: &str| {
                    json!({ "name": name, "description": format!("fixture {name}"),
                            "inputSchema": { "type": "object" } })
                };
                if message["params"]["cursor"] == "page-2" {
                    send(reply(
                        json!({ "tools": [tool("fail"), tool("rpc_error"), tool("crash"), tool("env")] }),
                    ));
                } else {
                    send(reply(
                        json!({ "tools": [tool("echo"), tool("leak"), tool("slow")],
                                       "nextCursor": "page-2" }),
                    ));
                }
            }
            "tools/call" => {
                send(json!({ "jsonrpc": "2.0", "method": "notifications/message",
                             "params": { "level": "info", "data": "working" } }));
                let arguments = message["params"]["arguments"].clone();
                let text = |text: &str| json!({ "content": [{ "type": "text", "text": text }] });
                match message["params"]["name"].as_str().unwrap_or_default() {
                    "echo" => send(reply(json!({
                        "content": [{ "type": "text", "text": arguments.to_string() }],
                        "structuredContent": arguments,
                    }))),
                    "leak" => send(reply(text("secret-data"))),
                    "slow" => {
                        let millis = arguments["ms"].as_u64().unwrap_or(0);
                        std::thread::sleep(std::time::Duration::from_millis(millis));
                        send(reply(text("slept")));
                    }
                    "fail" => send(reply(json!({
                        "content": [{ "type": "text", "text": "bad input" }], "isError": true
                    }))),
                    "rpc_error" => send(json!({ "jsonrpc": "2.0", "id": id,
                        "error": { "code": -32_000, "message": "upstream exploded" } })),
                    "crash" => std::process::exit(3),
                    "env" => send(reply(json!({
                        "content": [{ "type": "text", "text": "env" }],
                        "structuredContent": {
                            "literal": std::env::var("FIXTURE_LITERAL").ok(),
                            "passed": std::env::var("CARGO_MANIFEST_DIR").ok(),
                            "not_passed": std::env::var("PATH").ok(),
                        },
                    }))),
                    other => send(json!({ "jsonrpc": "2.0", "id": id,
                        "error": { "code": -32_602, "message": format!("unknown tool {other}") } })),
                }
            }
            _ => {
                if !id.is_null() {
                    send(json!({ "jsonrpc": "2.0", "id": id,
                                 "error": { "code": -32_601, "message": "method not found" } }));
                }
            }
        }
    }
}
