#![allow(missing_docs)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static TEMPORARY_ROOT_COUNTER: AtomicU64 = AtomicU64::new(0);
const SHA: &str = "0123456789abcdef0123456789abcdef01234567";

#[test]
fn successful_run_is_recorded_without_classification() {
    let root = initialized_project("success");
    let observed = observe(&root, &[]);
    assert_eq!(observed.status.code(), Some(0), "{observed:?}");
    let stdout = String::from_utf8_lossy(&observed.stdout);
    assert!(
        stdout.contains(&format!(
            "ci run=102 sha={SHA} status=completed conclusion=success"
        )),
        "{stdout}"
    );
    assert!(
        stdout.contains("recorded CiObserved and 0 FailureClassified"),
        "{stdout}"
    );
    let hud = hud(&root);
    assert!(hud.contains("CiObserved"), "{hud}");
    assert!(!hud.contains("FailureClassified"), "{hud}");
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn failed_jobs_are_classified_and_recorded() {
    let root = initialized_project("failure");
    let observed = observe(&root, &[]);
    assert_eq!(observed.status.code(), Some(1), "{observed:?}");
    let stdout = String::from_utf8_lossy(&observed.stdout);
    assert!(
        stdout.contains(
            "job \"Stable code gate\" status=completed conclusion=failure category=semantic_test"
        ),
        "{stdout}"
    );
    assert!(
        stdout
            .contains("job \"MSRV\" status=completed conclusion=failure category=compilation_type"),
        "{stdout}"
    );
    assert!(
        stdout.contains("job \"Repository policy\" status=completed conclusion=success\n"),
        "{stdout}"
    );
    assert!(stdout.contains("2 FailureClassified"), "{stdout}");
    assert_eq!(hud(&root).matches("FailureClassified").count(), 2);
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn pending_run_exits_three() {
    let root = initialized_project("pending");
    let observed = observe(&root, &[]);
    assert_eq!(observed.status.code(), Some(3), "{observed:?}");
    assert!(
        String::from_utf8_lossy(&observed.stdout).contains("status=in_progress conclusion=none")
    );
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn missing_or_ambiguous_evidence_records_nothing() {
    for (scenario, expected) in [
        ("missing", "no CI run matched"),
        ("ambiguous", "multiple CI runs matched"),
    ] {
        let root = initialized_project(scenario);
        let observed = observe(&root, &[]);
        assert_eq!(observed.status.code(), Some(1), "{observed:?}");
        assert!(
            String::from_utf8_lossy(&observed.stderr).contains(expected),
            "{}",
            String::from_utf8_lossy(&observed.stderr)
        );
        assert!(!hud(&root).contains("CiObserved"), "nothing is recorded");
        fs::remove_dir_all(root).expect("cleanup");
    }
}

#[test]
fn unknown_task_and_missing_provider_fail_before_observation() {
    let root = initialized_project("success");
    let linked = observe(&root, &["--task", "P1-M005-T0001"]);
    assert_eq!(linked.status.code(), Some(0), "{linked:?}");
    assert!(hud(&root).contains("CiObserved task=P1-M005-T0001"));
    let inspected = forge(
        &root,
        &[
            "task",
            "inspect",
            root.to_str().expect("root"),
            "P1-M005-T0001",
        ],
    );
    assert!(
        String::from_utf8_lossy(&inspected.stdout).contains("state=pending"),
        "observation never changes task state"
    );

    let unknown = observe(&root, &["--task", "P9-M999-T0001"]);
    assert_eq!(unknown.status.code(), Some(1), "{unknown:?}");
    assert!(String::from_utf8_lossy(&unknown.stderr).contains("task not found"));

    fs::remove_file(root.join(".forge/ci/provider.conf")).expect("remove provider");
    let missing = observe(&root, &[]);
    assert_eq!(missing.status.code(), Some(1), "{missing:?}");
    assert!(
        String::from_utf8_lossy(&missing.stderr).contains("no CI provider profile"),
        "{}",
        String::from_utf8_lossy(&missing.stderr)
    );
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn malformed_arguments_are_usage_errors() {
    let root = initialized_project("success");
    let root_text = root.to_str().expect("root");
    for arguments in [
        vec!["ci"],
        vec!["ci", "observe", root_text, "owner/repo", "CI"],
        vec![
            "ci",
            "observe",
            root_text,
            "owner/repo",
            "CI",
            SHA,
            "--task",
        ],
        vec!["ci", "watch", root_text, "owner/repo", "CI", SHA],
    ] {
        let output = forge(&root, &arguments);
        assert_eq!(output.status.code(), Some(2), "{arguments:?}: {output:?}");
    }
    fs::remove_dir_all(root).expect("cleanup");
}

fn initialized_project(scenario: &str) -> PathBuf {
    let root = temporary_repo();
    let root_text = root.to_str().expect("root");
    let init = forge(&root, &["init", root_text]);
    assert!(init.status.success(), "{init:?}");
    // A task snapshot lets the HUD render and gives `--task` something to resolve against.
    let created = forge(
        &root,
        &[
            "task",
            "create",
            root_text,
            "P1-M005-T0001",
            "P1-M005",
            "implementer",
            "ci fixture",
            "--capability",
            "run_local_commands",
        ],
    );
    assert!(created.status.success(), "{created:?}");
    fs::create_dir_all(root.join(".forge/ci")).expect("provider directory");
    fs::write(
        root.join(".forge/ci/provider.conf"),
        format!(
            "version=1\nexecutable={}\nenv.AGENTFORGE_CLI_FIXTURE_MODE=ci-{scenario}\n",
            env!("CARGO_BIN_EXE_agentforge-cli-fixture")
        ),
    )
    .expect("provider profile");
    root
}

fn observe(root: &Path, extra: &[&str]) -> std::process::Output {
    let root_text = root.to_str().expect("root");
    let mut arguments = vec![
        "ci",
        "observe",
        root_text,
        "owner/repo",
        "AgentForge CI",
        SHA,
    ];
    arguments.extend_from_slice(extra);
    forge(root, &arguments)
}

fn hud(root: &Path) -> String {
    let output = forge(root, &["hud", root.to_str().expect("root")]);
    String::from_utf8_lossy(&output.stdout).into_owned()
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
        "agentforge-cli-ci-{}-{stamp}-{}",
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
