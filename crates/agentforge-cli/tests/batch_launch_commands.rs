#![allow(missing_docs)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static TEMPORARY_ROOT_COUNTER: AtomicU64 = AtomicU64::new(0);

#[test]
fn launch_batch_runs_disjoint_tasks_concurrently_and_defers_overlap() {
    let root = temporary_repo();
    let root_text = root.to_str().expect("root");
    assert!(forge(&root, &["init", root_text]).status.success());
    for (id, path) in [
        ("P1-M006-T0001", "src"),
        ("P1-M006-T0002", "docs"),
        ("P1-M006-T0003", "src/lib.rs"),
    ] {
        create_task(&root, id, path);
    }
    // Each fixture agent waits until two agents have started, so the batch only succeeds when
    // both agents really run at the same time.
    let rendezvous = root.join("rendezvous");
    fs::create_dir(&rendezvous).expect("rendezvous directory");
    write_profile(
        &root,
        "rendezvous",
        &format!(
            "env.AGENTFORGE_CLI_FIXTURE_MODE=rendezvous\nenv.AGENTFORGE_RENDEZVOUS_DIR={}\nenv.AGENTFORGE_RENDEZVOUS_COUNT=2\n",
            rendezvous.display()
        ),
    );

    let batch = forge(
        &root,
        &["task", "launch-batch", root_text, "--profile", "rendezvous"],
    );
    let stdout = String::from_utf8_lossy(&batch.stdout);
    assert_eq!(batch.status.code(), Some(0), "{batch:?}");
    assert!(
        stdout.contains("launched P1-M006-T0001 termination=Exited exit=0 gates=0/0"),
        "{stdout}"
    );
    assert!(stdout.contains("launched P1-M006-T0002"), "{stdout}");
    assert!(
        stdout
            .contains("deferred P1-M006-T0003 reason=overlap owner=P1-M006-T0001 path=src/lib.rs"),
        "{stdout}"
    );
    assert!(
        stdout.contains("launched=2 succeeded=2 deferred=1"),
        "{stdout}"
    );
    assert_eq!(state(&root, "P1-M006-T0001"), "running");
    assert_eq!(state(&root, "P1-M006-T0002"), "running");
    assert_eq!(state(&root, "P1-M006-T0003"), "pending");

    // While T0001 is running, its overlapping sibling stays deferred.
    let again = forge(
        &root,
        &["task", "launch-batch", root_text, "--profile", "rendezvous"],
    );
    let again_stdout = String::from_utf8_lossy(&again.stdout);
    assert!(
        again_stdout.contains("deferred P1-M006-T0003 reason=overlap owner=P1-M006-T0001"),
        "{again_stdout}"
    );
    assert!(again_stdout.contains("launched=0"), "{again_stdout}");
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn launch_batch_reports_no_ready_tasks_and_rejects_bad_arguments() {
    let root = temporary_repo();
    let root_text = root.to_str().expect("root");
    assert!(forge(&root, &["init", root_text]).status.success());
    create_task(&root, "P1-M006-T0001", "src");
    let executable = env!("CARGO_BIN_EXE_agentforge-cli-fixture");
    for arguments in [
        vec!["task", "launch-batch"],
        vec!["task", "launch-batch", root_text],
        vec!["task", "launch-batch", root_text, executable, "--max", "0"],
        vec!["task", "launch-batch", root_text, executable, "--max", "17"],
        vec![
            "task",
            "launch-batch",
            root_text,
            executable,
            "--profile",
            "x",
        ],
        vec!["task", "launch-batch", root_text, executable, "--pty"],
    ] {
        let output = forge(&root, &arguments);
        assert_eq!(output.status.code(), Some(2), "{arguments:?}: {output:?}");
    }

    let first = forge(
        &root,
        &["task", "launch-batch", root_text, executable, "--max", "1"],
    );
    assert_eq!(first.status.code(), Some(0), "{first:?}");
    let empty = forge(&root, &["task", "launch-batch", root_text, executable]);
    assert_eq!(empty.status.code(), Some(0), "{empty:?}");
    assert_eq!(String::from_utf8_lossy(&empty.stdout), "no ready tasks\n");
    fs::remove_dir_all(root).expect("cleanup");
}

fn create_task(root: &Path, id: &str, path: &str) {
    let created = forge(
        root,
        &[
            "task",
            "create",
            root.to_str().expect("root"),
            id,
            "P1-M006",
            "implementer",
            "batch task",
            "--allowed",
            path,
            "--capability",
            "run_local_commands",
        ],
    );
    assert!(created.status.success(), "{created:?}");
}

fn write_profile(root: &Path, id: &str, extra: &str) {
    let directory = root.join(".forge/agents");
    fs::create_dir_all(&directory).expect("agent directory");
    fs::write(
        directory.join(format!("{id}.conf")),
        format!(
            "version=1\nexecutable={}\n{extra}",
            env!("CARGO_BIN_EXE_agentforge-cli-fixture")
        ),
    )
    .expect("agent profile");
}

fn state(root: &Path, id: &str) -> String {
    let inspected = forge(root, &["task", "inspect", root.to_str().expect("root"), id]);
    let stdout = String::from_utf8_lossy(&inspected.stdout);
    stdout
        .split_whitespace()
        .find_map(|field| field.strip_prefix("state="))
        .unwrap_or_else(|| panic!("no state in {stdout}"))
        .to_owned()
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
        "agentforge-cli-batch-{}-{stamp}-{}",
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
