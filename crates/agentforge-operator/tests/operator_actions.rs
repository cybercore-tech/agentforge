//! Integration coverage for durable operator actions.

use agentforge_audit::FileAuditStore;
use agentforge_core::agent::{AgentRole, AgentTask, ApprovalBoundary};
use agentforge_core::task::{TaskGraph, TaskId, TaskState};
use agentforge_operator::{approve_task, approved_boundaries, inspect_tasks, transition_task};
use agentforge_state::{FileTaskStore, TaskStore};
use std::fs;
use std::path::PathBuf;
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
