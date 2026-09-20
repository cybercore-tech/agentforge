//! CLI integration coverage for controlled operator actions.

use agentforge_audit::FileAuditStore;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

static SEQUENCE: AtomicU64 = AtomicU64::new(0);

fn root() -> PathBuf {
    let sequence = SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!(
        "agentforge-cli-operator-{}-{sequence}",
        std::process::id()
    ));
    fs::create_dir_all(&root).expect("root");
    root
}

fn forge(root: &Path, arguments: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_forge"))
        .args(arguments)
        .current_dir(root)
        .output()
        .expect("forge command")
}

#[test]
fn operator_commands_inspect_and_record_required_approval() {
    let root = root();
    assert!(
        forge(&root, &["init", root.to_str().expect("root")])
            .status
            .success()
    );
    assert!(
        forge(
            &root,
            &[
                "task",
                "create",
                root.to_str().expect("root"),
                "P2-M003-T0001",
                "P2-M003",
                "implementer",
                "controlled action",
                "--approval",
                "activate_implementation_plan",
            ],
        )
        .status
        .success()
    );
    fs::create_dir_all(root.join(".forge")).expect("forge dir");
    FileAuditStore::open(root.join(".forge/audit.log")).expect("audit");

    let inspected = forge(&root, &["task", "inspect", root.to_str().expect("root")]);
    assert!(inspected.status.success(), "{inspected:?}");
    assert!(String::from_utf8_lossy(&inspected.stdout).contains("state=pending"));
    let approved = forge(
        &root,
        &[
            "task",
            "approve",
            root.to_str().expect("root"),
            "P2-M003-T0001",
            "activate_implementation_plan",
            "--actor",
            "operator",
        ],
    );
    assert!(approved.status.success(), "{approved:?}");
    assert!(String::from_utf8_lossy(&approved.stdout).contains("approved"));
    fs::remove_dir_all(root).expect("cleanup");
}
