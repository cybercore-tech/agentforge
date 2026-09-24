//! Concurrent batch launch through the single state coordinator.
use agentforge_adapter::{
    AdapterError, AdapterRequest, AgentAdapter, ExecutionReport, ProcessAdapter,
    ProcessAdapterConfig,
};
use agentforge_audit::{AuditEventKind, AuditStore, FileAuditStore};
use agentforge_core::agent::{AgentRole, AgentTask, ApprovalBoundary, Capability};
use agentforge_core::task::{TaskGraph, TaskId, TaskState};
use agentforge_orchestrator::{BatchTaskOutcome, SliceError, launch_batch_persisted};
use agentforge_scheduler::DeferReason;
use agentforge_state::{FileTaskStore, TaskStore};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

static TEMPORARY_ROOT_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Succeeds only if `expected` agents are inside `execute` at the same time, which cannot happen
/// when tasks run one after another.
struct RendezvousAdapter {
    inner: ProcessAdapter,
    arrived: AtomicUsize,
    expected: usize,
}

impl AgentAdapter for RendezvousAdapter {
    fn id(&self) -> &str {
        "rendezvous"
    }

    fn execute(&self, request: AdapterRequest<'_>) -> Result<ExecutionReport, AdapterError> {
        self.arrived.fetch_add(1, Ordering::SeqCst);
        let deadline = Instant::now() + Duration::from_secs(10);
        while self.arrived.load(Ordering::SeqCst) < self.expected {
            if Instant::now() >= deadline {
                return Err(AdapterError::CapturePoisoned);
            }
            thread::sleep(Duration::from_millis(5));
        }
        self.inner.execute(request)
    }
}

/// Fails one named task and delegates the rest.
struct SelectiveAdapter {
    inner: ProcessAdapter,
    fail: &'static str,
}

impl AgentAdapter for SelectiveAdapter {
    fn id(&self) -> &str {
        "selective"
    }

    fn execute(&self, request: AdapterRequest<'_>) -> Result<ExecutionReport, AdapterError> {
        if request.task.task_id == self.fail {
            return Err(AdapterError::CapturePoisoned);
        }
        self.inner.execute(request)
    }
}

fn true_adapter() -> ProcessAdapter {
    ProcessAdapter::new(ProcessAdapterConfig::new("true", "/usr/bin/true")).unwrap()
}

fn task(id: &str, path: &str) -> AgentTask {
    let mut task = AgentTask::new(id, "P1-M006", AgentRole::Implementer, "batch fixture");
    task.capabilities = vec![Capability::RunLocalCommands];
    task.allowed_paths = vec![path.into()];
    task
}

struct Project {
    root: PathBuf,
    store: FileTaskStore,
}

