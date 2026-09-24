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

fn forge_with_input_file(
    root: &std::path::Path,
    arguments: &[&str],
    input: &str,
) -> std::process::Output {
    let input_path = root.join("guided-session.txt");
    fs::write(&input_path, input).expect("input file");
    let mut command_arguments = arguments.to_vec();
    command_arguments.extend(["--input-file", input_path.to_str().expect("input path")]);
    forge(root, &command_arguments)
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

fn create_task(root: &std::path::Path, task_id: &str, gates: &[&str]) -> Vec<String> {
    let root_text = root.to_str().expect("root");
    let mut arguments = vec![
        "task",
        "create",
        root_text,
        task_id,
        "P1-M007",
        "implementer",
        "Select gates",
        "--allowed",
        "src",
        "--capability",
        "write_owned_paths",
    ];
    for gate in gates {
        arguments.extend(["--gate", gate]);
    }
    let created = forge(root, &arguments);
    assert!(created.status.success(), "{created:?}");
    required_gates(root, task_id)
}

fn required_gates(root: &std::path::Path, task_id: &str) -> Vec<String> {
    let graph = FileTaskStore::for_project_root(root)
        .load()
        .expect("load snapshot")
        .expect("snapshot exists");
    graph
        .get(&TaskId::parse(task_id).expect("task ID"))
        .expect("task exists")
        .task()
        .required_gates
        .clone()
}

#[test]
fn blueprint_default_gate_with_a_profile_is_copied_into_the_task() {
    let root = temporary_root();
    let initialized = forge(&root, &["init", root.to_str().expect("root")]);
    assert!(initialized.status.success(), "{initialized:?}");
    fs::create_dir_all(root.join(".forge/gates")).expect("gate directory");
    fs::write(
        root.join(".forge/gates/full.conf"),
        "version=1\nexecutable=/usr/bin/true\n",
    )
    .expect("gate profile");
    assert_eq!(create_task(&root, "P1-M007-T0001", &[]), ["full"]);
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn blueprint_default_gate_without_a_profile_is_dropped() {
    let root = temporary_root();
    let initialized = forge(&root, &["init", root.to_str().expect("root")]);
    assert!(initialized.status.success(), "{initialized:?}");
    assert!(create_task(&root, "P1-M007-T0001", &[]).is_empty());

    let input = "Project\nShip the project.\nP2-M010\nsrc\n.env\nread_repository,write_owned_paths\n\nfull\n- Keep it reviewable.\n.\nP2-M010-T0001\nP2-M010\nimplementer\nAdd the guided command.\n\n\n\n\n\n\n\n\n\ny\n";
    let guided_root = temporary_root();
    let result = forge_with_input(
        &guided_root,
        &["intake", guided_root.to_str().expect("root"), "--task"],
        input,
    );
    assert!(result.status.success(), "{result:?}");
    assert!(required_gates(&guided_root, "P2-M010-T0001").is_empty());
    fs::remove_dir_all(root).expect("cleanup");
    fs::remove_dir_all(guided_root).expect("cleanup");
}

#[test]
fn explicit_unconfigured_gate_is_kept_as_given() {
    let root = temporary_root();
    let initialized = forge(&root, &["init", root.to_str().expect("root")]);
    assert!(initialized.status.success(), "{initialized:?}");
    assert_eq!(
        create_task(&root, "P1-M007-T0001", &["missing"]),
        ["missing"]
    );
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

#[test]
fn guided_intake_replays_a_named_input_file() {
    let root = temporary_root();
    let input = "Replay Project\nReplay the workflow.\nP2-M011\nsrc\n.env\nread_repository\n\nfull\n- Replayable guidance.\n.\ny\n";
    let result = forge_with_input_file(&root, &["intake", root.to_str().expect("root")], input);
    assert!(result.status.success(), "{result:?}");
    let bundle = agentforge_intake::load(&root).expect("guided bundle");
    assert_eq!(bundle.blueprint.name, "Replay Project");
    assert!(bundle.guidelines.body.contains("Replayable guidance"));
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn guided_input_file_task_handoff_is_visible_to_inspect_and_hud() {
    let root = temporary_root();
    let input = "Replay Project\nReplay the task handoff.\nP2-M011\nsrc\n.env\nread_repository\n\nfull\n- Keep the handoff inspectable.\n.\nP2-M011-T0001\nP2-M011\nimplementer\nExercise the handoff.\n\n\n\n\n\n\n\n\n\ny\n";
    let result = forge_with_input_file(
        &root,
        &["intake", root.to_str().expect("root"), "--task"],
        input,
    );
    assert!(result.status.success(), "{result:?}");
    let inspected = forge(
        &root,
        &[
            "task",
            "inspect",
            root.to_str().expect("root"),
            "P2-M011-T0001",
        ],
    );
    assert!(inspected.status.success(), "{inspected:?}");
    assert!(String::from_utf8_lossy(&inspected.stdout).contains("P2-M011-T0001"));
    agentforge_audit::FileAuditStore::open(root.join(".forge/audit.log")).expect("audit log");
    let git = Command::new("git")
        .args(["init", "--quiet"])
        .current_dir(&root)
        .status()
        .expect("git init");
    assert!(git.success(), "{git:?}");
    let hud = forge(&root, &["hud", root.to_str().expect("root")]);
    assert!(hud.status.success(), "{hud:?}");
    assert!(String::from_utf8_lossy(&hud.stdout).contains("pending: 1"));
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn guided_intake_rejects_invalid_or_oversized_input_files_before_mutation() {
    let root = temporary_root();
    let missing = forge(
        &root,
        &[
            "intake",
            root.to_str().expect("root"),
            "--input-file",
            root.join("missing.txt").to_str().expect("missing path"),
        ],
    );
    assert!(!missing.status.success(), "{missing:?}");
    assert!(!root.join(".forge").exists());

    let oversized_path = root.join("oversized.txt");
    fs::write(&oversized_path, vec![b'x'; 512 * 1024 + 1]).expect("oversized input");
    let oversized = forge(
        &root,
        &[
            "intake",
            root.to_str().expect("root"),
            "--input-file",
            oversized_path.to_str().expect("oversized path"),
        ],
    );
    assert!(!oversized.status.success(), "{oversized:?}");
    assert!(!root.join(".forge").exists());
    fs::remove_dir_all(root).expect("cleanup");
}
