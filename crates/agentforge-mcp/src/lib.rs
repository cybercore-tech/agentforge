//! MCP task-tool gateway (P3-M005, ADR-0052).
//!
//! A stdio [Model Context Protocol](https://modelcontextprotocol.io) server bound to one task
//! contract and one worktree. It offers the agent working on that task a few AgentForge tools, each
//! mapped to the task's capabilities and checked with [`PolicyEngine`] both when tools are listed
//! and when one is called. Every call, allowed or denied, is recorded as a
//! [`AuditEventKind::ToolInvoked`] event when the project root is known.
//!
//! The gateway never grants authority, records approvals, commits, or changes task state. Gate runs
//! are advisory; the orchestrator's gates after the agent's run stay authoritative.

pub mod config;
pub mod upstream;

use agentforge_audit::{AuditEvent, AuditEventKind, AuditStore, FileAuditStore};
use agentforge_core::agent::{AgentTask, Capability};
use agentforge_gate::{GateOutcome, GateProfileStore, GateRunner};
use agentforge_policy::{PolicyDecision, PolicyEngine, PolicyRequest};
use agentforge_state::FileTaskStore;
use serde_json::{Value, json};
use std::io::{self, BufRead, Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use upstream::{Upstream, UpstreamError};

/// Protocol versions this server speaks, newest first. A client asking for another version is
/// answered with the newest, as the specification requires; the client then decides.
pub const SUPPORTED_PROTOCOL_VERSIONS: &[&str] =
    &["2025-11-25", "2025-06-18", "2025-03-26", "2024-11-05"];
/// Largest accepted JSON-RPC message (one line).
pub const MAX_MESSAGE_BYTES: usize = 1024 * 1024;
/// Most changed paths reported by `check_changes`.
pub const MAX_REPORTED_CHANGES: usize = 1_000;
/// Bytes kept from the end of a gate's stdout and stderr.
pub const MAX_GATE_TAIL_BYTES: usize = 4 * 1024;

const AUDIT_RELATIVE_PATH: &str = ".forge/audit.log";

/// The task tools and the capabilities each requires.
///
/// | Tool | Requires |
/// | --- | --- |
/// | `task_contract` | `use_mcp_tools`, `read_repository` |
/// | `check_changes` | `use_mcp_tools`, `read_repository` |
/// | `run_gate` | `use_mcp_tools`, `run_local_commands` |
pub const TOOL_POLICY: &[(&str, &[Capability])] = &[
    (
        "task_contract",
        &[Capability::UseMcpTools, Capability::ReadRepository],
    ),
    (
        "check_changes",
        &[Capability::UseMcpTools, Capability::ReadRepository],
    ),
    (
        "run_gate",
        &[Capability::UseMcpTools, Capability::RunLocalCommands],
    ),
];

// JSON-RPC 2.0 error codes.
const PARSE_ERROR: i64 = -32_700;
const INVALID_REQUEST: i64 = -32_600;
const METHOD_NOT_FOUND: i64 = -32_601;
const INVALID_PARAMS: i64 = -32_602;
const INTERNAL_ERROR: i64 = -32_603;

/// A stdio MCP server for one task.
#[derive(Debug)]
pub struct Gateway {
    task: AgentTask,
    worktree: PathBuf,
    root: Option<PathBuf>,
    initialized: bool,
    /// External MCP servers declared in the project (P3-M006), started at `initialize`.
    upstreams: Vec<Upstream>,
}

impl Gateway {
    /// Creates a gateway for `task`, working in `worktree`. With `root`, calls are audited in that
    /// project, and `run_gate` loads gate profiles from it.
    #[must_use]
    pub fn new(task: AgentTask, worktree: impl Into<PathBuf>, root: Option<PathBuf>) -> Self {
        Self {
            task,
            worktree: worktree.into(),
            root,
            initialized: false,
            upstreams: Vec::new(),
        }
    }

    /// Serves newline-delimited JSON-RPC messages from `reader` until end of input, writing only
    /// protocol messages to `writer`.
    ///
    /// # Errors
    ///
    /// Returns an I/O error when reading or writing the transport fails.
    pub fn serve(&mut self, mut reader: impl BufRead, mut writer: impl Write) -> io::Result<()> {
        loop {
            let mut line = Vec::new();
            let read = (&mut reader)
                .take(MAX_MESSAGE_BYTES as u64 + 1)
                .read_until(b'\n', &mut line)?;
            if read == 0 {
                return Ok(());
            }
            let response = if line.len() > MAX_MESSAGE_BYTES && !line.ends_with(b"\n") {
                skip_rest_of_line(&mut reader)?;
                Some(error(
                    &Value::Null,
                    INVALID_REQUEST,
                    "message exceeds the size limit",
                ))
            } else {
                let text = String::from_utf8_lossy(&line);
                let text = text.trim();
                if text.is_empty() {
                    continue;
                }
                self.handle(text)
            };
            if let Some(response) = response {
                writeln!(writer, "{response}")?;
                writer.flush()?;
            }
        }
    }

    /// Handles one JSON-RPC message; returns the response, or `None` for a notification.
    pub fn handle(&mut self, message: &str) -> Option<Value> {
        let value: Value = match serde_json::from_str(message) {
            Ok(value) => value,
            Err(parse) => {
                return Some(error(
                    &Value::Null,
                    PARSE_ERROR,
                    &format!("parse error: {parse}"),
                ));
            }
        };
        let Some(object) = value.as_object() else {
            let reason = if value.is_array() {
                "batches are not supported"
            } else {
                "a message must be a JSON object"
            };
            return Some(error(&Value::Null, INVALID_REQUEST, reason));
        };
        let id = object.get("id").cloned();
        let respond = |response: Value| id.is_some().then_some(response);
        let reply_id = id.clone().unwrap_or(Value::Null);
        if object.get("jsonrpc").and_then(Value::as_str) != Some("2.0") {
            return respond(error(&reply_id, INVALID_REQUEST, "jsonrpc must be \"2.0\""));
        }
        let Some(method) = object.get("method").and_then(Value::as_str) else {
            return respond(error(&reply_id, INVALID_REQUEST, "method is missing"));
        };
        if id
            .as_ref()
            .is_some_and(|id| !(id.is_string() || id.is_i64() || id.is_u64()))
        {
            return Some(error(
                &Value::Null,
                INVALID_REQUEST,
                "id must be a string or an integer",
            ));
        }
        let params = object.get("params").cloned().unwrap_or(Value::Null);
        match method {
            "initialize" => respond(self.initialize(&reply_id, &params)),
            "ping" => respond(success(&reply_id, json!({}))),
            "notifications/initialized" | "notifications/cancelled" => None,
            _ if !self.initialized => respond(error(
                &reply_id,
                INVALID_REQUEST,
                "the session is not initialized",
            )),
            "tools/list" => respond(success(&reply_id, json!({ "tools": self.listed_tools() }))),
            "tools/call" => respond(self.call(&reply_id, &params)),
            _ => respond(error(
                &reply_id,
                METHOD_NOT_FOUND,
                &format!("method not found: {method}"),
            )),
        }
    }

    fn initialize(&mut self, id: &Value, params: &Value) -> Value {
        let requested = params
            .get("protocolVersion")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let version = SUPPORTED_PROTOCOL_VERSIONS
            .iter()
            .find(|supported| **supported == requested)
            .unwrap_or(&SUPPORTED_PROTOCOL_VERSIONS[0]);
        if !self.initialized {
            self.start_upstreams();
        }
        self.initialized = true;
        success(
            id,
            json!({
                "protocolVersion": version,
                "capabilities": { "tools": { "listChanged": false } },
                "serverInfo": { "name": "agentforge", "version": env!("CARGO_PKG_VERSION") },
                "instructions": format!(
                    "AgentForge task tools for {}. task_contract shows what this task may change; \
                     check_changes judges your current changes against it; run_gate runs one of \
                     the task's required gates in your worktree (advisory). Every call is recorded.",
                    self.task.task_id
                ),
            }),
        )
    }

    /// Starts the project's declared external servers; a server that fails is logged to stderr
    /// and skipped, and the others still start (P3-M006).
    fn start_upstreams(&mut self) {
        let Some(root) = &self.root else { return };
        let (configs, errors) = config::load_server_configs(root);
        for error in errors {
            let _ = writeln!(io::stderr().lock(), "agentforge-mcp: skipped: {error}");
        }
        for config in configs {
            let name = config.name.clone();
            match Upstream::start(config) {
                Ok(upstream) => self.upstreams.push(upstream),
                Err(error) => {
                    let _ = writeln!(
                        io::stderr().lock(),
                        "agentforge-mcp: server {name} skipped: {error}"
                    );
                }
            }
        }
    }

    /// The tools this task may call: the native ones in [`TOOL_POLICY`] order, then each external
    /// server's mapped tools as `<server>__<tool>`.
    #[must_use]
    pub fn listed_tools(&self) -> Vec<Value> {
        let mut tools = TOOL_POLICY
            .iter()
            .filter(|(name, _)| self.availability(name).is_ok())
            .map(|(name, _)| self.descriptor(name))
            .collect::<Vec<_>>();
        for upstream in &self.upstreams {
            for (tool, described) in &upstream.tools {
                let name = format!("{}__{tool}", upstream.config.name);
                if self.availability(&name).is_ok() {
                    let mut descriptor = described.clone();
                    descriptor["name"] = json!(name);
                    let description = described["description"].as_str().unwrap_or_default();
                    descriptor["description"] =
                        json!(format!("[{}] {description}", upstream.config.name));
                    tools.push(descriptor);
                }
            }
        }
        tools
    }

    /// Splits `<server>__<tool>` into the upstream index and its tool name, when such a server
    /// offers such a mapped tool.
    fn upstream_tool(&self, name: &str) -> Option<(usize, String)> {
        let (server, tool) = name.split_once("__")?;
        let index = self
            .upstreams
            .iter()
            .position(|upstream| upstream.config.name == server)?;
        self.upstreams[index]
            .tools
            .contains_key(tool)
            .then(|| (index, tool.to_owned()))
    }

    fn policy_allows(&self, capabilities: &[Capability]) -> Result<(), String> {
        for capability in capabilities {
            let request = PolicyRequest {
                capability: *capability,
                paths: Vec::new(),
                approval: None,
            };
            if let PolicyDecision::Denied(violation) =
                PolicyEngine.evaluate(&self.task, &request, &[])
            {
                return Err(violation.to_string());
            }
        }
        Ok(())
    }

    /// Whether this task may call `tool`, with the policy's reason when not.
    fn availability(&self, tool: &str) -> Result<(), String> {
        if let Some((index, upstream_tool)) = self.upstream_tool(tool) {
            let mapped = self.upstreams[index].config.tools[&upstream_tool];
            return self.policy_allows(&[Capability::UseMcpTools, mapped]);
        }
        let (_, required) = TOOL_POLICY
            .iter()
            .find(|(name, _)| *name == tool)
            .ok_or_else(|| format!("unknown tool: {tool}"))?;
        self.policy_allows(required)?;
        if tool == "run_gate" {
            if self.task.required_gates.is_empty() {
                return Err("the task requires no gates".into());
            }
            if self.root.is_none() {
                return Err("no project root is known, so no gate profiles are available".into());
            }
        }
        Ok(())
    }

    fn descriptor(&self, tool: &str) -> Value {
        let no_arguments =
            json!({ "type": "object", "properties": {}, "additionalProperties": false });
        let read_only = json!({ "readOnlyHint": true, "openWorldHint": false });
        match tool {
            "task_contract" => json!({
                "name": tool,
                "title": "Task contract",
                "description": "The task's goal, allowed and forbidden paths, required gates, \
                                capabilities, and approvals.",
                "inputSchema": no_arguments,
                "annotations": read_only,
            }),
            "check_changes" => json!({
                "name": tool,
                "title": "Check changes against the task scope",
                "description": "Lists every changed path in the worktree (tracked and untracked) \
                                and whether the task may change it, with the policy's reason when \
                                not. Run it before you finish.",
                "inputSchema": no_arguments,
                "annotations": read_only,
            }),
            _ => json!({
                "name": tool,
                "title": "Run a required gate",
                "description": "Runs one of the task's required gates in the worktree and returns \
                                its outcome and the end of its output. Advisory: AgentForge runs \
                                the gates again after you finish.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "gate": {
                            "type": "string",
                            "enum": self.task.required_gates,
                            "description": "The gate to run.",
                        }
                    },
                    "required": ["gate"],
                    "additionalProperties": false,
                },
                "annotations": {
                    "readOnlyHint": false,
                    "destructiveHint": false,
                    "idempotentHint": true,
                    "openWorldHint": false,
                },
            }),
        }
    }

    fn call(&mut self, id: &Value, params: &Value) -> Value {
        let Some(tool) = params.get("name").and_then(Value::as_str) else {
            return error(id, INVALID_PARAMS, "tools/call needs a tool name");
        };
        let arguments = params
            .get("arguments")
            .cloned()
            .unwrap_or_else(|| json!({}));
        if let Err(reason) = self.availability(tool) {
            // Denied calls are recorded too; a denial that cannot be recorded is still refused.
            let _ = self.record(tool, "denied", &reason, &[]);
            return error(
                id,
                INVALID_PARAMS,
                &format!("tool {tool} is not available to this task: {reason}"),
            );
        }
        // Fail closed: a call that cannot be recorded does not run.
        let audit = match self.open_audit() {
            Ok(audit) => audit,
            Err(reason) => {
                return error(
                    id,
                    INTERNAL_ERROR,
                    &format!("cannot record the call: {reason}"),
                );
            }
        };
        drop(audit);
        if let Some((index, upstream_tool)) = self.upstream_tool(tool) {
            return self.call_upstream(id, tool, index, &upstream_tool, &arguments);
        }
        let outcome = match tool {
            "task_contract" => Ok(self.task_contract()),
            "check_changes" => self.check_changes(),
            _ => self.run_gate(&arguments),
        };
        let (result, fields) = match outcome {
            Ok(ToolOutput { text, data, fields }) => (
                json!({
                    "content": [{ "type": "text", "text": text }],
                    "structuredContent": data,
                    "isError": false,
                }),
                fields,
            ),
            Err(reason) => (
                json!({ "content": [{ "type": "text", "text": reason }], "isError": true }),
                vec![("outcome".to_owned(), "error".to_owned())],
            ),
        };
        let reason = if result["isError"] == true {
            result["content"][0]["text"].as_str().unwrap_or_default()
        } else {
            ""
        }
        .to_owned();
        if let Err(failure) = self.record(tool, "allowed", &reason, &fields) {
            return error(
                id,
                INTERNAL_ERROR,
                &format!("the call ran but could not be recorded: {failure}"),
            );
        }
        success(id, result)
    }

    /// Forwards one call to an external server and returns its result unchanged; failures become
    /// `isError` results. Recorded like native calls, with `server` and `upstream_tool` (P3-M006).
    fn call_upstream(
        &mut self,
        id: &Value,
        tool: &str,
        index: usize,
        upstream_tool: &str,
        arguments: &Value,
    ) -> Value {
        let server = self.upstreams[index].config.name.clone();
        let (result, outcome, reason) = match self.upstreams[index].call(upstream_tool, arguments) {
            Ok(result) => {
                let failed = result.get("isError") == Some(&Value::Bool(true));
                let outcome = if failed { "error" } else { "ok" };
                (result, outcome, String::new())
            }
            Err(failure) => {
                let outcome = if failure == UpstreamError::Timeout {
                    "timeout"
                } else {
                    "error"
                };
                let text = format!("{server}: {failure}");
                (
                    json!({ "content": [{ "type": "text", "text": text }], "isError": true }),
                    outcome,
                    text,
                )
            }
        };
        let fields = [
            ("server".to_owned(), server),
            ("upstream_tool".to_owned(), upstream_tool.to_owned()),
            ("outcome".to_owned(), outcome.to_owned()),
        ];
        if let Err(failure) = self.record(tool, "allowed", &reason, &fields) {
            return error(
                id,
                INTERNAL_ERROR,
                &format!("the call ran but could not be recorded: {failure}"),
            );
        }
        success(id, result)
    }

    fn task_contract(&self) -> ToolOutput {
        let task = &self.task;
        let names = |capabilities: &[Capability]| {
            capabilities
                .iter()
                .map(|capability| capability.as_str())
                .collect::<Vec<_>>()
        };
        let data = json!({
            "task_id": task.task_id,
            "milestone_id": task.milestone_id,
            "role": task.primary_role.as_str(),
            "goal": task.goal,
            "non_goals": task.non_goals,
            "allowed_paths": task.allowed_paths,
            "forbidden_paths": task.forbidden_paths,
            "required_gates": task.required_gates,
            "capabilities": names(&task.capabilities),
            "required_approvals": task
                .required_approvals
                .iter()
                .map(|boundary| boundary.as_str())
                .collect::<Vec<_>>(),
            "expected_outputs": task.expected_outputs,
        });
        ToolOutput {
            text: serde_json::to_string_pretty(&data).unwrap_or_default(),
            data,
            fields: vec![("outcome".to_owned(), "ok".to_owned())],
        }
    }

    fn check_changes(&self) -> Result<ToolOutput, String> {
        let output = Command::new("git")
            .current_dir(&self.worktree)
            .args([
                "status",
                "--porcelain=v1",
                "-z",
                "--untracked-files=all",
                "--no-renames",
            ])
            .output()
            .map_err(|error| format!("cannot run git status: {error}"))?;
        if !output.status.success() {
            return Err(format!(
                "git status failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
        let mut changes = Vec::new();
        let mut denied = 0_usize;
        let mut total = 0_usize;
        for entry in output.stdout.split(|byte| *byte == 0) {
            if entry.len() < 4 {
                continue;
            }
            total += 1;
            let status = String::from_utf8_lossy(&entry[..2]).trim().to_owned();
            let path = String::from_utf8_lossy(&entry[3..]).into_owned();
            let request = PolicyRequest {
                capability: Capability::WriteOwnedPaths,
                paths: vec![path.clone()],
                approval: None,
            };
            let verdict = PolicyEngine.evaluate(&self.task, &request, &[]);
            let reason = match &verdict {
                PolicyDecision::Allowed => None,
                PolicyDecision::Denied(violation) => {
                    denied += 1;
                    Some(violation.to_string())
                }
            };
            if changes.len() < MAX_REPORTED_CHANGES {
                changes.push(json!({
                    "path": path,
                    "status": status,
                    "allowed": reason.is_none(),
                    "reason": reason,
                }));
            }
        }
        let summary = if total == 0 {
            "no changes".to_owned()
        } else if denied == 0 {
            format!("{total} changed path(s), all within the task's scope")
        } else {
            format!("{total} changed path(s), {denied} outside what the task may change")
        };
        let mut text = summary.clone();
        for change in &changes {
            if change["allowed"] == false {
                text.push_str(&format!(
                    "\n- {}: {}",
                    change["path"].as_str().unwrap_or_default(),
                    change["reason"].as_str().unwrap_or_default()
                ));
            }
        }
        Ok(ToolOutput {
            text,
            data: json!({
                "summary": summary,
                "total": total,
                "denied": denied,
                "truncated": total > changes.len(),
                "changes": changes,
            }),
            fields: vec![
                (
                    "outcome".to_owned(),
                    if denied == 0 { "clean" } else { "violations" }.to_owned(),
                ),
                ("changed".to_owned(), total.to_string()),
                ("denied".to_owned(), denied.to_string()),
            ],
        })
    }

    fn run_gate(&self, arguments: &Value) -> Result<ToolOutput, String> {
        let gate = arguments
            .get("gate")
            .and_then(Value::as_str)
            .ok_or("run_gate needs a gate name")?;
        if !self.task.required_gates.iter().any(|name| name == gate) {
            return Err(format!(
                "gate {gate} is not required by this task (required: {})",
                self.task.required_gates.join(", ")
            ));
        }
        let root = self.root.as_ref().ok_or("no project root is known")?;
        let definition = GateProfileStore::new(root)
            .load(gate)
            .map_err(|error| format!("cannot load gate {gate}: {error}"))?;
        let report = GateRunner
            .run(&definition, &self.worktree)
            .map_err(|error| format!("gate {gate} could not run: {error}"))?;
        let outcome = match report.outcome() {
            GateOutcome::Passed => "passed",
            GateOutcome::Failed => "failed",
            GateOutcome::TimedOut => "timed_out",
            GateOutcome::OutputLimitExceeded => "output_limit_exceeded",
        };
        let exit = report
            .exit_code()
            .map_or_else(|| "none".to_owned(), |code| code.to_string());
        let (stdout, stderr) = (tail(report.stdout()), tail(report.stderr()));
        Ok(ToolOutput {
            text: format!(
                "gate {gate}: {outcome} (exit {exit})\n--- stdout (end) ---\n{stdout}\n--- stderr \
                 (end) ---\n{stderr}"
            ),
            data: json!({
                "gate": gate,
                "outcome": outcome,
                "exit_code": report.exit_code(),
                "output_truncated": report.output_truncated(),
                "stdout_tail": stdout,
                "stderr_tail": stderr,
            }),
            fields: vec![
                ("gate".to_owned(), gate.to_owned()),
                ("outcome".to_owned(), outcome.to_owned()),
            ],
        })
    }

    /// Opens the project audit log when the project is known; `Ok(None)` without a root.
    fn open_audit(&self) -> Result<Option<FileAuditStore>, String> {
        let Some(root) = &self.root else {
            return Ok(None);
        };
        let path = root.join(AUDIT_RELATIVE_PATH);
        if !path.is_file() {
            // Same rule as the operator's: never create a log outside an initialized project.
            if !FileTaskStore::for_project_root(root).path().is_file() {
                return Err("the project's task snapshot is missing".into());
            }
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
            }
        }
        FileAuditStore::open(path)
            .map(Some)
            .map_err(|error| error.to_string())
    }

    /// Records one call as `ToolInvoked`, or logs it to stderr when no project is known.
    fn record(
        &self,
        tool: &str,
        decision: &str,
        reason: &str,
        fields: &[(String, String)],
    ) -> Result<(), String> {
        let Some(mut audit) = self.open_audit()? else {
            let _ = writeln!(
                io::stderr().lock(),
                "agentforge-mcp: {tool} {decision}{}",
                if reason.is_empty() {
                    String::new()
                } else {
                    format!(": {reason}")
                }
            );
            return Ok(());
        };
        let sequence = audit
            .records()
            .last()
            .map_or(1, |record| record.event().sequence() + 1);
        let mut event = AuditEvent::now(
            sequence,
            format!("tool-{tool}-{sequence}"),
            AuditEventKind::ToolInvoked,
            format!("mcp:{}", self.task.task_id),
        )
        .with_task_id(self.task.task_id.as_str())
        .with_field("tool", tool)
        .with_field("decision", decision)
        .with_field("channel", "mcp");
        if !reason.is_empty() {
            event = event.with_field("reason", bounded(reason));
        }
        for (key, value) in fields {
            event = event.with_field(key.as_str(), value.as_str());
        }
        audit
            .append(event)
            .map(|_| ())
            .map_err(|error| error.to_string())
    }
}

/// Result of one tool: text for the model, structured data, and audit fields.
struct ToolOutput {
    text: String,
    data: Value,
    fields: Vec<(String, String)>,
}

fn success(id: &Value, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

fn error(id: &Value, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

fn skip_rest_of_line(reader: &mut impl BufRead) -> io::Result<()> {
    loop {
        let buffer = reader.fill_buf()?;
        if buffer.is_empty() {
            return Ok(());
        }
        if let Some(position) = buffer.iter().position(|byte| *byte == b'\n') {
            reader.consume(position + 1);
            return Ok(());
        }
        let length = buffer.len();
        reader.consume(length);
    }
}

fn tail(bytes: &[u8]) -> String {
    let start = bytes.len().saturating_sub(MAX_GATE_TAIL_BYTES);
    String::from_utf8_lossy(&bytes[start..]).into_owned()
}

pub(crate) fn bounded(text: &str) -> String {
    text.chars().take(256).collect()
}

/// Loads the task for a gateway from an `agentforge-task-prompt-v1` document on disk.
///
/// # Errors
///
/// Returns a message when the file cannot be read or is not a valid task contract.
pub fn load_contract(path: impl AsRef<Path>) -> Result<AgentTask, String> {
    let path = path.as_ref();
    let bytes =
        std::fs::read(path).map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    agentforge_adapter::parse_task_prompt(&bytes)
        .map_err(|error| format!("invalid task contract {}: {error}", path.display()))
}
