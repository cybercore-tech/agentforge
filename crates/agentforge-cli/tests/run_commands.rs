#![allow(missing_docs)]

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

static SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[test]
fn interactive_run_streams_cooked_input_and_output_without_changing_lifecycle() {
    let root = temporary_repo();
    let root_text = root.to_str().expect("root");
    let task_id = "P2-M012-T0001";

    assert!(forge(&root, &["init", root_text]).status.success());
    let created = forge(
        &root,
        &[
            "task",
            "create",
            root_text,
            task_id,
            "P2-M012",
            "implementer",
            "interactive session",
            "--allowed",
            "README.md",
            "--capability",
            "run_local_commands",
        ],
    );
    assert!(created.status.success(), "{created:?}");
    let prepared = forge(&root, &["worktree", "create", root_text, task_id, "HEAD"]);
    assert!(prepared.status.success(), "{prepared:?}");

    let profile_dir = root.join(".forge/agents");
    fs::create_dir_all(&profile_dir).expect("profile directory");
    let executable = PathBuf::from(env!("CARGO_BIN_EXE_agentforge-cli-fixture"));
    fs::write(
        profile_dir.join("interactive.conf"),
        format!(
            "version=1\nexecutable={}\nenv.AGENTFORGE_CLI_FIXTURE_MODE=interactive\ntimeout_ms=5000\nmax_output_bytes=4096\n",
            executable.display()
        ),
    )
    .expect("interactive profile");

    let mut child = Command::new(env!("CARGO_BIN_EXE_forge"))
        .args([
            "run",
            root_text,
            task_id,
            "--profile",
            "interactive",
            "--interactive",
        ])
        .current_dir(&root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("interactive forge command");
    child
        .stdin
        .take()
        .expect("interactive stdin")
        .write_all(b"operator follow-up\n")
        .expect("operator input");
    let output = child.wait_with_output().expect("interactive output");
    assert!(output.status.success(), "{output:?}");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("fixture interactive start"), "{stdout}");
    assert!(stdout.contains("operator follow-up"), "{stdout}");
    assert!(stdout.contains("task P2-M012-T0001 launched"), "{stdout}");

    let inspected = forge(&root, &["task", "inspect", root_text, task_id]);
    assert!(inspected.status.success(), "{inspected:?}");
    assert!(String::from_utf8_lossy(&inspected.stdout).contains("state=running"));
    let retired = forge(&root, &["worktree", "retire", root_text, task_id]);
    assert!(retired.status.success(), "{retired:?}");
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn pty_run_rejects_non_terminal_streams_before_child_spawn() {
    let root = temporary_repo();
    let root_text = root.to_str().expect("root");
    let task_id = "P2-M013-T0001";
    assert!(forge(&root, &["init", root_text]).status.success());
    let created = forge(
        &root,
        &[
            "task",
            "create",
            root_text,
            task_id,
            "P2-M013",
            "implementer",
            "PTY session",
            "--allowed",
            "README.md",
            "--capability",
            "run_local_commands",
        ],
    );
    assert!(created.status.success(), "{created:?}");
    let prepared = forge(&root, &["worktree", "create", root_text, task_id, "HEAD"]);
    assert!(prepared.status.success(), "{prepared:?}");
    let executable = env!("CARGO_BIN_EXE_agentforge-cli-fixture");
    let output = Command::new(env!("CARGO_BIN_EXE_forge"))
        .args([
            "run",
            root_text,
            task_id,
            executable,
            "--interactive",
            "--pty",
        ])
        .current_dir(&root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("PTY run");
    assert!(!output.status.success(), "{output:?}");
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("terminal unavailable"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let retired = forge(&root, &["worktree", "retire", root_text, task_id]);
    assert!(retired.status.success(), "{retired:?}");
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn pty_flag_requires_interactive_mode() {
    let root = temporary_repo();
    let root_text = root.to_str().expect("root");
    let output = forge_with_args(
        &root,
        &["run", root_text, "P2-M013-T0001", "/bin/true", "--pty"],
    );
    assert_eq!(output.status.code(), Some(2), "{output:?}");
    assert!(String::from_utf8_lossy(&output.stderr).contains("requires --interactive"));
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn pty_duplicate_and_daemon_flags_are_rejected() {
    let root = temporary_repo();
    let root_text = root.to_str().expect("root");
    let duplicate = forge_with_args(
        &root,
        &[
            "run",
            root_text,
            "P2-M013-T0001",
            "/bin/true",
            "--interactive",
            "--pty",
            "--pty",
        ],
    );
    assert_eq!(duplicate.status.code(), Some(2), "{duplicate:?}");
    assert!(String::from_utf8_lossy(&duplicate.stderr).contains("at most once"));
    let daemon = forge_with_args(
        &root,
        &[
            "daemon",
            "run",
            root_text,
            "P2-M013-T0001",
            "/bin/true",
            "--pty",
        ],
    );
    assert_eq!(daemon.status.code(), Some(2), "{daemon:?}");
    fs::remove_dir_all(root).expect("cleanup");
}

fn forge(root: &Path, arguments: &[&str]) -> std::process::Output {
    forge_with_args(root, arguments)
}

fn forge_with_args(root: &Path, arguments: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_forge"))
        .args(arguments)
        .current_dir(root)
        .output()
        .expect("forge command")
}

fn temporary_repo() -> PathBuf {
    let sequence = SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!(
        "agentforge-cli-run-{pid}-{sequence}",
        pid = std::process::id()
    ));
    fs::create_dir_all(&root).expect("temporary root");
    git(&root, &["init", "-q"]);
    fs::write(root.join("README.md"), "interactive fixture\n").expect("fixture");
    git(
        &root,
        &["config", "user.email", "agentforge@example.invalid"],
    );
    git(&root, &["config", "user.name", "AgentForge Test"]);
    git(&root, &["add", "README.md"]);
    git(&root, &["commit", "-qm", "fixture"]);
    root
}

fn git(root: &Path, arguments: &[&str]) {
    let output = Command::new("git")
        .args(arguments)
        .current_dir(root)
        .output()
        .expect("git command");
    assert!(output.status.success(), "git {arguments:?}: {output:?}");
}
