//! Audit-log behavior through the real `forge` binary (P0-M013, findings 9 and 10).

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static TEMPORARY_ROOT_COUNTER: AtomicU64 = AtomicU64::new(0);

fn forge(root: &Path, arguments: &[&str]) -> Output {
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
        "agentforge-cli-audit-{}-{stamp}-{}",
        std::process::id(),
        TEMPORARY_ROOT_COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&root).expect("temporary root");
    let status = Command::new("git")
        .args(["init", "-q"])
        .current_dir(&root)
        .status()
        .expect("git init");
    assert!(status.success());
    root
}

fn create(root: &Path, task: &str, path: &str) {
    let root_text = root.to_str().expect("root");
    let created = forge(
        root,
        &[
            "task",
            "create",
            root_text,
            task,
            "P0-M013",
            "implementer",
            "audit fixture",
            "--allowed",
            path,
            "--approval",
            "activate_implementation_plan",
        ],
    );
    assert!(created.status.success(), "{created:?}");
}

#[test]
fn a_fresh_project_can_record_its_first_approval() {
    let root = temporary_repo();
    let root_text = root.to_str().expect("root");

    // No task snapshot yet: approval fails and no audit log is created.
    let early = forge(
        &root,
        &[
            "task",
            "approve",
            root_text,
            "P0-M013-T0001",
            "activate_implementation_plan",
            "--actor",
            "op",
        ],
    );
    assert!(!early.status.success());
    assert!(!root.join(".forge/audit.log").exists());

    assert!(forge(&root, &["init", root_text]).status.success());
    create(&root, "P0-M013-T0001", "a.txt");
    assert!(
        !root.join(".forge/audit.log").exists(),
        "task create writes no audit"
    );
    let approved = forge(
        &root,
        &[
            "task",
            "approve",
            root_text,
            "P0-M013-T0001",
            "activate_implementation_plan",
            "--actor",
            "op",
        ],
    );
    assert!(approved.status.success(), "{approved:?}");
    assert!(root.join(".forge/audit.log").is_file());
    let hud = forge(&root, &["hud", root_text]);
    assert!(
        String::from_utf8_lossy(&hud.stdout).contains("#1 ApprovalRecorded task=P0-M013-T0001"),
        "{hud:?}"
    );
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn concurrent_forge_processes_keep_the_audit_chain_valid() {
    let root = temporary_repo();
    let root_text = root.to_str().expect("root").to_owned();
    assert!(forge(&root, &["init", &root_text]).status.success());
    for index in 1..=24 {
        create(
            &root,
            &format!("P0-M013-T{index:04}"),
            &format!("f{index}.txt"),
        );
    }
    fs::create_dir_all(root.join(".forge/workers")).expect("workers");
    fs::write(
        root.join(".forge/workers/w-a.conf"),
        "platform=linux-x86_64\ncapability=rust\nmax_leases=64\n",
    )
    .expect("worker");

    // Three processes at once: two approving, one granting and releasing leases.
    let approver = |range: std::ops::RangeInclusive<u32>| {
        let root = root.clone();
        let root_text = root_text.clone();
        std::thread::spawn(move || {
            for index in range {
                let task = format!("P0-M013-T{index:04}");
                let output = forge(
                    &root,
                    &[
                        "task",
                        "approve",
                        &root_text,
                        &task,
                        "activate_implementation_plan",
                        "--actor",
                        "op",
                    ],
                );
                assert!(output.status.success(), "{output:?}");
            }
        })
    };
    let leaser = {
        let root = root.clone();
        let root_text = root_text.clone();
        std::thread::spawn(move || {
            for index in 17..=24 {
                let task = format!("P0-M013-T{index:04}");
                let granted = forge(
                    &root,
                    &["lease", "grant", &root_text, &task, "--actor", "op"],
                );
                assert!(granted.status.success(), "{granted:?}");
                let lease = format!("{task}.L1");
                let released = forge(
                    &root,
                    &["lease", "release", &root_text, &lease, "--actor", "op"],
                );
                assert!(released.status.success(), "{released:?}");
            }
        })
    };
    for handle in [approver(1..=8), approver(9..=16), leaser] {
        handle.join().expect("writer");
    }

    // 16 approvals + 8 grants + 8 releases, in one chain that verifies on open.
    let hud = forge(&root, &["hud", &root_text]);
    assert!(hud.status.success(), "{hud:?}");
    let text = String::from_utf8_lossy(&hud.stdout);
    assert!(
        text.contains("audit_records: 32\naudit_latest_sequence: 32\n"),
        "{text}"
    );
    assert!(!root.join(".forge/audit.log.lock").exists());
    fs::remove_dir_all(root).expect("cleanup");
}
