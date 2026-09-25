//! The MCP task-tool gateway (P3-M005): protocol, policy map, tools, and audit.

use agentforge_audit::{AuditEventKind, AuditStore, FileAuditStore};
use agentforge_core::agent::{AgentRole, AgentTask, Capability};
use agentforge_core::task::TaskGraph;
use agentforge_mcp::{Gateway, MAX_MESSAGE_BYTES, SUPPORTED_PROTOCOL_VERSIONS};
use agentforge_state::{FileTaskStore, TaskStore};
use serde_json::{Value, json};
use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

static SEQUENCE: AtomicU64 = AtomicU64::new(0);

const ALL: &[Capability] = &[
    Capability::UseMcpTools,
    Capability::ReadRepository,
    Capability::WriteOwnedPaths,
    Capability::RunLocalCommands,
];

fn task(capabilities: &[Capability]) -> AgentTask {
    let mut task = AgentTask::new(
        "P3-M005-T0001",
        "P3-M005",
        AgentRole::Implementer,
        "gateway fixture",
    );
    task.capabilities = capabilities.to_vec();
    task.allowed_paths = vec!["src".into(), "README.md".into()];
    task.forbidden_paths = vec!["src/secret".into()];
    task.required_gates = vec!["pass".into(), "fail".into()];
    task
}

fn git(directory: &Path, arguments: &[&str]) {
    let output = Command::new("git")
        .current_dir(directory)
        .args(arguments)
        .output()
        .expect("git");
    assert!(output.status.success(), "git {arguments:?}: {output:?}");
}

/// A committed repository standing in for the task worktree, and a project root with a task
/// snapshot (so calls are audited).
fn project() -> (PathBuf, PathBuf) {
    let base = std::env::temp_dir().join(format!(
        "agentforge-mcp-{}-{}",
        std::process::id(),
        SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = fs::remove_dir_all(&base);
    let worktree = base.join("worktree");
    fs::create_dir_all(worktree.join("src")).expect("worktree");
    git(&worktree, &["init", "-q"]);
    git(&worktree, &["config", "user.name", "AgentForge Test"]);
    git(
        &worktree,
        &["config", "user.email", "agentforge@example.invalid"],
    );
    fs::write(worktree.join("README.md"), "fixture\n").expect("readme");
    git(&worktree, &["add", "README.md"]);
    git(&worktree, &["commit", "-qm", "init"]);
    let root = base.join("root");
    fs::create_dir_all(&root).expect("root");
    FileTaskStore::for_project_root(&root)
        .save(&TaskGraph::from_tasks([task(ALL)]).expect("graph"))
        .expect("snapshot");
    (worktree, root)
}

fn cleanup(worktree: &Path) {
    let _ = fs::remove_dir_all(worktree.parent().expect("base"));
}

/// Runs a whole session through `serve` and returns every response line as JSON.
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

fn request(id: u64, method: &str, params: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params })
}

fn initialize(version: &str) -> Value {
    request(
        1,
        "initialize",
        json!({ "protocolVersion": version, "capabilities": {}, "clientInfo": { "name": "test", "version": "0" } }),
    )
}

fn initialized() -> Value {
    json!({ "jsonrpc": "2.0", "method": "notifications/initialized" })
}

fn call(id: u64, tool: &str, arguments: Value) -> Value {
    request(
        id,
        "tools/call",
        json!({ "name": tool, "arguments": arguments }),
    )
}

fn tool_names(response: &Value) -> Vec<String> {
    response["result"]["tools"]
        .as_array()
        .expect("tools")
        .iter()
        .map(|tool| tool["name"].as_str().expect("name").to_owned())
        .collect()
}

fn tool_events(root: &Path) -> Vec<String> {
    let path = root.join(".forge/audit.log");
    if !path.exists() {
        return Vec::new();
    }
    FileAuditStore::open(path)
        .expect("audit")
        .records()
        .iter()
        .map(|record| record.event())
        .filter(|event| event.kind() == AuditEventKind::ToolInvoked)
        .map(|event| {
            let field = |key: &str| event.fields().get(key).map_or("-", String::as_str);
            format!(
                "{} {} {} {} outcome={}",
                event.actor(),
                field("tool"),
                field("decision"),
                field("channel"),
                field("outcome")
            )
        })
        .collect()
}

