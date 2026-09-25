//! `forge mcp serve` over real pipes (P3-M005).

use agentforge_audit::{AuditEventKind, AuditStore, FileAuditStore};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn forge(root: &Path, arguments: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_forge"))
        .args(arguments)
        .current_dir(root)
        .output()
        .expect("forge command")
}

fn git(root: &Path, arguments: &[&str]) {
    let output = Command::new("git")
        .args(arguments)
        .current_dir(root)
        .output()
        .expect("git command");
    assert!(output.status.success(), "git {arguments:?}: {output:?}");
}

fn project() -> PathBuf {
    let root = std::env::temp_dir().join(format!("agentforge-cli-mcp-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("root");
    git(&root, &["init", "-q"]);
    git(&root, &["config", "user.name", "AgentForge Test"]);
    git(
        &root,
        &["config", "user.email", "agentforge@example.invalid"],
    );
    fs::write(root.join(".gitignore"), ".forge/\n").expect("gitignore");
    git(&root, &["add", ".gitignore"]);
    git(&root, &["commit", "-qm", "init"]);
    let root_text = root.to_str().expect("root");
    assert!(forge(&root, &["init", root_text]).status.success());
    let created = forge(
        &root,
        &[
            "task",
            "create",
            root_text,
            "P3-M005-T0001",
            "P3-M005",
            "implementer",
            "use the gateway",
            "--allowed",
            "src",
            "--capability",
            "use_mcp_tools",
            "--capability",
            "read_repository",
            "--capability",
            "write_owned_paths",
        ],
    );
    assert!(created.status.success(), "{created:?}");
    root
}

#[test]
fn forge_mcp_serve_speaks_mcp_on_stdout_only() {
    let root = project();
    let task = agentforge_state::TaskStore::load(
        &agentforge_state::FileTaskStore::for_project_root(&root),
    )
    .expect("load")
    .expect("snapshot")
    .records()
    .next()
    .expect("task")
    .task()
    .clone();
    let contract = root.join("contract.txt");
    fs::write(&contract, agentforge_adapter::render_task_prompt(&task)).expect("contract");
    fs::write(root.join("stray.txt"), "outside\n").expect("change");

    let mut child = Command::new(env!("CARGO_BIN_EXE_forge"))
        .args([
            "mcp",
            "serve",
            "--contract",
            contract.to_str().expect("contract"),
            "--worktree",
            root.to_str().expect("worktree"),
            "--root",
            root.to_str().expect("root"),
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn");
    let requests = [
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"cli-test","version":"0"}}}"#,
        r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
        r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#,
        r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"check_changes","arguments":{}}}"#,
    ];
    {
        let mut stdin = child.stdin.take().expect("stdin");
        for request in requests {
            writeln!(stdin, "{request}").expect("write");
        }
    }
    let output = child.wait_with_output().expect("wait");
    assert!(output.status.success(), "{output:?}");
    let stdout = String::from_utf8(output.stdout).expect("utf8");
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(
        lines.len(),
        3,
        "one response per request, none for the notification:\n{stdout}"
    );
    for line in &lines {
        assert!(
            line.starts_with(r#"{"id":"#) || line.starts_with(r#"{"jsonrpc":"2.0""#),
            "{line}"
        );
        assert!(line.contains(r#""jsonrpc":"2.0""#), "{line}");
    }
    assert!(
        lines[0].contains(r#""protocolVersion":"2025-06-18""#),
        "{}",
        lines[0]
    );
    assert!(
        lines[1].contains(r#""name":"task_contract""#),
        "{}",
        lines[1]
    );
    assert!(
        lines[1].contains(r#""name":"check_changes""#),
        "{}",
        lines[1]
    );
    assert!(
        !lines[1].contains("run_gate"),
        "no run_local_commands: {}",
        lines[1]
    );
    assert!(
        lines[2].contains("path outside allowed scope: stray.txt"),
        "{}",
        lines[2]
    );

    let audit = FileAuditStore::open(root.join(".forge/audit.log")).expect("audit");
    let tools: Vec<String> = audit
        .records()
        .iter()
        .map(|record| record.event())
        .filter(|event| event.kind() == AuditEventKind::ToolInvoked)
        .map(|event| format!("{} {}", event.fields()["tool"], event.fields()["outcome"]))
        .collect();
    assert_eq!(tools, ["check_changes violations"]);
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn forge_mcp_serve_refuses_bad_arguments() {
    let root = std::env::temp_dir();
    let missing = forge(&root, &["mcp", "serve", "--worktree", "."]);
    assert_eq!(missing.status.code(), Some(2));
    assert!(
        String::from_utf8_lossy(&missing.stderr).contains("--contract and --worktree are required")
    );
    let bad = forge(
        &root,
        &[
            "mcp",
            "serve",
            "--contract",
            "/nonexistent/contract",
            "--worktree",
            ".",
        ],
    );
    assert_eq!(bad.status.code(), Some(2));
    assert!(bad.stdout.is_empty(), "nothing on stdout");
}
