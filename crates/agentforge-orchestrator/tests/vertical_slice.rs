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
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static TEMPORARY_ROOT_COUNTER: AtomicU64 = AtomicU64::new(0);

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
    // P0-M015: every event from the run records a real, ordered wall-clock time.
    let times = execution
        .audit
        .records()
        .iter()
        .map(|record| record.event().timestamp())
        .collect::<Vec<_>>();
    assert!(
        times
            .iter()
            .all(|time| *time >= agentforge_audit::UNSET_TIMESTAMP_BELOW),
        "{times:?}"
    );
    assert!(times.windows(2).all(|pair| pair[0] <= pair[1]), "{times:?}");
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

#[test]
fn leased_tasks_are_refused_before_any_local_side_effect() {
    use agentforge_core::remote::{
        LeaseBook, LeaseId, RemoteWorkerDescriptor, RemoteWorkerId, WorkerCapability,
    };
    use agentforge_state::{FileLeaseStore, LeaseStore};
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
        "P4-M004-T0001",
        "P4-M004",
        AgentRole::Implementer,
        "leased fixture",
    );
    task.capabilities = vec![Capability::RunLocalCommands];
    task.allowed_paths = vec!["README.md".into()];
    let task_id = TaskId::parse(task.task_id.clone()).unwrap();
    let task_store = FileTaskStore::from_path(root.join("tasks.snapshot"));
    task_store
        .save(&TaskGraph::from_tasks([task]).unwrap())
        .unwrap();
    let worker = RemoteWorkerDescriptor::new(
        RemoteWorkerId::parse("w-a").unwrap(),
        "linux-x86_64",
        vec![WorkerCapability::parse("rust").unwrap()],
        1,
    )
    .unwrap();
    let now = agentforge_orchestrator::wall_clock_ms();
    let mut book = LeaseBook::new();
    book.grant(
        &worker,
        LeaseId::parse("P4-M004-T0001.L1").unwrap(),
        task_id.clone(),
        now,
        now + 600_000,
    )
    .unwrap();
    let leases = FileLeaseStore::for_project_root(&root);
    leases.save(&book).unwrap();
    let mut audit_store = FileAuditStore::open(root.join("audit.log")).unwrap();

    let launched = launch_process_persisted(
        &root,
        &task_store,
        &mut audit_store,
        &task_id,
        &FailingAdapter,
        &[],
        "HEAD",
    )
    .unwrap_err();
    assert!(
        matches!(&launched, SliceError::Preflight(reason) if reason.contains("is leased to worker w-a")),
        "{launched}"
    );
    let manager = WorktreeManager::new(&root).unwrap();
    assert!(manager.inspect(&task_id).unwrap().is_none());

    manager
        .create(&WorktreeSpec::new(task_id.clone(), "HEAD"))
        .unwrap();
    let executed = execute_process_persisted(
        &root,
        &task_store,
        &mut audit_store,
        &task_id,
        &FailingAdapter,
        &[],
    )
    .unwrap_err();
    assert!(matches!(executed, SliceError::Preflight(_)), "{executed}");
    assert!(audit_store.records().is_empty());
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

    // Once released, the task runs locally again.
    let lease_id = LeaseId::parse("P4-M004-T0001.L1").unwrap();
    book.release(&lease_id, worker.worker_id(), 1, now + 1)
        .unwrap();
    leases.save(&book).unwrap();
    let after = execute_process_persisted(
        &root,
        &task_store,
        &mut audit_store,
        &task_id,
        &FailingAdapter,
        &[],
    )
    .unwrap_err();
    assert!(!matches!(after, SliceError::Preflight(_)), "{after}");
    let _ = manager.retire(&task_id);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn only_the_exact_lease_holder_can_run_a_leased_task() {
    use agentforge_core::remote::{
        LeaseBook, LeaseId, RemoteWorkerDescriptor, RemoteWorkerId, WorkerCapability,
    };
    use agentforge_orchestrator::{LeaseClaim, launch_leased_process_persisted};
    use agentforge_state::{FileLeaseStore, LeaseStore};
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
    let make = |id: &str, path: &str| {
        let mut task = AgentTask::new(id, "P4-M005", AgentRole::Implementer, "claim fixture");
        task.capabilities = vec![Capability::RunLocalCommands];
        task.allowed_paths = vec![path.into()];
        task
    };
    let task_store = FileTaskStore::from_path(root.join("tasks.snapshot"));
    task_store
        .save(
            &TaskGraph::from_tasks([
                make("P4-M005-T0001", "src"),
                make("P4-M005-T0002", "src/other"),
                make("P4-M005-T0003", "docs"),
            ])
            .unwrap(),
        )
        .unwrap();
    let worker = |id: &str| {
        RemoteWorkerDescriptor::new(
            RemoteWorkerId::parse(id).unwrap(),
            "linux-x86_64",
            vec![WorkerCapability::parse("rust").unwrap()],
            4,
        )
        .unwrap()
    };
    let (w_a, w_b) = (worker("w-a"), worker("w-b"));
    let now = agentforge_orchestrator::wall_clock_ms();
    let lease = |value: &str| LeaseId::parse(value).unwrap();
    let task = |value: &str| TaskId::parse(value).unwrap();
    let mut book = LeaseBook::new();
    book.grant(
        &w_a,
        lease("P4-M005-T0001.L1"),
        task("P4-M005-T0001"),
        now,
        now + 600_000,
    )
    .unwrap();
    book.grant(
        &w_b,
        lease("P4-M005-T0002.L1"),
        task("P4-M005-T0002"),
        now,
        now + 600_000,
    )
    .unwrap();
    // T0003's lease is already past due when the worker claims it.
    book.grant(
        &w_a,
        lease("P4-M005-T0003.L1"),
        task("P4-M005-T0003"),
        now - 10_000,
        now - 5_000,
    )
    .unwrap();
    let store = FileLeaseStore::for_project_root(&root);
    store.save(&book).unwrap();
    let mut audit_store = FileAuditStore::open(root.join("audit.log")).unwrap();
    let claim = |lease_id: &str, worker_id: &str, generation: u64| LeaseClaim {
        lease_id: lease(lease_id),
        worker_id: RemoteWorkerId::parse(worker_id).unwrap(),
        generation,
    };
    let mut attempt = |task_id: &str, claim: LeaseClaim| {
        launch_leased_process_persisted(
            &root,
            &task_store,
            &mut audit_store,
            &task(task_id),
            &FailingAdapter,
            &[],
            "HEAD",
            &claim,
        )
        .unwrap_err()
    };
    for (task_id, wrong) in [
        ("P4-M005-T0001", claim("P4-M005-T0001.L1", "w-b", 1)),
        ("P4-M005-T0001", claim("P4-M005-T0001.L1", "w-a", 2)),
        ("P4-M005-T0001", claim("P4-M005-T0002.L1", "w-a", 1)),
        ("P4-M005-T0003", claim("P4-M005-T0003.L1", "w-a", 1)),
    ] {
        let error = attempt(task_id, wrong.clone());
        assert!(
            matches!(&error, SliceError::Preflight(reason) if reason.contains("does not match an active lease")),
            "{wrong:?}: {error}"
        );
    }
    // The right claim, but T0001's `src` overlaps T0002's leased `src/other`.
    let overlap = attempt("P4-M005-T0001", claim("P4-M005-T0001.L1", "w-a", 1));
    assert!(
        matches!(&overlap, SliceError::Preflight(reason) if reason.contains("overlaps task P4-M005-T0002")),
        "{overlap}"
    );
    let manager = WorktreeManager::new(&root).unwrap();
    assert!(manager.inspect(&task("P4-M005-T0001")).unwrap().is_none());
    assert!(
        FileAuditStore::open(root.join("audit.log"))
            .unwrap()
            .records()
            .is_empty(),
        "refusals leave no evidence"
    );

    book.release(&lease("P4-M005-T0002.L1"), w_b.worker_id(), 1, now + 1)
        .unwrap();
    store.save(&book).unwrap();
    let allowed = attempt("P4-M005-T0001", claim("P4-M005-T0001.L1", "w-a", 1));
    assert!(!matches!(allowed, SliceError::Preflight(_)), "{allowed}");
    assert!(manager.inspect(&task("P4-M005-T0001")).unwrap().is_some());
    let _ = manager.retire(&task("P4-M005-T0001"));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn foreground_launch_does_not_require_post_execution_approvals() {
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
        "P1-M008-T0001",
        "P1-M008",
        AgentRole::Implementer,
        "post-review approval fixture",
    );
    task.capabilities = vec![
        Capability::RunLocalCommands,
        Capability::MergeProtectedBranch,
    ];
    task.required_approvals = vec![
        ApprovalBoundary::ActivateImplementationPlan,
        ApprovalBoundary::MergeProtectedBranch,
    ];
    let task_id = TaskId::parse(task.task_id.clone()).unwrap();
    let task_store = FileTaskStore::from_path(root.join("tasks.snapshot"));
    task_store
        .save(&TaskGraph::from_tasks([task]).unwrap())
        .unwrap();
    let mut audit_store = FileAuditStore::open(root.join("audit.log")).unwrap();

    // Only the pre-execution approval is recorded; the merge approval comes after review.
    let error = launch_process_persisted(
        &root,
        &task_store,
        &mut audit_store,
        &task_id,
        &FailingAdapter,
        &[ApprovalBoundary::ActivateImplementationPlan],
        "HEAD",
    )
    .unwrap_err();
    assert!(
        !matches!(error, SliceError::Preflight(_)),
        "launch was refused before the agent ran: {error}"
    );
    assert!(
        WorktreeManager::new(&root)
            .unwrap()
            .inspect(&task_id)
            .unwrap()
            .is_some()
    );
    WorktreeManager::new(&root)
        .unwrap()
        .retire(&task_id)
        .unwrap();
    std::fs::remove_dir_all(root).unwrap();
}