#[test]
fn initialize_negotiates_the_protocol_version() {
    let (worktree, root) = project();
    for version in SUPPORTED_PROTOCOL_VERSIONS {
        let mut gateway = Gateway::new(task(ALL), &worktree, Some(root.clone()));
        let responses = session(&mut gateway, &[initialize(version)]);
        assert_eq!(responses[0]["result"]["protocolVersion"], *version);
        assert_eq!(responses[0]["result"]["serverInfo"]["name"], "agentforge");
        assert_eq!(
            responses[0]["result"]["capabilities"]["tools"]["listChanged"],
            false
        );
    }
    // An unknown version is answered with the newest supported one.
    let mut gateway = Gateway::new(task(ALL), &worktree, Some(root));
    let responses = session(&mut gateway, &[initialize("1999-01-01")]);
    assert_eq!(
        responses[0]["result"]["protocolVersion"],
        SUPPORTED_PROTOCOL_VERSIONS[0]
    );
    cleanup(&worktree);
}

#[test]
fn the_protocol_refuses_what_it_does_not_speak() {
    let (worktree, root) = project();
    let mut gateway = Gateway::new(task(ALL), &worktree, Some(root.clone()));
    let input = [
        // Before initialize: only ping (and initialize) are served.
        request(1, "tools/list", json!({})).to_string(),
        request(2, "ping", json!({})).to_string(),
        initialize("2025-06-18").to_string(),
        initialized().to_string(),
        "{not json".to_owned(),
        json!([request(3, "ping", json!({}))]).to_string(),
        json!({ "jsonrpc": "1.0", "id": 4, "method": "ping" }).to_string(),
        request(5, "resources/list", json!({})).to_string(),
        json!({ "jsonrpc": "2.0", "id": 6.5, "method": "ping" }).to_string(),
        // A notification with an unknown method gets no answer at all.
        json!({ "jsonrpc": "2.0", "method": "notifications/unknown" }).to_string(),
    ]
    .join("\n");
    let mut output = Vec::new();
    gateway
        .serve(Cursor::new(format!("{input}\n").into_bytes()), &mut output)
        .expect("serve");
    let responses: Vec<Value> = String::from_utf8(output)
        .expect("utf8")
        .lines()
        .map(|line| serde_json::from_str(line).expect("json"))
        .collect();
    let codes: Vec<Value> = responses
        .iter()
        .map(|response| response["error"]["code"].clone())
        .collect();
    assert_eq!(
        codes,
        [
            json!(-32_600), // not initialized
            Value::Null,    // ping
            Value::Null,    // initialize
            json!(-32_700), // parse error
            json!(-32_600), // batch
            json!(-32_600), // wrong jsonrpc
            json!(-32_601), // unknown method
            json!(-32_600), // fractional id
        ],
        "{responses:#?}"
    );
    assert_eq!(responses[1]["result"], json!({}));
    assert!(tool_events(&root).is_empty(), "no tool calls, no audit");
    cleanup(&worktree);
}

#[test]
fn an_oversized_message_is_refused_and_the_session_continues() {
    let (worktree, root) = project();
    let mut gateway = Gateway::new(task(ALL), &worktree, Some(root));
    let huge = format!(
        "{{\"jsonrpc\":\"2.0\",\"id\":9,\"method\":\"ping\",\"params\":{{\"pad\":\"{}\"}}}}",
        "x".repeat(MAX_MESSAGE_BYTES)
    );
    let input = format!("{huge}\n{}\n", request(10, "ping", json!({})));
    let mut output = Vec::new();
    gateway
        .serve(Cursor::new(input.into_bytes()), &mut output)
        .expect("serve");
    let responses: Vec<Value> = String::from_utf8(output)
        .expect("utf8")
        .lines()
        .map(|line| serde_json::from_str(line).expect("json"))
        .collect();
    assert_eq!(responses.len(), 2);
    assert_eq!(responses[0]["error"]["code"], -32_600);
    assert_eq!(responses[1]["id"], 10);
    cleanup(&worktree);
}

#[test]
fn tools_are_listed_by_capability() {
    let (worktree, root) = project();
    let list = |capabilities: &[Capability], root: Option<PathBuf>| {
        let mut gateway = Gateway::new(task(capabilities), &worktree, root);
        let responses = session(
            &mut gateway,
            &[
                initialize("2025-06-18"),
                initialized(),
                request(2, "tools/list", json!({})),
            ],
        );
        tool_names(&responses[1])
    };
    assert_eq!(
        list(ALL, Some(root.clone())),
        ["task_contract", "check_changes", "run_gate"]
    );
    assert!(
        list(
            &[Capability::ReadRepository, Capability::RunLocalCommands],
            Some(root.clone())
        )
        .is_empty(),
        "no use_mcp_tools, no tools"
    );
    assert_eq!(
        list(
            &[Capability::UseMcpTools, Capability::ReadRepository],
            Some(root.clone())
        ),
        ["task_contract", "check_changes"],
        "no run_local_commands, no run_gate"
    );
    assert_eq!(
        list(ALL, None),
        ["task_contract", "check_changes"],
        "without a project root there are no gate profiles"
    );
    cleanup(&worktree);
}

