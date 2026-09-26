//! External MCP servers behind the gateway (P3-M006): mapping, policy, forwarding, containment,
//! and audit, against a deliberately awkward fixture server.

use agentforge_audit::{AuditEventKind, AuditStore, FileAuditStore};
use agentforge_core::agent::{AgentRole, AgentTask, Capability};
use agentforge_core::task::TaskGraph;
use agentforge_mcp::Gateway;
use agentforge_state::{FileTaskStore, TaskStore};
use serde_json::{Value, json};
use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

static SEQUENCE: AtomicU64 = AtomicU64::new(0);
const FIXTURE: &str = env!("CARGO_BIN_EXE_agentforge-mcp-fixture");

fn task(capabilities: &[Capability]) -> AgentTask {
    let mut task = AgentTask::new(
        "P3-M006-T0001",
        "P3-M006",
        AgentRole::Implementer,
        "upstream fixture",
    );
    task.capabilities = capabilities.to_vec();
    task.allowed_paths = vec!["src".into()];
    task
}

const MCP_READ: &[Capability] = &[Capability::UseMcpTools, Capability::ReadRepository];

/// A project root with a task snapshot (so calls are audited) and one declared server.
fn project(config: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "agentforge-mcp-upstream-{}-{}",
        std::process::id(),
        SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join(".forge/mcp")).expect("root");
    FileTaskStore::for_project_root(&root)
        .save(&TaskGraph::from_tasks([task(MCP_READ)]).expect("graph"))
        .expect("snapshot");
    fs::write(root.join(".forge/mcp/fix.conf"), config).expect("config");
    root
}

fn declaration(extra: &str) -> String {
    format!(
        "version=1\nexecutable={FIXTURE}\ntimeout_ms=2000\nenv.FIXTURE_LITERAL=literal-value\n\
         pass_env=CARGO_MANIFEST_DIR\ntool.echo=read_repository\ntool.slow=read_repository\n\
         tool.fail=read_repository\ntool.rpc_error=read_repository\ntool.crash=read_repository\n\
         tool.env=read_repository\n{extra}"
    )
}

fn session(gateway: &mut Gateway, messages: &[Value]) -> Vec<Value> {
    let input = messages
        .iter()
        .map(|message| format!("{message}\n"))
        .collect::<String>();
    let mut output = Vec::new();
    gateway
        .serve(Cursor::new(input.into_bytes()), &mut output)
        .expect("serve");
    String::from_utf8(output)
        .expect("utf8")
        .lines()
        .map(|line| serde_json::from_str(line).expect("every output line is JSON"))
        .collect()
}

fn initialize() -> Value {
    json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize",
            "params": { "protocolVersion": "2025-06-18", "capabilities": {},
                        "clientInfo": { "name": "test", "version": "0" } } })
}

fn list() -> Value {
    json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" })
}

fn call(id: u64, tool: &str, arguments: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "method": "tools/call",
            "params": { "name": tool, "arguments": arguments } })
}

fn names(response: &Value) -> Vec<String> {
    response["result"]["tools"]
        .as_array()
        .expect("tools")
        .iter()
        .map(|tool| tool["name"].as_str().expect("name").to_owned())
        .collect()
}

