//! Integration coverage for durable operator actions.

use agentforge_audit::{AuditStore, FileAuditStore};
use agentforge_core::agent::{AgentRole, AgentTask, ApprovalBoundary, Capability};
use agentforge_core::task::{TaskGraph, TaskId, TaskState};
use agentforge_operator::{
    approve_task, approved_boundaries, inspect_task_diff, inspect_tasks, integrate_task,
    transition_task,
};
use agentforge_state::{FileTaskStore, TaskStore};
use agentforge_worktree::{WorktreeManager, WorktreeSpec};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static SEQUENCE: AtomicU64 = AtomicU64::new(0);

fn root() -> PathBuf {
    let sequence = SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!(
        "agentforge-operator-{}-{sequence}",
        std::process::id()
    ));
    fs::create_dir_all(root.join(".forge")).expect("root");
    root
}

fn git(root: &Path, args: &[&str]) {
    let output = std::process::Command::new("git")
        .current_dir(root)
        .env_remove("GIT_INDEX_FILE")
        .args(args)
        .output()
        .expect("git");
    assert!(output.status.success(), "git failed: {:?}", output);
}

fn fixture(root: &PathBuf) -> TaskId {
    let mut task = AgentTask::new(
        "P2-M003-T0001",
        "P2-M003",
        AgentRole::Implementer,
        "operator action",
    );
    task.required_approvals = vec![ApprovalBoundary::ActivateImplementationPlan];
    let graph = TaskGraph::from_tasks([task]).expect("graph");
    FileTaskStore::for_project_root(root)
        .save(&graph)
        .expect("snapshot");
    FileAuditStore::open(root.join(".forge/audit.log")).expect("audit");
    TaskId::parse("P2-M003-T0001").expect("task ID")
}

#[test]
fn approvals_and_lifecycle_actions_are_durable_and_idempotent() {
    let root = root();
    let task_id = fixture(&root);
    let inspected = inspect_tasks(&root, Some(&task_id)).expect("inspect");
    assert_eq!(inspected[0].state, "pending");
    assert!(inspected[0].ready);

    approve_task(
        &root,
        &task_id,
        ApprovalBoundary::ActivateImplementationPlan,
        "operator",
    )
    .expect("approve");
    approve_task(
        &root,
        &task_id,
        ApprovalBoundary::ActivateImplementationPlan,
        "operator",
    )
    .expect("duplicate approval is idempotent");
    assert_eq!(
        approved_boundaries(&root, &task_id).expect("approvals"),
        vec![ApprovalBoundary::ActivateImplementationPlan]
    );

    transition_task(&root, &task_id, TaskState::Running, "operator").expect("run state");
    transition_task(&root, &task_id, TaskState::Succeeded, "operator").expect("success state");
    let inspected = inspect_tasks(&root, Some(&task_id)).expect("inspect final");
    assert_eq!(inspected[0].state, "succeeded");
    assert_eq!(inspected[0].revision, 2);
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn approved_succeeded_task_integrates_once_and_records_evidence() {
    let root = root();
    git(&root, &["init", "-b", "main"]);
    git(&root, &["config", "user.name", "AgentForge Test"]);
    git(
        &root,
        &["config", "user.email", "agentforge@example.invalid"],
    );
    fs::write(root.join("README.md"), "fixture\n").expect("fixture");
    git(&root, &["add", "README.md"]);
    git(&root, &["commit", "-m", "initial"]);

    let task_id = TaskId::parse("P2-M014-T0001").expect("task ID");
    let mut task = AgentTask::new(
        task_id.as_str(),
        "P2-M014",
        AgentRole::Integrator,
        "integrate reviewed change",
    );
    task.capabilities = vec![Capability::MergeProtectedBranch];
    task.required_approvals = vec![ApprovalBoundary::MergeProtectedBranch];
    let graph = TaskGraph::from_tasks([task]).expect("graph");
    FileTaskStore::for_project_root(&root)
        .save(&graph)
        .expect("snapshot");
    FileAuditStore::open(root.join(".forge/audit.log")).expect("audit");

    let manager = WorktreeManager::new(&root).expect("manager");
    let worktree = manager
        .create(&WorktreeSpec::new(task_id.clone(), "main"))
        .expect("worktree");
    fs::write(worktree.path().join("reviewed.txt"), "approved\n").expect("change");
    git(worktree.path(), &["add", "reviewed.txt"]);
    git(worktree.path(), &["commit", "-m", "reviewed change"]);

    approve_task(
        &root,
        &task_id,
        ApprovalBoundary::MergeProtectedBranch,
        "operator",
    )
    .expect("approval");
    transition_task(&root, &task_id, TaskState::Running, "operator").expect("running");
    transition_task(&root, &task_id, TaskState::Succeeded, "operator").expect("succeeded");

    let diff = inspect_task_diff(&root, &task_id).expect("diff");
    assert!(
        diff.changed_files()
            .iter()
            .any(|path| path == "reviewed.txt")
    );
    let report = integrate_task(&root, &task_id, "main", "operator").expect("integrate");
    assert!(!report.already_integrated());
    let audit_before = FileAuditStore::open(root.join(".forge/audit.log"))
        .expect("audit")
        .records()
        .len();
    let repeated = integrate_task(&root, &task_id, "main", "operator").expect("repeat");
    assert!(repeated.already_integrated());
    let audit_after = FileAuditStore::open(root.join(".forge/audit.log"))
        .expect("audit")
        .records()
        .len();
    assert_eq!(audit_before, audit_after);
    manager.retire(&task_id).expect("retire");
    fs::remove_dir_all(root).expect("cleanup");
}
