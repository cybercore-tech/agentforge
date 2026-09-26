//! The client side of one external MCP server, behind the gateway (P3-M006, ADR-0055).
//!
//! The server is started with a cleared environment (plus its declared `env.` and `pass_env`
//! values) and spoken to over stdio, newline-delimited JSON-RPC. Every request is bounded by the
//! declared timeout, and every message by [`crate::MAX_MESSAGE_BYTES`]. A server that crashes,
//! floods, or stops answering cannot hang or crash the gateway.

use crate::config::ServerConfig;
use crate::{MAX_MESSAGE_BYTES, SUPPORTED_PROTOCOL_VERSIONS};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::thread;
use std::time::Instant;

/// Most `tools/list` pages read from one server.
const MAX_TOOL_PAGES: usize = 16;

/// Why a request to an upstream server did not produce a result.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UpstreamError {
    /// No answer within the declared timeout.
    Timeout,
    /// The server answered with a JSON-RPC error.
    Rpc(String),
    /// The server is gone (exited, closed its output, or sent something unusable).
    Gone(String),
}

impl std::fmt::Display for UpstreamError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Timeout => formatter.write_str("the server did not answer in time"),
            Self::Rpc(message) => write!(formatter, "the server returned an error: {message}"),
            Self::Gone(reason) => write!(formatter, "the server is unavailable: {reason}"),
        }
    }
}

/// A running external MCP server.
pub struct Upstream {
    /// The declaration it was started from.
    pub config: ServerConfig,
    /// The mapped tools the server really offers, by upstream name, as it described them.
    pub tools: BTreeMap<String, Value>,
    child: Child,
    stdin: ChildStdin,
    messages: Receiver<Result<Value, String>>,
    next_id: u64,
    gone: Option<String>,
}

impl std::fmt::Debug for Upstream {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Upstream")
            .field("server", &self.config.name)
            .field("tools", &self.tools.keys().collect::<Vec<_>>())
            .finish_non_exhaustive()
    }
}

impl Upstream {
    /// Starts the server, runs `initialize`, and reads its tools.
    ///
    /// # Errors
    ///
    /// Returns why the server could not be started or did not complete the handshake.
    pub fn start(config: ServerConfig) -> Result<Self, String> {
        let mut command = Command::new(&config.executable);
        command
            .args(&config.arguments)
            .env_clear()
            .envs(&config.env)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        for name in &config.pass_env {
            if let Some(value) = std::env::var_os(name) {
                command.env(name, value);
            }
        }
        let mut child = command
            .spawn()
            .map_err(|error| format!("cannot start {}: {error}", config.executable.display()))?;
        let stdin = child.stdin.take().ok_or("no stdin")?;
        let stdout = child.stdout.take().ok_or("no stdout")?;
        let (sender, messages) = mpsc::channel();
        thread::spawn(move || read_messages(stdout, &sender));
        let mut upstream = Self {
            config,
            tools: BTreeMap::new(),
            child,
            stdin,
            messages,
            next_id: 0,
            gone: None,
        };
        upstream
            .request(
                "initialize",
                json!({
                    "protocolVersion": SUPPORTED_PROTOCOL_VERSIONS[0],
                    "capabilities": {},
                    "clientInfo": { "name": "agentforge-gateway", "version": env!("CARGO_PKG_VERSION") },
                }),
            )
            .map_err(|error| format!("initialize failed: {error}"))?;
        upstream
            .notify("notifications/initialized")
            .map_err(|error| format!("initialize failed: {error}"))?;
        let mut cursor = None::<Value>;
        for _ in 0..MAX_TOOL_PAGES {
            let params = cursor.map_or_else(|| json!({}), |cursor| json!({ "cursor": cursor }));
            let page = upstream
                .request("tools/list", params)
                .map_err(|error| format!("tools/list failed: {error}"))?;
            for tool in page["tools"].as_array().into_iter().flatten() {
                if let Some(name) = tool["name"].as_str() {
                    // Only mapped tools are kept: unmapped means invisible.
                    if upstream.config.tools.contains_key(name) {
                        upstream.tools.insert(name.to_owned(), tool.clone());
                    }
                }
            }
            match page.get("nextCursor") {
                Some(next) if !next.is_null() => cursor = Some(next.clone()),
                _ => break,
            }
        }
        Ok(upstream)
    }