#[test]
fn unavailable_tools_are_refused_and_recorded() {
    let (worktree, root) = project();
    let mut gateway = Gateway::new(
        task(&[Capability::UseMcpTools, Capability::ReadRepository]),
        &worktree,
        Some(root.clone()),
    );
    let responses = session(
        &mut gateway,
        &[
            initialize("2025-06-18"),
            call(2, "run_gate", json!({ "gate": "pass" })),
            call(3, "rm_rf", json!({})),
        ],
    );
    assert_eq!(responses[1]["error"]["code"], -32_602);
    assert!(
        responses[1]["error"]["message"]
            .as_str()
            .expect("message")
            .contains("missing capability: run_local_commands"),
        "{responses:#?}"
    );
    assert_eq!(responses[2]["error"]["code"], -32_602);
    assert_eq!(
        tool_events(&root),
        [
            "mcp:P3-M005-T0001 run_gate denied mcp outcome=-",
            "mcp:P3-M005-T0001 rm_rf denied mcp outcome=-",
        ]
    );
    cleanup(&worktree);
}

#[test]
fn task_contract_returns_the_contract() {
    let (worktree, root) = project();
    let mut gateway = Gateway::new(task(ALL), &worktree, Some(root.clone()));
    let responses = session(
        &mut gateway,
        &[
            initialize("2025-06-18"),
            call(2, "task_contract", json!({})),
        ],
    );
    let result = &responses[1]["result"];
    assert_eq!(result["isError"], false);
    let contract = &result["structuredContent"];
    assert_eq!(contract["task_id"], "P3-M005-T0001");
    assert_eq!(contract["allowed_paths"], json!(["src", "README.md"]));
    assert_eq!(contract["forbidden_paths"], json!(["src/secret"]));
    assert_eq!(contract["required_gates"], json!(["pass", "fail"]));
    assert_eq!(
        contract["capabilities"],
        json!([
            "use_mcp_tools",
            "read_repository",
            "write_owned_paths",
            "run_local_commands"
        ])
    );
    let text: Value =
        serde_json::from_str(result["content"][0]["text"].as_str().expect("text")).expect("json");
    assert_eq!(&text, contract, "the text is the same contract");
    assert_eq!(
        tool_events(&root),
        ["mcp:P3-M005-T0001 task_contract allowed mcp outcome=ok"]
    );
    cleanup(&worktree);
}

#[test]
fn check_changes_judges_each_path_with_the_policy() {
    let (worktree, root) = project();
    let mut gateway = Gateway::new(task(ALL), &worktree, Some(root.clone()));
    let clean = session(
        &mut gateway,
        &[
            initialize("2025-06-18"),
            call(2, "check_changes", json!({})),
        ],
    );
    assert_eq!(clean[1]["result"]["structuredContent"]["total"], 0);

    fs::write(worktree.join("README.md"), "changed\n").expect("modify");
    fs::write(worktree.join("src/lib.rs"), "// new\n").expect("add");
    fs::create_dir_all(worktree.join("src/secret")).expect("secret dir");
    fs::write(worktree.join("src/secret/key.txt"), "x\n").expect("forbidden");
    fs::write(worktree.join("Cargo.toml"), "[package]\n").expect("outside");
    let mut gateway = Gateway::new(task(ALL), &worktree, Some(root.clone()));
    let responses = session(
        &mut gateway,
        &[
            initialize("2025-06-18"),
            call(2, "check_changes", json!({})),
        ],
    );
    let data = &responses[1]["result"]["structuredContent"];
    assert_eq!(data["total"], 4);
    assert_eq!(data["denied"], 2);
    let verdict = |path: &str| {
        data["changes"]
            .as_array()
            .expect("changes")
            .iter()
            .find(|change| change["path"] == path)
            .map(|change| (change["allowed"].clone(), change["reason"].clone()))
            .expect("path reported")
    };
    assert_eq!(verdict("README.md"), (json!(true), Value::Null));
    assert_eq!(verdict("src/lib.rs"), (json!(true), Value::Null));
    assert_eq!(
        verdict("src/secret/key.txt"),
        (json!(false), json!("forbidden path: src/secret/key.txt"))
    );
    assert_eq!(
        verdict("Cargo.toml"),
        (
            json!(false),
            json!("path outside allowed scope: Cargo.toml")
        )
    );
    let text = responses[1]["result"]["content"][0]["text"]
        .as_str()
        .expect("text");
    assert!(
        text.starts_with("4 changed path(s), 2 outside what the task may change"),
        "{text}"
    );
    assert_eq!(
        tool_events(&root),
        [
            "mcp:P3-M005-T0001 check_changes allowed mcp outcome=clean",
            "mcp:P3-M005-T0001 check_changes allowed mcp outcome=violations",
        ]
    );
    cleanup(&worktree);
}

