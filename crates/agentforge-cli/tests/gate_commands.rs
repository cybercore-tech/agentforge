#![allow(missing_docs)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static TEMPORARY_ROOT_COUNTER: AtomicU64 = AtomicU64::new(0);

#[test]
fn gate_list_is_read_only_and_deterministic() {
    let root = temporary_repo();
    let root_text = root.to_str().expect("root");
    let empty = forge(&root, &["gate", "list", root_text]);
    assert!(empty.status.success(), "{empty:?}");
    assert_eq!(String::from_utf8_lossy(&empty.stdout), "no gates\n");

    write_gate(&root, "zz-last", "");
    write_gate(&root, "aa-first", "argument=--check\n");
    let listed = forge(&root, &["gate", "list", root_text]);
    assert!(listed.status.success(), "{listed:?}");
    let stdout = String::from_utf8_lossy(&listed.stdout);
    let ids = stdout
        .lines()
        .map(|line| line.split_whitespace().nth(1).expect("id field"))
        .collect::<Vec<_>>();
    assert_eq!(ids, ["id=aa-first", "id=zz-last"], "{stdout}");
    assert!(stdout.contains("arguments=1"), "{stdout}");
    assert!(
        !root.join(".forge/state").exists(),
        "listing must not mutate"
    );

    fs::write(
        root.join(".forge/gates/broken.conf"),
        "version=1\nexecutable=relative\n",
    )
    .expect("broken gate");
    let rejected = forge(&root, &["gate", "list", root_text]);
    assert!(!rejected.status.success(), "{rejected:?}");
    assert!(
        String::from_utf8_lossy(&rejected.stderr).contains("broken"),
        "{}",
        String::from_utf8_lossy(&rejected.stderr)
    );
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn task_launch_runs_gates_and_fails_the_task_on_a_failing_gate() {
    let root = temporary_repo();
    let root_text = root.to_str().expect("root");
    assert!(forge(&root, &["init", root_text]).status.success());
    let created = forge(
        &root,
        &[
            "task",
            "create",
            root_text,
            "P1-M004-T0001",
            "P1-M004",
            "implementer",
            "gated task",
            "--allowed",
            "README.md",
            "--capability",
            "run_local_commands",
        ],
    );
    assert!(created.status.success(), "{created:?}");
    write_gate(&root, "a-passes", "");
    write_gate(&root, "b-fails", "env.AGENTFORGE_CLI_FIXTURE_MODE=fail\n");

    let launched = forge(
        &root,
        &[
            "task",
            "launch",
            root_text,
            "P1-M004-T0001",
            env!("CARGO_BIN_EXE_agentforge-cli-fixture"),
            "--base",
            "HEAD",
        ],
    );
    assert_eq!(launched.status.code(), Some(1), "{launched:?}");
    let stdout = String::from_utf8_lossy(&launched.stdout);
    assert!(
        stdout.contains("gate a-passes outcome=passed exit=0"),
        "{stdout}"
    );
    assert!(
        stdout.contains("gate b-fails outcome=failed exit=3"),
        "{stdout}"
    );
    assert!(stdout.contains("gates=1/2"), "{stdout}");
    assert!(
        String::from_utf8_lossy(&launched.stderr).contains("gates failed"),
        "{}",
        String::from_utf8_lossy(&launched.stderr)
    );

    let inspected = forge(&root, &["task", "inspect", root_text, "P1-M004-T0001"]);
    assert!(inspected.status.success(), "{inspected:?}");
    assert!(
        String::from_utf8_lossy(&inspected.stdout).contains("state=failed"),
        "{}",
        String::from_utf8_lossy(&inspected.stdout)
    );
    let hud = forge(&root, &["hud", root_text]);
    let hud_text = String::from_utf8_lossy(&hud.stdout);
    assert!(hud_text.contains("GateFinished"), "{hud_text}");

    let worktree = root.join(".forge/worktrees/P1-M004-T0001");
    fs::remove_file(worktree.join("agentforge-fixture-output.txt")).expect("fixture output");
    let retired = forge(&root, &["worktree", "retire", root_text, "P1-M004-T0001"]);
    assert!(retired.status.success(), "{retired:?}");
    fs::remove_dir_all(root).expect("cleanup");
}

fn write_gate(root: &Path, id: &str, extra: &str) {
    let directory = root.join(".forge/gates");
    fs::create_dir_all(&directory).expect("gate directory");
    fs::write(
        directory.join(format!("{id}.conf")),
        format!(
            "version=1\nexecutable={}\n{extra}",
            env!("CARGO_BIN_EXE_agentforge-cli-fixture")
        ),
    )
    .expect("gate profile");
}

fn forge(root: &Path, arguments: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_forge"))
        .args(arguments)
        .current_dir(root)
        .output()
        .expect("forge command")
}

fn temporary_repo() -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "agentforge-cli-gates-{}-{stamp}-{}",
        std::process::id(),
        TEMPORARY_ROOT_COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&root).expect("temporary root");
    git(&root, &["init", "-q"]);
    fs::write(root.join("README.md"), "fixture\n").expect("fixture");
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