impl Project {
    fn new(tasks: Vec<AgentTask>, gates: &[(&str, &str)]) -> Self {
        let root = std::env::temp_dir().join(format!(
            "agentforge-batch-{}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            TEMPORARY_ROOT_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&root).unwrap();
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
        let store = FileTaskStore::from_path(root.join("tasks.snapshot"));
        store.save(&TaskGraph::from_tasks(tasks).unwrap()).unwrap();
        Self { root, store }
    }

    fn launch<A: AgentAdapter + Sync>(
        &self,
        adapter: &A,
        approvals: &BTreeMap<TaskId, Vec<ApprovalBoundary>>,
        max: usize,
    ) -> (
        Result<agentforge_orchestrator::BatchLaunch, SliceError>,
        FileAuditStore,
    ) {
        let mut audit = FileAuditStore::open(self.root.join("audit.log")).unwrap();
        let result = launch_batch_persisted(
            &self.root,
            &self.store,
            &mut audit,
            adapter,
            approvals,
            "HEAD",
            max,
        );
        (result, audit)
    }

    fn state(&self, id: &str) -> TaskState {
        self.store
            .load()
            .unwrap()
            .unwrap()
            .get(&TaskId::parse(id).unwrap())
            .unwrap()
            .state()
    }

    fn worktree_exists(&self, id: &str) -> bool {
        self.root.join(".forge/worktrees").join(id).exists()
    }
}

impl Drop for Project {
    fn drop(&mut self) {
        // Worktrees live inside the root, so removing it is sufficient for these fixtures.
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn git(root: &Path, args: &[&str]) {
    let status = Command::new("git")
        .args(args)
        .current_dir(root)
        .status()
        .unwrap();
    assert!(status.success(), "git {args:?}");
}

fn count(audit: &FileAuditStore, kind: AuditEventKind) -> usize {
    audit
        .records()
        .iter()
        .filter(|record| record.event().kind() == kind)
        .count()
}

#[test]
fn disjoint_tasks_run_concurrently_and_overlap_is_deferred() {
    let project = Project::new(
        vec![
            task("P1-M006-T0001", "src"),
            task("P1-M006-T0002", "docs"),
            task("P1-M006-T0003", "src/lib.rs"),
        ],
        &[],
    );
    let adapter = RendezvousAdapter {
        inner: true_adapter(),
        arrived: AtomicUsize::new(0),
        expected: 2,
    };
    let (result, audit) = project.launch(&adapter, &BTreeMap::new(), 4);
    let batch = result.unwrap();
    assert!(batch.succeeded(), "{batch:?}");
    let launched = batch
        .outcomes
        .iter()
        .map(|outcome| outcome.task_id().as_str())
        .collect::<Vec<_>>();
    assert_eq!(launched, ["P1-M006-T0001", "P1-M006-T0002"]);
    assert_eq!(batch.deferred.len(), 1);
    assert_eq!(batch.deferred[0].task_id.as_str(), "P1-M006-T0003");
    assert!(matches!(
        &batch.deferred[0].reason,
        DeferReason::Overlap { owner, .. } if owner.as_str() == "P1-M006-T0001"
    ));
    // Both transitions survive: no lost update from concurrent execution.
    assert_eq!(project.state("P1-M006-T0001"), TaskState::Running);
    assert_eq!(project.state("P1-M006-T0002"), TaskState::Running);
    assert_eq!(project.state("P1-M006-T0003"), TaskState::Pending);
    assert_eq!(count(&audit, AuditEventKind::AgentStarted), 2);
    assert_eq!(count(&audit, AuditEventKind::AgentFinished), 2);
    assert_eq!(count(&audit, AuditEventKind::WorktreeObserved), 2);
}

#[test]
fn a_task_missing_its_approval_is_skipped_without_blocking_siblings() {
    let mut gated = task("P1-M006-T0002", "docs");
    gated.required_approvals = vec![ApprovalBoundary::ActivateImplementationPlan];
    let project = Project::new(vec![task("P1-M006-T0001", "src"), gated], &[]);
    let (result, _) = project.launch(&true_adapter(), &BTreeMap::new(), 4);
    let batch = result.unwrap();
    assert!(!batch.succeeded());
    assert!(matches!(
        &batch.outcomes[1],
        BatchTaskOutcome::Skipped { task_id, reason }
            if task_id.as_str() == "P1-M006-T0002" && reason.contains("approval")
    ));
    assert_eq!(project.state("P1-M006-T0001"), TaskState::Running);
    assert_eq!(project.state("P1-M006-T0002"), TaskState::Pending);
    assert!(!project.worktree_exists("P1-M006-T0002"));

    let approvals = BTreeMap::from([(
        TaskId::parse("P1-M006-T0002").unwrap(),
        vec![ApprovalBoundary::ActivateImplementationPlan],
    )]);
    let (retried, _) = project.launch(&true_adapter(), &approvals, 4);
    assert!(retried.unwrap().succeeded());
    assert_eq!(project.state("P1-M006-T0002"), TaskState::Running);
}

#[test]
fn adapter_failure_is_isolated_to_its_task() {
    let project = Project::new(
        vec![task("P1-M006-T0001", "src"), task("P1-M006-T0002", "docs")],
        &[],
    );
    let adapter = SelectiveAdapter {
        inner: true_adapter(),
        fail: "P1-M006-T0001",
    };
    let (result, audit) = project.launch(&adapter, &BTreeMap::new(), 4);
    let batch = result.unwrap();
    assert!(matches!(
        &batch.outcomes[0],
        BatchTaskOutcome::AdapterFailed { task_id, .. } if task_id.as_str() == "P1-M006-T0001"
    ));
    assert!(batch.outcomes[1].succeeded());
    assert_eq!(project.state("P1-M006-T0001"), TaskState::Failed);
    assert_eq!(project.state("P1-M006-T0002"), TaskState::Running);
    assert_eq!(count(&audit, AuditEventKind::FailureClassified), 1);
}

#[test]
fn gate_failures_fail_each_task_with_evidence() {
    let project = Project::new(
        vec![task("P1-M006-T0001", "src"), task("P1-M006-T0002", "docs")],
        &[("check", "version=1\nexecutable=/usr/bin/false\n")],
    );
    let (result, audit) = project.launch(&true_adapter(), &BTreeMap::new(), 4);
    let batch = result.unwrap();
    assert!(!batch.succeeded());
    assert_eq!(project.state("P1-M006-T0001"), TaskState::Failed);
    assert_eq!(project.state("P1-M006-T0002"), TaskState::Failed);
    assert_eq!(count(&audit, AuditEventKind::GateFinished), 2);
    assert_eq!(count(&audit, AuditEventKind::FailureClassified), 2);
}

#[test]
fn malformed_gate_profile_aborts_before_any_worktree() {
    let project = Project::new(
        vec![task("P1-M006-T0001", "src")],
        &[("broken", "version=1\nexecutable=relative\n")],
    );
    let (result, audit) = project.launch(&true_adapter(), &BTreeMap::new(), 4);
    assert!(matches!(
        result,
        Err(SliceError::Preflight(reason)) if reason.contains("gate configuration")
    ));
    assert!(!project.worktree_exists("P1-M006-T0001"));
    assert_eq!(project.state("P1-M006-T0001"), TaskState::Pending);
    assert!(audit.records().is_empty());
}

#[test]
fn capacity_limits_the_batch() {
    let project = Project::new(
        vec![
            task("P1-M006-T0001", "a"),
            task("P1-M006-T0002", "b"),
            task("P1-M006-T0003", "c"),
        ],
        &[],
    );
    let (result, _) = project.launch(&true_adapter(), &BTreeMap::new(), 1);
    let batch = result.unwrap();
    assert_eq!(batch.outcomes.len(), 1);
    assert_eq!(batch.deferred.len(), 2);
    assert!(
        batch
            .deferred
            .iter()
            .all(|deferred| deferred.reason == DeferReason::Capacity)
    );
}
