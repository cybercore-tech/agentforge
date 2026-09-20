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

fn forge_with_input(
    root: &std::path::Path,
    arguments: &[&str],
    input: &str,
) -> std::process::Output {
    use std::io::Write;

    let mut child = Command::new(env!("CARGO_BIN_EXE_forge"))
        .args(arguments)
        .current_dir(root)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("forge command");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(input.as_bytes())
        .expect("write input");
    child.wait_with_output().expect("forge output")
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

#[test]
fn guided_intake_creates_valid_documents_without_a_task() {
    let root = temporary_root();
    let input = "Omniscient\nBuild a useful operator surface.\nP2-M010\nsrc,docs\n.env\nread_repository,write_owned_paths\n\nfull\n- Prefer bounded workflows.\n.\ny\n";
    let result = forge_with_input(&root, &["intake", root.to_str().expect("root")], input);
    assert!(result.status.success(), "{result:?}");
    let bundle = agentforge_intake::load(&root).expect("guided bundle");
    assert_eq!(bundle.blueprint.name, "Omniscient");
    assert!(bundle.guidelines.body.contains("bounded workflows"));
    assert!(root.join(".forge/blueprint.conf").is_file());
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn guided_intake_can_create_an_explicit_task() {
    let root = temporary_root();
    let input = "Project\nShip the project.\nP2-M010\nsrc\n.env\nread_repository,write_owned_paths\n\nfull\n- Keep it reviewable.\n.\nP2-M010-T0001\nP2-M010\nimplementer\nAdd the guided command.\n\n\n\n\n\n\n\n\n\ny\n";
    let result = forge_with_input(
        &root,
        &["intake", root.to_str().expect("root"), "--task"],
        input,
    );
    assert!(result.status.success(), "{result:?}");
    let graph = FileTaskStore::for_project_root(&root)
        .load()
        .expect("load snapshot")
        .expect("snapshot exists");
    let task_id = TaskId::parse("P2-M010-T0001").expect("task ID");
    assert!(graph.get(&task_id).is_some());
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn guided_intake_cancellation_does_not_mutate_existing_documents() {
    let root = temporary_root();
    let initialized = forge(&root, &["init", root.to_str().expect("root")]);
    assert!(initialized.status.success(), "{initialized:?}");
    let blueprint_path = root.join(".forge/blueprint.conf");
    let guidelines_path = root.join(".forge/guidelines.md");
    let before_blueprint = fs::read(&blueprint_path).expect("blueprint");
    let before_guidelines = fs::read(&guidelines_path).expect("guidelines");
    let result = forge_with_input(&root, &["intake", root.to_str().expect("root")], "\n");
    assert!(result.status.success(), "{result:?}");
    assert_eq!(
        fs::read(blueprint_path).expect("blueprint"),
        before_blueprint
    );
    assert_eq!(
        fs::read(guidelines_path).expect("guidelines"),
        before_guidelines
    );
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn guided_intake_rejects_unknown_capability_before_writing() {
    let root = temporary_root();
    let input = "Project\nMission\nP2-M010\nsrc\n.env\nnot_a_capability\n";
    let result = forge_with_input(&root, &["intake", root.to_str().expect("root")], input);
    assert!(!result.status.success(), "{result:?}");
    assert!(!root.join(".forge/blueprint.conf").exists());
    assert!(!root.join(".forge/guidelines.md").exists());
    fs::remove_dir_all(root).expect("cleanup");
}