fn unique_temp_repo() -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "agentforge-orchestrator-{}-{stamp}-{}",
        std::process::id(),
        TEMPORARY_ROOT_COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
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

struct PreparedRepo {
    root: PathBuf,
    task_id: TaskId,
    task_store: FileTaskStore,
    manager: WorktreeManager,
}

fn prepared_repo(gates: &[(&str, &str)]) -> PreparedRepo {
    prepared_repo_requiring(gates, &[])
}

fn prepared_repo_requiring(gates: &[(&str, &str)], required: &[&str]) -> PreparedRepo {
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
    let gate_dir = root.join(agentforge_gate::GATE_PROFILE_RELATIVE_PATH);
    for (id, body) in gates {
        std::fs::create_dir_all(&gate_dir).unwrap();
        std::fs::write(gate_dir.join(format!("{id}.conf")), body).unwrap();
    }
    let mut task = AgentTask::new(
        "P1-M004-T0001",
        "P1-M004",
        AgentRole::Implementer,
        "gate fixture",
    );
    task.capabilities = vec![Capability::RunLocalCommands];
    task.allowed_paths = vec!["README.md".into()];
    task.required_gates = required.iter().map(|name| (*name).to_owned()).collect();
    let task_id = TaskId::parse(task.task_id.clone()).unwrap();
    let task_store = FileTaskStore::from_path(root.join("tasks.snapshot"));
    task_store
        .save(&TaskGraph::from_tasks([task]).unwrap())
        .unwrap();
    let manager = WorktreeManager::new(&root).unwrap();
    manager
        .create(&WorktreeSpec::new(task_id.clone(), "HEAD"))
        .unwrap();
    PreparedRepo {
        root,
        task_id,
        task_store,
        manager,
    }
}

