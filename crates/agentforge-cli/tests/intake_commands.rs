//! CLI integration coverage for project intake.

use agentforge_core::task::TaskId;
use agentforge_state::{FileTaskStore, TaskStore};
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

static SEQUENCE: AtomicU64 = AtomicU64::new(0);

fn temporary_root() -> PathBuf {
    let sequence = SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!(
        "agentforge-cli-intake-{}-{sequence}",
        std::process::id()
    ));
    fs::create_dir_all(&root).expect("temporary root");
    root
}

fn forge(root: &std::path::Path, arguments: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_forge"))
        .args(arguments)
        .current_dir(root)
        .output()
        .expect("forge command")
}

#[test]
fn intake_commands_create_and_validate_a_task_snapshot() {
    let root = temporary_root();
    let initialized = forge(&root, &["init", root.to_str().expect("root")]);
    assert!(initialized.status.success(), "{initialized:?}");

    let validated = forge(
        &root,
        &["blueprint", "validate", root.to_str().expect("root")],
    );
    assert!(validated.status.success(), "{validated:?}");

    let created = forge(
        &root,
        &[
            "task",
            "create",
            root.to_str().expect("root"),
            "P1-M003-T0001",
            "P1-M003",
            "implementer",
            "Create intake",
            "--allowed",
            "src",
            "--capability",
            "write_owned_paths",
            "--gate",
            "full",
        ],
    );
    assert!(created.status.success(), "{created:?}");

    let graph = FileTaskStore::for_project_root(&root)
        .load()
        .expect("load snapshot")
        .expect("snapshot exists");
    let task_id = TaskId::parse("P1-M003-T0001").expect("task ID");
    assert!(graph.get(&task_id).is_some());
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn initialization_refuses_to_overwrite_existing_files() {
    let root = temporary_root();
    let first = forge(&root, &["init", root.to_str().expect("root")]);
    assert!(first.status.success(), "{first:?}");
    let blueprint = root.join(".forge/blueprint.conf");
    let original = fs::read(&blueprint).expect("blueprint");

    let second = forge(&root, &["init", root.to_str().expect("root")]);
    assert!(!second.status.success(), "{second:?}");
    assert_eq!(fs::read(blueprint).expect("blueprint"), original);
    fs::remove_dir_all(root).expect("cleanup");
}
