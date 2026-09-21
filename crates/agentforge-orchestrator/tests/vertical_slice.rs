//! End-to-end ordering checks for the bounded vertical slice.
use agentforge_adapter::{
    AdapterError, AdapterRequest, AgentAdapter, ProcessAdapter, ProcessAdapterConfig,
};
use agentforge_audit::FileAuditStore;
use agentforge_audit::{AuditEvent, AuditEventKind, AuditStore};
use agentforge_core::agent::{AgentRole, AgentTask, ApprovalBoundary, Capability};
use agentforge_core::task::{TaskGraph, TaskId, TaskState};
use agentforge_orchestrator::{
    SliceError, SliceEvidence, SliceStage, execute_process_persisted, launch_process_persisted, run,
};
use agentforge_state::{FileTaskStore, TaskStore};
use agentforge_worktree::{WorktreeManager, WorktreeSpec};
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

struct FailingAdapter;

impl AgentAdapter for FailingAdapter {
    fn id(&self) -> &str {
        "failing-fixture"
    }

    fn execute(
        &self,
        _request: AdapterRequest<'_>,
    ) -> Result<agentforge_adapter::ExecutionReport, AdapterError> {
        Err(AdapterError::CapturePoisoned)
    }
}

#[test]
fn missing_policy_authority_prevents_execution() {
    let task = AgentTask::new("t", "m", AgentRole::Reviewer, "review");
    let error = run(
        &task,
        &SliceEvidence {
            worktree_verified: true,
            agent_reported: true,
            gates_passed: true,
        },
    )
    .unwrap_err();
    assert!(matches!(error, SliceError::Policy(_)));
}

#[test]
fn successful_flow_has_canonical_stage_order() {
    let mut task = AgentTask::new("t", "m", AgentRole::Implementer, "work");
    task.capabilities = vec![Capability::RunLocalCommands];
    task.allowed_paths = vec!["src".into()];
    let report = run(
        &task,
        &SliceEvidence {
            worktree_verified: true,
            agent_reported: true,
            gates_passed: true,
        },
    )
    .unwrap();
    assert_eq!(
        report.stages,
        vec![
            SliceStage::Policy,
            SliceStage::Worktree,
            SliceStage::Agent,
            SliceStage::Gates,
            SliceStage::ReviewHandoff
        ]
    );
}

