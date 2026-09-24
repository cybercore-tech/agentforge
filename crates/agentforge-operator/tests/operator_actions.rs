//! Integration coverage for durable operator actions.

use agentforge_audit::{AuditEvent, AuditEventKind, AuditStore, FileAuditStore};
use agentforge_core::agent::{AgentRole, AgentTask, ApprovalBoundary, Capability};
use agentforge_core::task::{TaskGraph, TaskId, TaskState};
use agentforge_operator::{
    approval_records, approve_task, approved_boundaries, inspect_task_diff, inspect_tasks,
    integrate_task, transition_task,
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

    // The merge approval is recorded after review, not before the agent runs (P1-M008).
    let audit_len = || {
        FileAuditStore::open(root.join(".forge/audit.log"))
            .expect("audit")
            .records()
            .len()
    };
    let before = audit_len();
    let early = approve_task(
        &root,
        &task_id,
        ApprovalBoundary::MergeProtectedBranch,
        "operator",
    )
    .expect_err("approval before acceptance");
    assert!(
        early.to_string().contains("approved after review"),
        "{early}"
    );
    assert_eq!(audit_len(), before);

    transition_task(&root, &task_id, TaskState::Running, "operator").expect("running");
    transition_task(&root, &task_id, TaskState::Succeeded, "operator").expect("succeeded");

    let diff = inspect_task_diff(&root, &task_id).expect("diff");
    assert!(
        diff.changed_files()
            .iter()
            .any(|path| path == "reviewed.txt")
    );
    let reviewed = diff.source_head().to_owned();
    let bound = approve_task(
        &root,
        &task_id,
        ApprovalBoundary::MergeProtectedBranch,
        "operator",
    )
    .expect("approval");
    assert_eq!(bound.as_deref(), Some(reviewed.as_str()));
    let after_approval = audit_len();
    approve_task(
        &root,
        &task_id,
        ApprovalBoundary::MergeProtectedBranch,
        "operator",
    )
    .expect("repeat approval");
    assert_eq!(audit_len(), after_approval, "same-head approval is a no-op");
    let inspected = inspect_tasks(&root, Some(&task_id)).expect("inspect");
    assert_eq!(
        inspected[0].recorded_approvals,
        vec![format!("merge_protected_branch@{reviewed}")]
    );

    // A commit added after the approval is not what was reviewed.
    fs::write(worktree.path().join("late.txt"), "unreviewed\n").expect("late change");
    git(worktree.path(), &["add", "late.txt"]);
    git(worktree.path(), &["commit", "-m", "late change"]);
    let late = inspect_task_diff(&root, &task_id)
        .expect("diff")
        .source_head()
        .to_owned();
    let main_before = inspect_task_diff(&root, &task_id)
        .expect("diff")
        .target_head()
        .to_owned();
    let refused = integrate_task(&root, &task_id, "main", "operator").expect_err("stale approval");
    let message = refused.to_string();
    assert!(
        message.contains(&reviewed) && message.contains(&late),
        "{message}"
    );
    assert_eq!(
        inspect_task_diff(&root, &task_id)
            .expect("diff")
            .target_head(),
        main_before
    );

    // Approving again binds the new head.
    let rebound = approve_task(
        &root,
        &task_id,
        ApprovalBoundary::MergeProtectedBranch,
        "operator",
    )
    .expect("re-approval");
    assert_eq!(rebound.as_deref(), Some(late.as_str()));
    assert_eq!(
        approval_records(&root, &task_id)
            .expect("records")
            .iter()
            .filter_map(|record| record.source_head.clone())
            .collect::<Vec<_>>(),
        vec![reviewed.clone(), late.clone()]
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

#[test]
fn legacy_unbound_merge_approval_cannot_integrate() {
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

    let task_id = TaskId::parse("P1-M008-T0001").expect("task ID");
    let mut task = AgentTask::new(
        task_id.as_str(),
        "P1-M008",
        AgentRole::Integrator,
        "legacy approval",
    );
    task.capabilities = vec![Capability::MergeProtectedBranch];
    task.required_approvals = vec![ApprovalBoundary::MergeProtectedBranch];
    FileTaskStore::for_project_root(&root)
        .save(&TaskGraph::from_tasks([task]).expect("graph"))
        .expect("snapshot");
    let mut audit = FileAuditStore::open(root.join(".forge/audit.log")).expect("audit");
    // An approval recorded before P1-M008 carries no source head.
    audit
        .append(
            AuditEvent::new(
                1,
                "operator-approval-1",
                AuditEventKind::ApprovalRecorded,
                "operator",
                1,
            )
            .with_task_id(task_id.as_str())
            .with_field("boundary", "merge_protected_branch"),
        )
        .expect("legacy approval");

    let manager = WorktreeManager::new(&root).expect("manager");
    let worktree = manager
        .create(&WorktreeSpec::new(task_id.clone(), "main"))
        .expect("worktree");
    fs::write(worktree.path().join("change.txt"), "change\n").expect("change");
    git(worktree.path(), &["add", "change.txt"]);
    git(worktree.path(), &["commit", "-m", "change"]);
    transition_task(&root, &task_id, TaskState::Running, "operator").expect("running");
    transition_task(&root, &task_id, TaskState::Succeeded, "operator").expect("succeeded");

    let error = integrate_task(&root, &task_id, "main", "operator").expect_err("legacy");
    assert!(
        error.to_string().contains("not bound to a reviewed commit"),
        "{error}"
    );
    approve_task(
        &root,
        &task_id,
        ApprovalBoundary::MergeProtectedBranch,
        "operator",
    )
    .expect("bound approval");
    integrate_task(&root, &task_id, "main", "operator").expect("integrate");
    manager.retire(&task_id).expect("retire");
    fs::remove_dir_all(root).expect("cleanup");
}