    /// Calls one of the server's tools and returns its result unchanged.
    ///
    /// # Errors
    ///
    /// Returns [`UpstreamError`] for a timeout, an error answer, or a server that is gone.
    pub fn call(&mut self, tool: &str, arguments: &Value) -> Result<Value, UpstreamError> {
        self.request(
            "tools/call",
            json!({ "name": tool, "arguments": arguments }),
        )
    }

    fn notify(&mut self, method: &str) -> Result<(), UpstreamError> {
        self.send(&json!({ "jsonrpc": "2.0", "method": method }))
    }

    fn send(&mut self, message: &Value) -> Result<(), UpstreamError> {
        if let Some(reason) = &self.gone {
            return Err(UpstreamError::Gone(reason.clone()));
        }
        let written = writeln!(self.stdin, "{message}").and_then(|()| self.stdin.flush());
        written.map_err(|error| self.mark_gone(format!("cannot write to it: {error}")))
    }

    fn request(&mut self, method: &str, params: Value) -> Result<Value, UpstreamError> {
        self.next_id += 1;
        let id = self.next_id;
        self.send(&json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }))?;
        let deadline = Instant::now() + self.config.timeout;
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            let message = match self.messages.recv_timeout(remaining) {
                Ok(Ok(message)) => message,
                Ok(Err(reason)) => return Err(self.mark_gone(reason)),
                Err(RecvTimeoutError::Timeout) => return Err(UpstreamError::Timeout),
                Err(RecvTimeoutError::Disconnected) => {
                    return Err(self.mark_gone("its output closed".into()));
                }
            };
            // A message with a method is the server's own request or notification, never an answer
            // to ours (its ids are its own). The gateway answers its pings and refuses the rest.
            if let Some(method) = message.get("method").and_then(Value::as_str) {
                if let Some(request_id) = message.get("id").cloned() {
                    let reply = if method == "ping" {
                        json!({ "jsonrpc": "2.0", "id": request_id, "result": {} })
                    } else {
                        json!({ "jsonrpc": "2.0", "id": request_id,
                                "error": { "code": -32_601, "message": "not supported by the gateway" } })
                    };
                    self.send(&reply)?;
                }
                continue;
            }
            // Late answers to earlier (timed-out) requests are skipped.
            if message.get("id").and_then(Value::as_u64) != Some(id) {
                continue;
            }
            if let Some(error) = message.get("error") {
                let text = error["message"].as_str().unwrap_or("unknown error");
                return Err(UpstreamError::Rpc(crate::bounded(text)));
            }
            return Ok(message.get("result").cloned().unwrap_or(Value::Null));
        }
    }

    fn mark_gone(&mut self, reason: String) -> UpstreamError {
        let reason = match self.child.try_wait() {
            Ok(Some(status)) => format!("{reason} (exited: {status})"),
            _ => reason,
        };
        self.gone = Some(reason.clone());
        UpstreamError::Gone(reason)
    }
}

impl Drop for Upstream {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Reads bounded, newline-delimited JSON messages until the server's output ends or misbehaves.
fn read_messages(stdout: impl Read, sender: &mpsc::Sender<Result<Value, String>>) {
    let mut reader = BufReader::new(stdout);
    loop {
        let mut line = Vec::new();
        let read = match (&mut reader)
            .take(MAX_MESSAGE_BYTES as u64 + 1)
            .read_until(b'\n', &mut line)
        {
            Ok(read) => read,
            Err(error) => {
                let _ = sender.send(Err(format!("reading its output failed: {error}")));
                return;
            }
        };
        if read == 0 {
            let _ = sender.send(Err("it closed its output".into()));
            return;
        }
        if line.len() > MAX_MESSAGE_BYTES {
            let _ = sender.send(Err("it sent a message over the size limit".into()));
            return;
        }
        let text = String::from_utf8_lossy(&line);
        if text.trim().is_empty() {
            continue;
        }
        match serde_json::from_str::<Value>(text.trim()) {
            Ok(message) => {
                if sender.send(Ok(message)).is_err() {
                    return;
                }
            }
            Err(error) => {
                let _ = sender.send(Err(format!("it sent malformed JSON: {error}")));
                return;
            }
        }
    }
}