fn run_prepared(
    repo: &PreparedRepo,
    agent: &str,
) -> (
    Result<agentforge_orchestrator::ProcessExecution, SliceError>,
    FileAuditStore,
) {
    let adapter = ProcessAdapter::new(ProcessAdapterConfig::new("agent", agent)).unwrap();
    let mut audit_store = FileAuditStore::open(repo.root.join("audit.log")).unwrap();
    let result = execute_process_persisted(
        &repo.root,
        &repo.task_store,
        &mut audit_store,
        &repo.task_id,
        &adapter,
        &[],
    );
    (result, audit_store)
}

fn task_state(repo: &PreparedRepo) -> TaskState {
    repo.task_store
        .load()
        .unwrap()
        .unwrap()
        .records()
        .next()
        .unwrap()
        .state()
}

fn events_of(store: &FileAuditStore, kind: AuditEventKind) -> Vec<AuditEvent> {
    store
        .records()
        .iter()
        .map(|record| record.event().clone())
        .filter(|event| event.kind() == kind)
        .collect()
}

fn cleanup(repo: PreparedRepo) {
    let _ = repo.manager.retire(&repo.task_id);
    std::fs::remove_dir_all(repo.root).unwrap();
}

#[test]
fn passing_gates_record_evidence_and_keep_the_task_running() {
    let repo = prepared_repo(&[("check", "version=1\nexecutable=/usr/bin/true\n")]);
    let (result, audit) = run_prepared(&repo, "/usr/bin/true");
    let execution = result.unwrap();
    assert_eq!(execution.gates.len(), 1);
    assert!(execution.gates_passed());
    assert_eq!(task_state(&repo), TaskState::Running);
    let gates = events_of(&audit, AuditEventKind::GateFinished);
    assert_eq!(gates.len(), 1);
    assert_eq!(
        gates[0].fields().get("gate").map(String::as_str),
        Some("check")
    );
    assert_eq!(
        gates[0].fields().get("outcome").map(String::as_str),
        Some("passed")
    );
    assert!(events_of(&audit, AuditEventKind::FailureClassified).is_empty());
    cleanup(repo);
}

