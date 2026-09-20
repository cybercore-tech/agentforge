//! End-to-end ordering checks for the bounded vertical slice.
use agentforge_adapter::{ProcessAdapter, ProcessAdapterConfig};
use agentforge_audit::AuditStore;
use agentforge_audit::FileAuditStore;
use agentforge_core::agent::{AgentRole, AgentTask, Capability};
use agentforge_core::task::{TaskGraph, TaskId, TaskState};
use agentforge_orchestrator::{
    SliceError, SliceEvidence, SliceStage, execute_process_persisted, run,
};
use agentforge_state::{FileTaskStore, TaskStore};
use agentforge_worktree::{WorktreeManager, WorktreeSpec};
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

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
    assert_eq!(audit_store.records().len(), 3);
    assert_eq!(execution.report.task_id().as_str(), task.task_id);
    let _ = manager.retire(&task_id);
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