#[test]
fn a_task_without_write_authority_may_change_nothing() {
    let (worktree, root) = project();
    fs::write(worktree.join("README.md"), "changed\n").expect("modify");
    let mut gateway = Gateway::new(
        task(&[Capability::UseMcpTools, Capability::ReadRepository]),
        &worktree,
        Some(root),
    );
    let responses = session(
        &mut gateway,
        &[
            initialize("2025-06-18"),
            call(2, "check_changes", json!({})),
        ],
    );
    let change = &responses[1]["result"]["structuredContent"]["changes"][0];
    assert_eq!(change["allowed"], false);
    assert_eq!(change["reason"], "missing capability: write_owned_paths");
    cleanup(&worktree);
}

#[cfg(unix)]
#[test]
fn run_gate_runs_only_required_gates_in_the_worktree() {
    let (worktree, root) = project();
    fs::create_dir_all(root.join(".forge/gates")).expect("gates");
    for (name, script) in [
        ("pass", "pwd; echo gate-ok"),
        ("fail", "echo broken >&2; exit 3"),
        ("extra", "exit 0"),
    ] {
        fs::write(
            root.join(format!(".forge/gates/{name}.conf")),
            format!(
                "version=1\nexecutable=/bin/sh\nargument=-c\nargument={script}\ntimeout_ms=10000\n\
                 max_output_bytes=65536\n"
            ),
        )
        .expect("gate profile");
    }
    let mut gateway = Gateway::new(task(ALL), &worktree, Some(root.clone()));
    let responses = session(
        &mut gateway,
        &[
            initialize("2025-06-18"),
            call(2, "run_gate", json!({ "gate": "pass" })),
            call(3, "run_gate", json!({ "gate": "fail" })),
            call(4, "run_gate", json!({ "gate": "extra" })),
            call(5, "run_gate", json!({})),
        ],
    );
    let passed = &responses[1]["result"];
    assert_eq!(passed["isError"], false);
    assert_eq!(passed["structuredContent"]["outcome"], "passed");
    let stdout = passed["structuredContent"]["stdout_tail"]
        .as_str()
        .expect("stdout");
    assert!(stdout.contains("gate-ok"), "{stdout}");
    assert!(
        stdout
            .lines()
            .next()
            .is_some_and(|pwd| Path::new(pwd) == worktree.canonicalize().expect("canonical")),
        "the gate runs in the worktree: {stdout}"
    );
    let failed = &responses[2]["result"]["structuredContent"];
    assert_eq!(failed["outcome"], "failed");
    assert_eq!(failed["exit_code"], 3);
    assert!(
        failed["stderr_tail"]
            .as_str()
            .expect("stderr")
            .contains("broken")
    );
    // A gate the task does not require is a tool error, not a policy denial.
    assert_eq!(responses[3]["result"]["isError"], true);
    assert!(
        responses[3]["result"]["content"][0]["text"]
            .as_str()
            .expect("text")
            .contains("gate extra is not required by this task")
    );
    assert_eq!(responses[4]["result"]["isError"], true);
    assert_eq!(
        tool_events(&root),
        [
            "mcp:P3-M005-T0001 run_gate allowed mcp outcome=passed",
            "mcp:P3-M005-T0001 run_gate allowed mcp outcome=failed",
            "mcp:P3-M005-T0001 run_gate allowed mcp outcome=error",
            "mcp:P3-M005-T0001 run_gate allowed mcp outcome=error",
        ]
    );
    cleanup(&worktree);
}

#[test]
fn without_a_project_nothing_is_written() {
    let (worktree, root) = project();
    let mut gateway = Gateway::new(task(ALL), &worktree, None);
    let responses = session(
        &mut gateway,
        &[
            initialize("2025-06-18"),
            call(2, "task_contract", json!({})),
        ],
    );
    assert_eq!(responses[1]["result"]["isError"], false);
    assert!(!root.join(".forge/audit.log").exists());
    cleanup(&worktree);
}

#[test]
fn a_call_that_cannot_be_recorded_does_not_run() {
    let (worktree, root) = project();
    // A root without a task snapshot: the audit log may not be created there.
    let bare = root.with_file_name("bare-root");
    fs::create_dir_all(&bare).expect("bare root");
    let mut gateway = Gateway::new(task(ALL), &worktree, Some(bare.clone()));
    let responses = session(
        &mut gateway,
        &[
            initialize("2025-06-18"),
            call(2, "task_contract", json!({})),
        ],
    );
    assert_eq!(responses[1]["error"]["code"], -32_603);
    assert!(
        responses[1]["error"]["message"]
            .as_str()
            .expect("message")
            .contains("cannot record the call"),
        "{responses:#?}"
    );
    assert!(!bare.join(".forge").exists());
    cleanup(&worktree);
}