#[test]
fn a_failing_gate_fails_the_task_with_durable_evidence() {
    let repo = prepared_repo(&[
        ("a-lint", "version=1\nexecutable=/usr/bin/false\n"),
        ("b-test", "version=1\nexecutable=/usr/bin/true\n"),
    ]);
    let (result, audit) = run_prepared(&repo, "/usr/bin/true");
    let execution = result.unwrap();
    assert!(!execution.gates_passed());
    let names = execution
        .gates
        .iter()
        .map(|gate| gate.name().to_owned())
        .collect::<Vec<_>>();
    assert_eq!(
        names,
        ["a-lint", "b-test"],
        "all gates run in lexical order"
    );
    assert_eq!(task_state(&repo), TaskState::Failed);
    let outcomes = events_of(&audit, AuditEventKind::GateFinished)
        .iter()
        .map(|event| event.fields().get("outcome").cloned().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(outcomes, ["failed", "passed"]);
    let failures = events_of(&audit, AuditEventKind::FailureClassified);
    assert_eq!(failures.len(), 1);
    assert_eq!(
        failures[0].fields().get("stage").map(String::as_str),
        Some("gates")
    );
    assert_eq!(
        failures[0].fields().get("gate").map(String::as_str),
        Some("a-lint")
    );
    cleanup(repo);
}

#[test]
fn malformed_gate_profile_fails_before_the_task_runs() {
    let repo = prepared_repo(&[("broken", "version=1\nexecutable=relative/path\n")]);
    let (result, audit) = run_prepared(&repo, "/usr/bin/true");
    let error = result.unwrap_err();
    assert!(
        matches!(&error, SliceError::Preflight(reason) if reason.contains("gate configuration")),
        "{error:?}"
    );
    assert_eq!(task_state(&repo), TaskState::Pending);
    assert!(audit.records().is_empty());
    cleanup(repo);
}

#[test]
fn gates_are_skipped_when_the_agent_exits_nonzero() {
    let repo = prepared_repo(&[("check", "version=1\nexecutable=/usr/bin/false\n")]);
    let (result, audit) = run_prepared(&repo, "/usr/bin/false");
    let execution = result.unwrap();
    assert!(execution.gates.is_empty());
    assert_eq!(task_state(&repo), TaskState::Running);
    assert!(events_of(&audit, AuditEventKind::GateFinished).is_empty());
    cleanup(repo);
}

fn gate_names(execution: &agentforge_orchestrator::ProcessExecution) -> Vec<String> {
    execution
        .gates
        .iter()
        .map(|gate| gate.name().to_owned())
        .collect()
}

#[test]
fn required_gates_select_exactly_the_declared_gates() {
    let repo = prepared_repo_requiring(
        &[
            ("a-check", "version=1\nexecutable=/usr/bin/true\n"),
            ("b-check", "version=1\nexecutable=/usr/bin/true\n"),
        ],
        &["b-check"],
    );
    let (result, audit) = run_prepared(&repo, "/usr/bin/true");
    let execution = result.unwrap();
    assert_eq!(gate_names(&execution), ["b-check"]);
    let gates = events_of(&audit, AuditEventKind::GateFinished);
    assert_eq!(gates.len(), 1);
    assert_eq!(
        gates[0].fields().get("gate").map(String::as_str),
        Some("b-check")
    );
    assert_eq!(task_state(&repo), TaskState::Running);
    cleanup(repo);
}

#[test]
fn a_task_without_required_gates_runs_every_gate() {
    let repo = prepared_repo(&[
        ("a-check", "version=1\nexecutable=/usr/bin/true\n"),
        ("b-check", "version=1\nexecutable=/usr/bin/true\n"),
    ]);
    let (result, audit) = run_prepared(&repo, "/usr/bin/true");
    let execution = result.unwrap();
    assert_eq!(gate_names(&execution), ["a-check", "b-check"]);
    assert_eq!(events_of(&audit, AuditEventKind::GateFinished).len(), 2);
    cleanup(repo);
}

#[test]
fn an_unconfigured_required_gate_fails_preflight_before_the_agent_runs() {
    let repo = prepared_repo_requiring(
        &[("a-check", "version=1\nexecutable=/usr/bin/true\n")],
        &["a-check", "z-missing"],
    );
    let (result, audit) = run_prepared(&repo, "/usr/bin/true");
    let error = result.unwrap_err();
    assert!(
        matches!(&error, SliceError::Preflight(reason) if reason.contains("z-missing")),
        "{error:?}"
    );
    assert_eq!(task_state(&repo), TaskState::Pending);
    assert!(audit.records().is_empty(), "no agent start is recorded");
    cleanup(repo);
}

#[test]
fn a_duplicate_required_gate_runs_once() {
    let repo = prepared_repo_requiring(
        &[
            ("a-check", "version=1\nexecutable=/usr/bin/true\n"),
            ("b-check", "version=1\nexecutable=/usr/bin/true\n"),
        ],
        &["b-check", "a-check", "b-check"],
    );
    let (result, audit) = run_prepared(&repo, "/usr/bin/true");
    let execution = result.unwrap();
    assert_eq!(
        gate_names(&execution),
        ["a-check", "b-check"],
        "each gate once, in lexical order"
    );
    assert_eq!(events_of(&audit, AuditEventKind::GateFinished).len(), 2);
    cleanup(repo);
}

#[test]
fn agent_output_is_persisted_as_evidence_and_referenced_by_the_audit() {
    let failing = prepared_repo(&[]);
    let (result, audit) = run_prepared(&failing, "/usr/bin/false");
    let execution = result.unwrap();
    assert!(
        !execution.succeeded(),
        "a non-zero agent exit is not success"
    );
    let stdout = execution.evidence.stdout_log.clone().expect("stdout log");
    let stderr = execution.evidence.stderr_log.clone().expect("stderr log");
    assert!(stdout.starts_with(agentforge_orchestrator::EVIDENCE_RELATIVE_PATH));
    assert!(failing.root.join(&stdout).is_file());
    assert!(failing.root.join(&stderr).is_file());
    let finished = events_of(&audit, AuditEventKind::AgentFinished);
    assert_eq!(finished.len(), 1);
    let fields = finished[0].fields();
    assert_eq!(fields.get("exit_code").map(String::as_str), Some("1"));
    assert_eq!(
        fields.get("termination").map(String::as_str),
        Some("exited")
    );
    assert_eq!(
        fields.get("stdout_log").map(String::as_str),
        Some(stdout.to_string_lossy().as_ref())
    );
    assert!(
        !failing
            .root
            .join(".forge/worktrees")
            .join(failing.task_id.as_str())
            .join(".forge/evidence")
            .exists(),
        "evidence never lands inside the task worktree"
    );
    cleanup(failing);

    let passing = prepared_repo(&[]);
    let (result, _) = run_prepared(&passing, "/usr/bin/true");
    let execution = result.unwrap();
    assert!(execution.succeeded());
    assert!(
        passing
            .root
            .join(execution.evidence.stdout_log.expect("stdout log"))
            .is_file()
    );
    cleanup(passing);
}