fn tool_events(root: &Path) -> Vec<String> {
    FileAuditStore::open(root.join(".forge/audit.log"))
        .map(|audit| {
            audit
                .records()
                .iter()
                .map(|record| record.event())
                .filter(|event| event.kind() == AuditEventKind::ToolInvoked)
                .map(|event| {
                    let field = |key: &str| event.fields().get(key).map_or("-", String::as_str);
                    format!(
                        "{} {} server={} upstream={} outcome={}",
                        field("tool"),
                        field("decision"),
                        field("server"),
                        field("upstream_tool"),
                        field("outcome")
                    )
                })
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn only_mapped_upstream_tools_are_listed_with_their_schema() {
    let root = project(&declaration(""));
    let mut gateway = Gateway::new(task(MCP_READ), &root, Some(root.clone()));
    let responses = session(&mut gateway, &[initialize(), list()]);
    let listed = names(&responses[1]);
    assert_eq!(
        listed,
        [
            "task_contract",
            "check_changes",
            "fix__crash",
            "fix__echo",
            "fix__env",
            "fix__fail",
            "fix__rpc_error",
            "fix__slow",
        ],
        "both pages read; `leak` is unmapped, so invisible"
    );
    let echo = responses[1]["result"]["tools"]
        .as_array()
        .expect("tools")
        .iter()
        .find(|tool| tool["name"] == "fix__echo")
        .expect("echo");
    assert_eq!(echo["description"], "[fix] fixture echo");
    assert_eq!(echo["inputSchema"], json!({ "type": "object" }));
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn calls_are_forwarded_unchanged_and_audited() {
    let root = project(&declaration(""));
    let mut gateway = Gateway::new(task(MCP_READ), &root, Some(root.clone()));
    let arguments = json!({ "text": "hello", "nested": { "n": 7 } });
    let responses = session(
        &mut gateway,
        &[
            initialize(),
            call(2, "fix__echo", arguments.clone()),
            call(3, "fix__fail", json!({})),
            call(4, "fix__rpc_error", json!({})),
            call(5, "fix__leak", json!({})),
            call(6, "fix__env", json!({})),
        ],
    );
    assert_eq!(responses[1]["result"]["structuredContent"], arguments);
    assert_eq!(
        responses[1]["result"]["content"][0]["text"],
        arguments.to_string()
    );
    assert_eq!(
        responses[2]["result"]["isError"], true,
        "upstream isError passes through"
    );
    assert_eq!(responses[2]["result"]["content"][0]["text"], "bad input");
    assert_eq!(responses[3]["result"]["isError"], true);
    assert!(
        responses[3]["result"]["content"][0]["text"]
            .as_str()
            .expect("text")
            .contains("upstream exploded")
    );
    assert_eq!(
        responses[4]["error"]["code"], -32_602,
        "unmapped is refused"
    );
    let env = &responses[5]["result"]["structuredContent"];
    assert_eq!(env["literal"], "literal-value");
    assert_eq!(
        env["passed"],
        env!("CARGO_MANIFEST_DIR"),
        "pass_env copies the named variable"
    );
    assert_eq!(env["not_passed"], Value::Null, "nothing else is inherited");
    assert_eq!(
        tool_events(&root),
        [
            "fix__echo allowed server=fix upstream=echo outcome=ok",
            "fix__fail allowed server=fix upstream=fail outcome=error",
            "fix__rpc_error allowed server=fix upstream=rpc_error outcome=error",
            "fix__leak denied server=- upstream=- outcome=-",
            "fix__env allowed server=fix upstream=env outcome=ok",
        ]
    );
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn the_mapped_capability_is_required() {
    let root = project(&declaration("tool.leak=read_github\n"));
    let mut gateway = Gateway::new(task(MCP_READ), &root, Some(root.clone()));
    let responses = session(
        &mut gateway,
        &[initialize(), list(), call(3, "fix__leak", json!({}))],
    );
    assert!(!names(&responses[1]).contains(&"fix__leak".to_owned()));
    assert_eq!(responses[2]["error"]["code"], -32_602);
    assert!(
        responses[2]["error"]["message"]
            .as_str()
            .expect("message")
            .contains("missing capability: read_github")
    );
    // Without use_mcp_tools nothing upstream is available at all.
    let mut gateway = Gateway::new(
        task(&[Capability::ReadRepository, Capability::ReadGitHub]),
        &root,
        Some(root.clone()),
    );
    let responses = session(&mut gateway, &[initialize(), list()]);
    assert!(names(&responses[1]).is_empty());
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn a_stalled_call_times_out_and_the_gateway_stays_responsive() {
    let root = project(&declaration(""));
    let mut gateway = Gateway::new(task(MCP_READ), &root, Some(root.clone()));
    let started = Instant::now();
    let responses = session(
        &mut gateway,
        &[
            initialize(),
            // Just past the 2 s timeout: the fixture answers one request at a time, so it then
            // sends its late "slept" answer before echo's, and the gateway must skip it.
            call(2, "fix__slow", json!({ "ms": 2500 })),
            call(3, "task_contract", json!({})),
            call(4, "fix__echo", json!({ "after": "timeout" })),
        ],
    );
    assert!(
        started.elapsed() < Duration::from_secs(8),
        "bounded by timeout_ms"
    );
    assert_eq!(responses[1]["result"]["isError"], true);
    assert!(
        responses[1]["result"]["content"][0]["text"]
            .as_str()
            .expect("text")
            .contains("did not answer in time")
    );
    assert_eq!(
        responses[2]["result"]["isError"], false,
        "native tools still work"
    );
    // The late "slept" answer is skipped; the next call gets its own answer.
    assert_eq!(
        responses[3]["result"]["structuredContent"],
        json!({ "after": "timeout" })
    );
    assert!(tool_events(&root)[0].ends_with("outcome=timeout"));
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn a_crashing_server_is_contained() {
    let root = project(&declaration(""));
    let mut gateway = Gateway::new(task(MCP_READ), &root, Some(root.clone()));
    let responses = session(
        &mut gateway,
        &[
            initialize(),
            call(2, "fix__crash", json!({})),
            call(3, "fix__echo", json!({})),
            call(4, "task_contract", json!({})),
        ],
    );
    for index in [1, 2] {
        assert_eq!(responses[index]["result"]["isError"], true, "{index}");
        assert!(
            responses[index]["result"]["content"][0]["text"]
                .as_str()
                .expect("text")
                .contains("unavailable"),
            "{:?}",
            responses[index]
        );
    }
    assert_eq!(
        responses[3]["result"]["isError"], false,
        "native tools still work"
    );
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn servers_that_cannot_start_are_skipped() {
    let root = project(&declaration(""));
    let broken = [
        (
            "exits",
            format!(
                "version=1\nexecutable={FIXTURE}\nargument=--exit-at-start\ntool.echo=read_repository\n"
            ),
        ),
        (
            "silent",
            format!(
                "version=1\nexecutable={FIXTURE}\nargument=--silent\ntimeout_ms=300\ntool.echo=read_repository\n"
            ),
        ),
        (
            "missing",
            "version=1\nexecutable=/nonexistent/server\ntool.echo=read_repository\n".to_owned(),
        ),
        ("invalid", "version=1\nexecutable=relative\n".to_owned()),
    ];
    for (name, text) in broken {
        fs::write(root.join(format!(".forge/mcp/{name}.conf")), text).expect("config");
    }
    let mut gateway = Gateway::new(task(MCP_READ), &root, Some(root.clone()));
    let responses = session(&mut gateway, &[initialize(), list()]);
    let listed = names(&responses[1]);
    assert!(listed.contains(&"fix__echo".to_owned()), "{listed:?}");
    assert!(
        !listed.iter().any(|name| name.starts_with("exits__")
            || name.starts_with("silent__")
            || name.starts_with("missing__")
            || name.starts_with("invalid__")),
        "{listed:?}"
    );
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn without_a_project_no_servers_are_started() {
    let root = project(&declaration(""));
    let mut gateway = Gateway::new(task(MCP_READ), &root, None);
    let responses = session(&mut gateway, &[initialize(), list()]);
    assert_eq!(names(&responses[1]), ["task_contract", "check_changes"]);
    fs::remove_dir_all(root).expect("cleanup");
}
