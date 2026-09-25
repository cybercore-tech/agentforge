//! CLI integration coverage for the read-only operator HUD.

use agentforge_audit::FileAuditStore;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

static SEQUENCE: AtomicU64 = AtomicU64::new(0);

fn temporary_root() -> PathBuf {
    let sequence = SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!(
        "agentforge-cli-hud-{}-{sequence}",
        std::process::id()
    ));
    fs::create_dir_all(&root).expect("temporary root");
    root
}

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
    assert!(output.status.success(), "git {:?}: {:?}", arguments, output);
}

#[test]
fn hud_renders_sources_without_mutating_the_project() {
    let root = temporary_root();
    let initialized = forge(&root, &["init", root.to_str().expect("root")]);
    assert!(initialized.status.success(), "{initialized:?}");
    let created = forge(
        &root,
        &[
            "task",
            "create",
            root.to_str().expect("root"),
            "P2-M001-T0001",
            "P2-M001",
            "implementer",
            "render HUD",
        ],
    );
    assert!(created.status.success(), "{created:?}");
    git(&root, &["init", "-q"]);
    git(
        &root,
        &["config", "user.email", "agentforge@example.invalid"],
    );
    git(&root, &["config", "user.name", "AgentForge Test"]);
    fs::write(root.join("README.md"), "fixture\n").expect("fixture");
    git(&root, &["add", "README.md", ".forge"]);
    git(&root, &["commit", "-qm", "fixture"]);
    let audit = FileAuditStore::open(root.join(".forge/audit.log")).expect("audit");
    drop(audit);
    let before = fs::read_dir(&root)
        .expect("root entries")
        .map(|entry| entry.expect("entry").file_name())
        .collect::<Vec<_>>();

    let output = forge(&root, &["hud", root.to_str().expect("root")]);
    assert!(output.status.success(), "{output:?}");
    let stdout = String::from_utf8(output.stdout).expect("UTF-8 HUD");
    assert!(stdout.contains("AgentForge HUD"));
    assert!(stdout.contains("project: My Project"));
    assert!(stdout.contains("tasks:"));
    assert!(stdout.contains("audit_records: 0"));
    assert!(stdout.contains("worktrees: 0"));
    assert!(stdout.contains("agent_runs:\n  - none\n"), "{stdout}");
    assert!(
        stdout.contains("workers:\n  - none\nleases: active=0 expired=0 released=0\n"),
        "{stdout}"
    );
    // Reading leases never creates the lease snapshot or its lock (P4-M010).
    assert!(!root.join(".forge/state/remote-leases.snapshot").exists());
    assert!(!root.join(".forge/state/remote-leases.lock").exists());
    let after = fs::read_dir(&root)
        .expect("root entries")
        .map(|entry| entry.expect("entry").file_name())
        .collect::<Vec<_>>();
    assert_eq!(before, after);
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn hud_shows_a_launched_agent_run_with_its_evidence() {
    let root = temporary_root();
    let root_text = root.to_str().expect("root");
    git(&root, &["init", "-q"]);
    git(
        &root,
        &["config", "user.email", "agentforge@example.invalid"],
    );
    git(&root, &["config", "user.name", "AgentForge Test"]);
    fs::write(root.join("README.md"), "fixture\n").expect("fixture");
    git(&root, &["add", "README.md"]);
    git(&root, &["commit", "-qm", "fixture"]);
    assert!(forge(&root, &["init", root_text]).status.success());
    let created = forge(
        &root,
        &[
            "task",
            "create",
            root_text,
            "P2-M033-T0001",
            "P2-M033",
            "implementer",
            "agent runs",
            "--allowed",
            "README.md",
            "--capability",
            "run_local_commands",
        ],
    );
    assert!(created.status.success(), "{created:?}");
    let launched = forge(
        &root,
        &[
            "task",
            "launch",
            root_text,
            "P2-M033-T0001",
            env!("CARGO_BIN_EXE_agentforge-cli-fixture"),
            "--base",
            "HEAD",
        ],
    );
    assert!(launched.status.success(), "{launched:?}");

    let output = forge(&root, &["hud", root_text]);
    assert!(output.status.success(), "{output:?}");
    let stdout = String::from_utf8(output.stdout).expect("UTF-8 HUD");
    let section = stdout
        .split_once("agent_runs:\n")
        .expect("agent_runs section")
        .1;
    let run = section.lines().next().expect("agent run line");
    assert!(
        run.contains(" task=P2-M033-T0001 agent-exit=0 "),
        "{stdout}"
    );
    assert!(run.contains(" termination=exited "), "{stdout}");
    assert!(
        run.contains(" stdout=.forge/evidence/P2-M033-T0001/"),
        "{stdout}"
    );
    assert!(
        run.contains(" stderr=.forge/evidence/P2-M033-T0001/"),
        "{stdout}"
    );
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn hud_fails_closed_when_a_required_source_is_missing() {
    let root = temporary_root();
    let output = forge(&root, &["hud", root.to_str().expect("root")]);
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stderr).contains("intake source unavailable"));
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn hud_watch_accepts_help_and_quit_without_mutation() {
    let root = temporary_root();
    let before = fs::read_dir(&root).expect("root entries").count();
    let mut child = Command::new(env!("CARGO_BIN_EXE_forge"))
        .args([
            "hud",
            root.to_str().expect("root"),
            "--watch",
            "--interval-ms",
            "50",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .current_dir(&root)
        .spawn()
        .expect("forge watch");
    child
        .stdin
        .take()
        .expect("watch stdin")
        .write_all(b"help\nq\n")
        .expect("watch commands");
    let output = child.wait_with_output().expect("watch output");
    assert!(output.status.success(), "{output:?}");
    let stdout = String::from_utf8(output.stdout).expect("UTF-8 watch output");
    assert!(stdout.contains("HUD diagnostic:"));
    assert!(stdout.contains("watch commands: r/refresh refresh, h/help help, q/quit exit"));
    assert_eq!(fs::read_dir(&root).expect("root entries").count(), before);
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn hud_watch_rejects_non_numeric_interval() {
    let root = temporary_root();
    let output = forge(
        &root,
        &[
            "hud",
            root.to_str().expect("root"),
            "--watch",
            "--interval-ms",
            "nope",
        ],
    );
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("numeric value"));
    fs::remove_dir_all(root).expect("cleanup");
}

/// Every file under `.forge`, with its contents, to prove the HUD changed nothing.
fn forge_tree(root: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    let mut files = Vec::new();
    let mut pending = vec![root.join(".forge")];
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(&directory).expect("directory") {
            let path = entry.expect("entry").path();
            if path.is_dir() {
                pending.push(path);
            } else {
                let contents = fs::read(&path).expect("file");
                files.push((path, contents));
            }
        }
    }
    files.sort();
    files
}

#[test]
fn hud_shows_registered_workers_and_active_leases() {
    let root = temporary_root();
    let root_text = root.to_str().expect("root");
    assert!(forge(&root, &["init", root_text]).status.success());
    let created = forge(
        &root,
        &[
            "task",
            "create",
            root_text,
            "P4-M010-T0001",
            "P4-M010",
            "implementer",
            "leased work",
        ],
    );
    assert!(created.status.success(), "{created:?}");
    fs::create_dir_all(root.join(".forge/workers")).expect("workers");
    for worker in ["remote-a", "remote-b"] {
        fs::write(
            root.join(format!(".forge/workers/{worker}.conf")),
            "platform=linux-x86_64\ncapability=rust\nmax_leases=2\n",
        )
        .expect("worker profile");
    }
    let granted = forge(
        &root,
        &[
            "lease",
            "grant",
            root_text,
            "P4-M010-T0001",
            "--worker",
            "remote-a",
            "--actor",
            "op",
        ],
    );
    assert!(granted.status.success(), "{granted:?}");
    git(&root, &["init", "-q"]);
    git(
        &root,
        &["config", "user.email", "agentforge@example.invalid"],
    );
    git(&root, &["config", "user.name", "AgentForge Test"]);
    fs::write(root.join(".gitignore"), ".forge/\n").expect("gitignore");
    git(&root, &["add", ".gitignore"]);
    git(&root, &["commit", "-qm", "fixture"]);
    let before = forge_tree(&root);

    let output = forge(&root, &["hud", root_text]);
    assert!(output.status.success(), "{output:?}");
    let stdout = String::from_utf8(output.stdout).expect("UTF-8 HUD");
    assert!(
        stdout.contains(
            "workers:\n  - remote-a platform=linux-x86_64 leases=1/2 last-seen=never\n  \
             - remote-b platform=linux-x86_64 leases=0/2 last-seen=never\n"
        ),
        "an operator grant is not a sighting of the worker:\n{stdout}"
    );
    assert!(
        stdout.contains(
            "leases: active=1 expired=0 released=0\n  - P4-M010-T0001.L1 task=P4-M010-T0001 \
             worker=remote-a gen=1 expires-in="
        ),
        "{stdout}"
    );
    assert_eq!(before, forge_tree(&root), "the HUD is read-only");
    fs::remove_dir_all(root).expect("cleanup");
}