#[test]
fn real_process_adapter_updates_state_and_audit_in_isolated_repo() {
    let root = unique_temp_repo();
    git(&root, &["init", "-q"]);
    git(
        &root,
        &["config", "user.email", "agentforge@example.invalid"],
    );
    git(&root, &["config", "user.name", "AgentForge Test"]);
    std::fs::write(root.join("README.md"), "fixture\n").unwrap();
    git(&root, &["add", "README.md"]);
    git(&root, &["commit", "-qm", "fixture"]);
    let mut task = AgentTask::new(
        "P1-M002-T0001",
        "P1-M002",
        AgentRole::Implementer,
        "run fixture",
    );
    task.capabilities = vec![Capability::RunLocalCommands];
    task.allowed_paths = vec!["README.md".into()];
    let task_id = TaskId::parse(task.task_id.clone()).unwrap();
    let graph = TaskGraph::from_tasks([task.clone()]).unwrap();
    let task_store = FileTaskStore::from_path(root.join("tasks.snapshot"));
    task_store.save(&graph).unwrap();
    let manager = WorktreeManager::new(&root).unwrap();
    manager
        .create(&WorktreeSpec::new(task_id.clone(), "HEAD"))
        .unwrap();
    let adapter = ProcessAdapter::new(ProcessAdapterConfig::new("true", "/usr/bin/true")).unwrap();
    let audit_path = root.join("audit.log");
    let mut audit_store = FileAuditStore::open(&audit_path).unwrap();
    audit_store
        .append(
            AuditEvent::new(1, "prior-task", AuditEventKind::TaskCreated, "operator", 1)
                .with_task_id("P1-M002-T0000"),
        )
        .unwrap();
    audit_store
        .append(
            AuditEvent::new(
                2,
                "prior-transition",
                AuditEventKind::TaskTransition,
                "operator",
                2,
            )
            .with_task_id("P1-M002-T0000"),
        )
        .unwrap();
    let prior_tail = *audit_store.records().last().unwrap().digest();
    let execution = execute_process_persisted(
        &root,
        &task_store,
        &mut audit_store,
        &task_id,
        &adapter,
        &[],
    )
    .unwrap();
    let restored = task_store.load().unwrap().unwrap();
    assert_eq!(
        restored.records().next().unwrap().state(),
        TaskState::Running
    );
    assert_eq!(execution.audit.records().len(), 3);
    assert_eq!(execution.audit.records()[0].event().sequence(), 3);
    assert_eq!(*execution.audit.records()[0].previous_digest(), prior_tail);
    assert_eq!(audit_store.records().len(), 5);
    assert_eq!(audit_store.records()[2].event().sequence(), 3);
    assert_eq!(execution.report.task_id().as_str(), task.task_id);
    let _ = manager.retire(&task_id);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn persisted_preflight_failure_preserves_existing_audit() {
    let root = unique_temp_repo();
    git(&root, &["init", "-q"]);
    git(
        &root,
        &["config", "user.email", "agentforge@example.invalid"],
    );
    git(&root, &["config", "user.name", "AgentForge Test"]);
    std::fs::write(root.join("README.md"), "fixture\n").unwrap();
    git(&root, &["add", "README.md"]);
    git(&root, &["commit", "-qm", "fixture"]);
    let mut task = AgentTask::new(
        "P2-M009-T0001",
        "P2-M009",
        AgentRole::Implementer,
        "preflight failure fixture",
    );
    task.allowed_paths = vec!["README.md".into()];
    let task_id = TaskId::parse(task.task_id.clone()).unwrap();
    let graph = TaskGraph::from_tasks([task]).unwrap();
    let task_store = FileTaskStore::from_path(root.join("tasks.snapshot"));
    task_store.save(&graph).unwrap();
    let mut audit_store = FileAuditStore::open(root.join("audit.log")).unwrap();
    audit_store
        .append(AuditEvent::new(
            1,
            "prior-task",
            AuditEventKind::TaskCreated,
            "operator",
            1,
        ))
        .unwrap();

    let error = execute_process_persisted(
        &root,
        &task_store,
        &mut audit_store,
        &task_id,
        &FailingAdapter,
        &[],
    )
    .unwrap_err();

    assert!(matches!(error, SliceError::Preflight(_)));
    assert_eq!(audit_store.records().len(), 1);
    assert_eq!(
        task_store
            .load()
            .unwrap()
            .unwrap()
            .get(&task_id)
            .unwrap()
            .state(),
        TaskState::Pending
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn adapter_failure_persists_failed_state_and_audit_before_returning() {
    let root = unique_temp_repo();
    git(&root, &["init", "-q"]);
    git(
        &root,
        &["config", "user.email", "agentforge@example.invalid"],
    );
    git(&root, &["config", "user.name", "AgentForge Test"]);
    std::fs::write(root.join("README.md"), "fixture\n").unwrap();
    git(&root, &["add", "README.md"]);
    git(&root, &["commit", "-qm", "fixture"]);
    let mut task = AgentTask::new(
        "P2-M005-T0001",
        "P2-M005",
        AgentRole::Implementer,
        "fail fixture",
    );
    task.capabilities = vec![Capability::RunLocalCommands];
    task.allowed_paths = vec!["README.md".into()];
    let task_id = TaskId::parse(task.task_id.clone()).unwrap();
    let graph = TaskGraph::from_tasks([task]).unwrap();
    let task_store = FileTaskStore::from_path(root.join("tasks.snapshot"));
    task_store.save(&graph).unwrap();
    let manager = WorktreeManager::new(&root).unwrap();
    manager
        .create(&WorktreeSpec::new(task_id.clone(), "HEAD"))
        .unwrap();
    let mut audit_store = FileAuditStore::open(root.join("audit.log")).unwrap();

    let error = execute_process_persisted(
        &root,
        &task_store,
        &mut audit_store,
        &task_id,
        &FailingAdapter,
        &[],
    )
    .unwrap_err();
    assert!(matches!(error, SliceError::Policy(_)));
    assert_eq!(
        task_store
            .load()
            .unwrap()
            .unwrap()
            .get(&task_id)
            .unwrap()
            .state(),
        TaskState::Failed
    );
    assert_eq!(audit_store.records().len(), 3);
    assert_eq!(audit_store.records()[2].event().event_id(), "agent-failed");
    let _ = manager.retire(&task_id);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn foreground_launch_creates_worktree_and_preserves_failure_evidence() {
    let root = unique_temp_repo();
    git(&root, &["init", "-q"]);
    git(
        &root,
        &["config", "user.email", "agentforge@example.invalid"],
    );
    git(&root, &["config", "user.name", "AgentForge Test"]);
    std::fs::write(root.join("README.md"), "fixture\n").unwrap();
    git(&root, &["add", "README.md"]);
    git(&root, &["commit", "-qm", "fixture"]);
    let mut task = AgentTask::new(
        "P2-M015-T0001",
        "P2-M015",
        AgentRole::Implementer,
        "foreground launch fixture",
    );
    task.capabilities = vec![Capability::RunLocalCommands];
    task.allowed_paths = vec!["README.md".into()];
    let task_id = TaskId::parse(task.task_id.clone()).unwrap();
    let graph = TaskGraph::from_tasks([task]).unwrap();
    let task_store = FileTaskStore::from_path(root.join("tasks.snapshot"));
    task_store.save(&graph).unwrap();
    let mut audit_store = FileAuditStore::open(root.join("audit.log")).unwrap();

    let error = launch_process_persisted(
        &root,
        &task_store,
        &mut audit_store,
        &task_id,
        &FailingAdapter,
        &[],
        "HEAD",
    )
    .unwrap_err();
    assert!(matches!(error, SliceError::Policy(_)));
    assert_eq!(
        audit_store.records()[0].event().kind(),
        AuditEventKind::WorktreeObserved
    );
    assert_eq!(
        audit_store.records()[0]
            .event()
            .fields()
            .get("mode")
            .map(String::as_str),
        Some("created")
    );
    assert_eq!(
        task_store
            .load()
            .unwrap()
            .unwrap()
            .get(&task_id)
            .unwrap()
            .state(),
        TaskState::Failed
    );
    let manager = WorktreeManager::new(&root).unwrap();
    assert!(manager.inspect(&task_id).unwrap().is_some());
    manager.retire(&task_id).unwrap();
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn foreground_launch_rejects_missing_approval_before_worktree_creation() {
    let root = unique_temp_repo();
    git(&root, &["init", "-q"]);
    git(
        &root,
        &["config", "user.email", "agentforge@example.invalid"],
    );
    git(&root, &["config", "user.name", "AgentForge Test"]);
    std::fs::write(root.join("README.md"), "fixture\n").unwrap();
    git(&root, &["add", "README.md"]);
    git(&root, &["commit", "-qm", "fixture"]);
    let mut task = AgentTask::new(
        "P2-M015-T0002",
        "P2-M015",
        AgentRole::Implementer,
        "approval fixture",
    );
    task.capabilities = vec![Capability::RunLocalCommands];
    task.required_approvals = vec![ApprovalBoundary::ActivateImplementationPlan];
    let task_id = TaskId::parse(task.task_id.clone()).unwrap();
    let task_store = FileTaskStore::from_path(root.join("tasks.snapshot"));
    task_store
        .save(&TaskGraph::from_tasks([task]).unwrap())
        .unwrap();
    let mut audit_store = FileAuditStore::open(root.join("audit.log")).unwrap();

    let error = launch_process_persisted(
        &root,
        &task_store,
        &mut audit_store,
        &task_id,
        &FailingAdapter,
        &[],
        "HEAD",
    )
    .unwrap_err();
    assert!(matches!(error, SliceError::Preflight(reason) if reason.contains("missing approval")));
    assert!(
        WorktreeManager::new(&root)
            .unwrap()
            .inspect(&task_id)
            .unwrap()
            .is_none()
    );
    assert!(audit_store.records().is_empty());
    std::fs::remove_dir_all(root).unwrap();
}

fn unique_temp_repo() -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!("agentforge-orchestrator-{stamp}"));
    std::fs::create_dir_all(&path).unwrap();
    path
}

fn git(root: &std::path::Path, args: &[&str]) {
    let output = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {:?}: {}",
        args,
        String::from_utf8_lossy(&output.stderr)
    );
}
