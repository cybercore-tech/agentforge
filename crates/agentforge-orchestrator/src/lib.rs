//! A bounded, provider-neutral single-agent orchestration flow.

use agentforge_adapter::{AdapterRequest, AgentAdapter, ExecutionReport, ExecutionTermination};
use agentforge_audit::{AuditEvent, AuditEventKind, AuditLog};
use agentforge_audit::{AuditStore, FileAuditStore};
use agentforge_core::agent::{AgentTask, ApprovalBoundary, Capability};
use agentforge_core::remote::{LeaseBook, LeaseId, RemoteWorkerId};
use agentforge_core::task::{TaskGraph, TaskId, TaskState};
use agentforge_gate::{GateDefinition, GateOutcome, GateProfileStore, GateReport, GateRunner};
use agentforge_policy::{ApprovalGrant, PolicyDecision, PolicyEngine, PolicyRequest};
use agentforge_scheduler::plan_launch_batch;
pub use agentforge_scheduler::{DeferReason, DeferredTask};
use agentforge_state::{FileLeaseStore, FileTaskStore, LeaseStore, TaskStore};
use agentforge_worktree::{WorktreeManager, WorktreeSpec, WorktreeStatus};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::{Path, PathBuf};
use std::thread;

/// Ordered stages in the vertical slice.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum SliceStage {
    /// Validate task and capability policy.
    Policy,
    /// Confirm the caller supplied a clean, verified worktree.
    Worktree,
    /// Record provider execution evidence.
    Agent,
    /// Record quality-gate evidence.
    Gates,
    /// Hand off evidence for independent review.
    ReviewHandoff,
}

impl SliceStage {
    /// Stable identity.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Policy => "policy",
            Self::Worktree => "worktree",
            Self::Agent => "agent",
            Self::Gates => "gates",
            Self::ReviewHandoff => "review_handoff",
        }
    }
}

/// Caller-supplied evidence for side-effecting stages.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SliceEvidence {
    /// Worktree identity was re-inspected immediately before execution.
    pub worktree_verified: bool,
    /// Adapter produced bounded process evidence.
    pub agent_reported: bool,
    /// Required gates completed successfully.
    pub gates_passed: bool,
}

/// Successful ordered handoff report. It does not accept the task or mutate state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerticalSliceReport {
    /// Originating task identity.
    pub task_id: String,
    /// Completed stages in canonical order.
    pub stages: Vec<SliceStage>,
    /// Review handoff is ready for an independent reviewer.
    pub review_ready: bool,
}

/// Fail-closed orchestration error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SliceError {
    /// Policy rejected the requested operation.
    Policy(String),
    /// A required evidence boundary was not satisfied.
    MissingEvidence(SliceStage),
    /// Repository/worktree preflight failed before execution.
    Preflight(String),
}

impl fmt::Display for SliceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Policy(reason) => write!(f, "policy denied vertical slice: {reason}"),
            Self::MissingEvidence(stage) => {
                write!(f, "missing evidence at {} stage", stage.as_str())
            }
            Self::Preflight(reason) => write!(f, "repository preflight failed: {reason}"),
        }
    }
}
impl std::error::Error for SliceError {}

/// Validates the real repository boundary before an adapter can be launched.
pub fn preflight_repository(root: impl AsRef<Path>, task: &AgentTask) -> Result<(), SliceError> {
    task.validate()
        .map_err(|e| SliceError::Preflight(e.to_string()))?;
    let manager = WorktreeManager::new(root).map_err(|e| SliceError::Preflight(e.to_string()))?;
    let request = PolicyRequest {
        capability: Capability::RunLocalCommands,
        paths: task.allowed_paths.clone(),
        approval: None,
    };
    if let PolicyDecision::Denied(error) = PolicyEngine.evaluate(task, &request, &[]) {
        return Err(SliceError::Preflight(error.to_string()));
    }
    let _ = manager.project_root();
    Ok(())
}

/// Evidence returned after launching one concrete process adapter.
#[derive(Debug)]
pub struct ProcessExecution {
    /// Bounded adapter evidence; successful exit is not task acceptance.
    pub report: ExecutionReport,
    /// Durable audit log containing ordered stage evidence.
    pub audit: AuditLog,
    /// Project gate results in lexical gate order. Empty when the project declares no gates or
    /// the agent did not exit successfully.
    pub gates: Vec<GateRun>,
    /// Where the agent's full output was persisted.
    pub evidence: AgentEvidence,
}

/// Project-relative locations of one agent run's persisted output.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AgentEvidence {
    /// Captured stdout, relative to the project root.
    pub stdout_log: Option<PathBuf>,
    /// Captured stderr, relative to the project root.
    pub stderr_log: Option<PathBuf>,
    /// Why the output could not be persisted, if it could not.
    pub error: Option<String>,
}

/// Project-relative directory for agent run evidence.
pub const EVIDENCE_RELATIVE_PATH: &str = ".forge/evidence";

/// Returns whether the agent process exited normally with status zero.
#[must_use]
pub fn agent_exited_cleanly(report: &ExecutionReport) -> bool {
    report.termination() == ExecutionTermination::Exited && report.exit_code() == Some(0)
}

impl ProcessExecution {
    /// Returns whether the agent exited cleanly and every executed gate passed.
    #[must_use]
    pub fn succeeded(&self) -> bool {
        agent_exited_cleanly(&self.report) && self.gates_passed()
    }

    /// Returns whether every executed gate passed. True when no gate ran.
    #[must_use]
    pub fn gates_passed(&self) -> bool {
        self.gates.iter().all(GateRun::passed)
    }
}

/// Evidence from one orchestrated gate.
#[derive(Debug)]
pub enum GateRun {
    /// The gate process ran and produced a bounded report.
    Reported(GateReport),
    /// The gate process could not be run; this counts as a failure.
    Errored {
        /// Gate name.
        name: String,
        /// Runner error.
        error: String,
    },
}

impl GateRun {
    /// Returns the gate name.
    #[must_use]
    pub fn name(&self) -> &str {
        match self {
            Self::Reported(report) => report.name(),
            Self::Errored { name, .. } => name,
        }
    }

    /// Returns whether the gate passed.
    #[must_use]
    pub fn passed(&self) -> bool {
        matches!(self, Self::Reported(report) if report.outcome() == GateOutcome::Passed)
    }

    /// Returns a stable outcome label for evidence and operator output.
    #[must_use]
    pub fn outcome_label(&self) -> &'static str {
        match self {
            Self::Reported(report) => match report.outcome() {
                GateOutcome::Passed => "passed",
                GateOutcome::Failed => "failed",
                GateOutcome::TimedOut => "timed_out",
                GateOutcome::OutputLimitExceeded => "output_limit_exceeded",
            },
            Self::Errored { .. } => "error",
        }
    }

    /// Returns the gate exit code when the platform provided one.
    #[must_use]
    pub fn exit_code(&self) -> Option<i32> {
        match self {
            Self::Reported(report) => report.exit_code(),
            Self::Errored { .. } => None,
        }
    }
}

/// Result of preparing a managed worktree and launching one foreground process.
#[derive(Debug)]
pub struct ForegroundLaunch {
    /// Bounded process and audit evidence.
    pub execution: ProcessExecution,
    /// Exact commit resolved from the requested base ref before launch.
    pub base_commit: String,
    /// Verified worktree state observed before the agent started.
    pub worktree: WorktreeStatus,
    /// Whether the worktree was created by this launch.
    pub worktree_created: bool,
}

/// Creates or verifies one task-owned worktree, then runs the persisted process path.
///
/// Validation, task readiness, capabilities, approvals, and exact base resolution happen before
/// any worktree is created or an agent is spawned. Existing owned worktrees are reused only after
/// cleanliness and unresolved-operation checks; unrelated worktrees are never adopted.
pub fn launch_process_persisted<A: AgentAdapter>(
    root: impl AsRef<Path>,
    task_store: &FileTaskStore,
    audit_store: &mut FileAuditStore,
    task_id: &TaskId,
    adapter: &A,
    approvals: &[ApprovalBoundary],
    base_ref: &str,
) -> Result<ForegroundLaunch, SliceError> {
    launch_with_claim(
        root.as_ref(),
        task_store,
        audit_store,
        task_id,
        adapter,
        approvals,
        base_ref,
        None,
    )
}

/// The lease a worker presents to run a leased task (P4-M005).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LeaseClaim {
    /// Claimed lease.
    pub lease_id: LeaseId,
    /// Worker holding the lease.
    pub worker_id: RemoteWorkerId,
    /// Lease generation at claim time.
    pub generation: u64,
}

/// Runs a leased task for its lease holder through the standard launch path.
///
/// Identical to [`launch_process_persisted`], except that the lease guard allows the task when its
/// active lease matches `claim` exactly (lease, worker, and generation). Overlap with any other
/// active lease is still refused, as is a missing, expired, or mismatched lease.
#[allow(clippy::too_many_arguments)]
pub fn launch_leased_process_persisted<A: AgentAdapter>(
    root: impl AsRef<Path>,
    task_store: &FileTaskStore,
    audit_store: &mut FileAuditStore,
    task_id: &TaskId,
    adapter: &A,
    approvals: &[ApprovalBoundary],
    base_ref: &str,
    claim: &LeaseClaim,
) -> Result<ForegroundLaunch, SliceError> {
    launch_with_claim(
        root.as_ref(),
        task_store,
        audit_store,
        task_id,
        adapter,
        approvals,
        base_ref,
        Some(claim),
    )
}

#[allow(clippy::too_many_arguments)]
fn launch_with_claim<A: AgentAdapter>(
    root: &Path,
    task_store: &FileTaskStore,
    audit_store: &mut FileAuditStore,
    task_id: &TaskId,
    adapter: &A,
    approvals: &[ApprovalBoundary],
    base_ref: &str,
    claim: Option<&LeaseClaim>,
) -> Result<ForegroundLaunch, SliceError> {
    let root = root.to_path_buf();
    let graph = task_store
        .load()
        .map_err(|error| SliceError::Preflight(error.to_string()))?
        .ok_or_else(|| SliceError::Preflight("task state snapshot is missing".into()))?;
    let task = graph
        .get(task_id)
        .ok_or_else(|| SliceError::Preflight("task not found".into()))?
        .task()
        .clone();
    task.validate()
        .map_err(|error| SliceError::Preflight(error.to_string()))?;
    if !graph
        .ready_task_ids()
        .map_err(|error| SliceError::Preflight(error.to_string()))?
        .contains(task_id)
    {
        return Err(SliceError::Preflight(
            "task is not ready; dependencies must succeed and state must be pending".into(),
        ));
    }
    validate_launch_policy(&task, approvals)?;
    check_remote_leases(&root, &graph, task_id, claim)?;
    // Required gates are checked before the worktree is created or observed.
    let gates = GateProfileStore::new(&root)
        .list()
        .map_err(|error| SliceError::Preflight(format!("gate configuration: {error}")))?;
    select_gates(&gates, &task)?;

    let manager =
        WorktreeManager::new(&root).map_err(|error| SliceError::Preflight(error.to_string()))?;
    let base_commit = manager
        .resolve_base(base_ref)
        .map_err(|error| SliceError::Preflight(error.to_string()))?;
    let (worktree, worktree_created) = prepare_worktree(&manager, task_id, base_ref)?;

    append_worktree_observation(
        audit_store,
        &task,
        &worktree,
        &base_commit,
        worktree_created,
    )?;
    let execution = execute_with_claim(
        &root,
        task_store,
        audit_store,
        task_id,
        adapter,
        approvals,
        claim,
    )?;
    Ok(ForegroundLaunch {
        execution,
        base_commit,
        worktree,
        worktree_created,
    })
}

/// Result of one concurrent batch launch.
#[derive(Debug)]
pub struct BatchLaunch {
    /// Exact commit resolved from the requested base ref.
    pub base_commit: String,
    /// One outcome per selected task, in canonical task-ID order.
    pub outcomes: Vec<BatchTaskOutcome>,
    /// Ready tasks left for a later batch.
    pub deferred: Vec<DeferredTask>,
}

impl BatchLaunch {
    /// Returns whether every launched task's adapter ran and all of its gates passed, and no
    /// selected task was skipped.
    #[must_use]
    pub fn succeeded(&self) -> bool {
        self.outcomes.iter().all(BatchTaskOutcome::succeeded)
    }
}

/// Outcome for one task selected into a batch.
#[derive(Debug)]
pub enum BatchTaskOutcome {
    /// The agent ran; gates ran when it exited with status zero.
    Launched {
        /// Task identity.
        task_id: TaskId,
        /// Bounded adapter evidence.
        report: ExecutionReport,
        /// Gate results in lexical order.
        gates: Vec<GateRun>,
        /// Task worktree path.
        worktree: PathBuf,
        /// Whether this launch created the worktree.
        worktree_created: bool,
        /// Where the agent's full output was persisted.
        evidence: Box<AgentEvidence>,
    },
    /// The adapter could not run the agent; the task is `failed`.
    AdapterFailed {
        /// Task identity.
        task_id: TaskId,
        /// Adapter error.
        error: String,
    },
    /// The task failed its own preflight and was not started; its state is unchanged.
    Skipped {
        /// Task identity.
        task_id: TaskId,
        /// Preflight failure.
        reason: String,
    },
}

impl BatchTaskOutcome {
    /// Returns the task identity.
    #[must_use]
    pub fn task_id(&self) -> &TaskId {
        match self {
            Self::Launched { task_id, .. }
            | Self::AdapterFailed { task_id, .. }
            | Self::Skipped { task_id, .. } => task_id,
        }
    }

    /// Returns whether the task launched, its agent exited cleanly, and every gate passed.
    #[must_use]
    pub fn succeeded(&self) -> bool {
        matches!(self, Self::Launched { report, gates, .. }
            if agent_exited_cleanly(report) && gates.iter().all(GateRun::passed))
    }
}

struct PreparedTask {
    task_id: TaskId,
    task: AgentTask,
    approvals: Vec<ApprovalBoundary>,
    worktree: PathBuf,
    worktree_created: bool,
    gates: Vec<GateDefinition>,
}

type AgentResult = Result<(ExecutionReport, Vec<GateRun>), String>;

/// Launches up to `max` disjoint ready tasks concurrently.
///
/// Prepare runs sequentially: it validates each task, creates or verifies worktrees, marks tasks
/// `running`, and persists. Execute runs each agent (and its gates) on its own scoped thread with
/// no access to shared state. Finalize applies outcomes in task-ID order and persists once. Only
/// the calling thread ever touches the task snapshot or audit log.
pub fn launch_batch_persisted<A: AgentAdapter + Sync>(
    root: impl AsRef<Path>,
    task_store: &FileTaskStore,
    audit_store: &mut FileAuditStore,
    adapter: &A,
    approvals: &BTreeMap<TaskId, Vec<ApprovalBoundary>>,
    base_ref: &str,
    max: usize,
) -> Result<BatchLaunch, SliceError> {
    let root = root.as_ref().to_path_buf();
    let mut graph = task_store
        .load()
        .map_err(|error| SliceError::Preflight(error.to_string()))?
        .ok_or_else(|| SliceError::Preflight("task state snapshot is missing".into()))?;
    let batch =
        plan_launch_batch(&graph, max).map_err(|error| SliceError::Preflight(error.to_string()))?;
    let gates = GateProfileStore::new(&root)
        .list()
        .map_err(|error| SliceError::Preflight(format!("gate configuration: {error}")))?;
    let manager =
        WorktreeManager::new(&root).map_err(|error| SliceError::Preflight(error.to_string()))?;
    let base_commit = manager
        .resolve_base(base_ref)
        .map_err(|error| SliceError::Preflight(error.to_string()))?;

    // Prepare: sequential, so Git worktree metadata is never written concurrently.
    let mut outcomes = Vec::new();
    let mut prepared = Vec::new();
    let mut log = audit_store.new_attempt_log();
    for task_id in batch.task_ids {
        let task = graph
            .get(&task_id)
            .expect("planned task exists")
            .task()
            .clone();
        let task_approvals = approvals.get(&task_id).cloned().unwrap_or_default();
        let checked = task
            .validate()
            .map_err(|error| SliceError::Preflight(error.to_string()))
            .and_then(|()| validate_launch_policy(&task, &task_approvals))
            .and_then(|()| check_remote_leases(&root, &graph, &task_id, None))
            .and_then(|()| preflight_repository(&root, &task))
            .and_then(|()| select_gates(&gates, &task))
            .and_then(|task_gates| {
                prepare_worktree(&manager, &task_id, base_ref)
                    .map(|(worktree, created)| (worktree, created, task_gates))
            });
        let (worktree, worktree_created, task_gates) = match checked {
            Ok(value) => value,
            Err(error) => {
                outcomes.push(BatchTaskOutcome::Skipped {
                    task_id,
                    reason: error.to_string(),
                });
                continue;
            }
        };
        append_fields(
            &mut log,
            "worktree-observed",
            AuditEventKind::WorktreeObserved,
            &task,
            &[
                ("base_commit", base_commit.as_str()),
                ("head", worktree.head()),
                ("path", &worktree.path().to_string_lossy()),
                (
                    "mode",
                    if worktree_created {
                        "created"
                    } else {
                        "reused"
                    },
                ),
            ],
        )?;
        graph
            .transition(&task_id, TaskState::Running)
            .map_err(|error| SliceError::Preflight(error.to_string()))?;
        append_fields(
            &mut log,
            "task-running",
            AuditEventKind::TaskTransition,
            &task,
            &[("state", "running")],
        )?;
        append_fields(
            &mut log,
            "agent-started",
            AuditEventKind::AgentStarted,
            &task,
            &[("adapter", adapter.id()), ("batch", "true")],
        )?;
        prepared.push(PreparedTask {
            task_id,
            task,
            approvals: task_approvals,
            worktree: worktree.path().to_path_buf(),
            worktree_created,
            gates: task_gates,
        });
    }
    persist_execution(task_store, audit_store, &graph, &log)?;

    // Execute: agents and gates run in parallel; threads share only read-only inputs.
    let results: Vec<AgentResult> = thread::scope(|scope| {
        let handles = prepared
            .iter()
            .map(|entry| {
                let manager = &manager;
                scope.spawn(move || -> AgentResult {
                    let report = adapter
                        .execute(AdapterRequest {
                            task: &entry.task,
                            worktrees: manager,
                            acknowledged_approvals: &entry.approvals,
                        })
                        .map_err(|error| error.to_string())?;
                    let succeeded = report.termination() == ExecutionTermination::Exited
                        && report.exit_code() == Some(0);
                    let gate_runs = if succeeded {
                        run_gates(&entry.gates, report.worktree_path())
                    } else {
                        Vec::new()
                    };
                    Ok((report, gate_runs))
                })
            })
            .collect::<Vec<_>>();
        handles
            .into_iter()
            .map(|handle| {
                handle
                    .join()
                    .unwrap_or_else(|_| Err("agent thread panicked".to_owned()))
            })
            .collect()
    });

    // Finalize: sequential, in task-ID order, one persistence step.
    let mut log = audit_store.new_attempt_log();
    for (entry, result) in prepared.into_iter().zip(results) {
        match result {
            Ok((report, gate_runs)) => {
                let evidence = record_agent_finished(&root, &entry.task, &report, &mut log, true)?;
                record_gate_evidence(
                    &mut graph,
                    &entry.task_id,
                    &entry.task,
                    &gate_runs,
                    &mut log,
                )?;
                outcomes.push(BatchTaskOutcome::Launched {
                    task_id: entry.task_id,
                    report,
                    gates: gate_runs,
                    worktree: entry.worktree,
                    worktree_created: entry.worktree_created,
                    evidence: Box::new(evidence),
                });
            }
            Err(error) => {
                graph
                    .transition(&entry.task_id, TaskState::Failed)
                    .map_err(|transition| SliceError::Preflight(transition.to_string()))?;
                append_fields(
                    &mut log,
                    "agent-failed",
                    AuditEventKind::FailureClassified,
                    &entry.task,
                    &[("error", &audit_text(&error)), ("batch", "true")],
                )?;
                outcomes.push(BatchTaskOutcome::AdapterFailed {
                    task_id: entry.task_id,
                    error,
                });
            }
        }
    }
    persist_execution(task_store, audit_store, &graph, &log)?;
    outcomes.sort_by(|left, right| left.task_id().cmp(right.task_id()));
    Ok(BatchLaunch {
        base_commit,
        outcomes,
        deferred: batch.deferred,
    })
}

fn append_fields(
    log: &mut AuditLog,
    prefix: &str,
    kind: AuditEventKind,
    task: &AgentTask,
    fields: &[(&str, &str)],
) -> Result<(), SliceError> {
    let sequence = log.next_sequence();
    let mut event = AuditEvent::new(
        sequence,
        format!("{prefix}-{sequence}"),
        kind,
        "orchestrator",
        1,
    )
    .with_task_id(task.task_id.clone());
    for (key, value) in fields {
        event = event.with_field(*key, *value);
    }
    log.append(event)
        .map(|_| ())
        .map_err(|error| SliceError::Preflight(error.to_string()))
}

/// Reuses a clean owned worktree or creates one from `base_ref`. Unrelated worktrees are never
/// adopted.
fn prepare_worktree(
    manager: &WorktreeManager,
    task_id: &TaskId,
    base_ref: &str,
) -> Result<(WorktreeStatus, bool), SliceError> {
    match manager
        .inspect(task_id)
        .map_err(|error| SliceError::Preflight(error.to_string()))?
    {
        Some(status) => {
            if status.is_dirty() {
                return Err(SliceError::Preflight(format!(
                    "managed worktree is dirty: {task_id}"
                )));
            }
            if status.operation().is_some() {
                return Err(SliceError::Preflight(format!(
                    "managed worktree has an unresolved Git operation: {task_id}"
                )));
            }
            Ok((status, false))
        }
        None => {
            let status = manager
                .create(&WorktreeSpec::new(task_id.clone(), base_ref))
                .map_err(|error| SliceError::Preflight(error.to_string()))?;
            Ok((status, true))
        }
    }
}

/// Returns the current wall-clock time in milliseconds since the Unix epoch.
#[must_use]
pub fn wall_clock_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| {
            u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX)
        })
}

/// Returns the first path in `left` that overlaps a path in `right` (equal, or one contains the
/// other), using the scheduler's path-ownership rule.
#[must_use]
pub fn first_path_overlap(left: &[String], right: &[String]) -> Option<String> {
    let overlaps = |a: &str, b: &str| {
        let a = a.trim_matches('/');
        let b = b.trim_matches('/');
        a == b
            || a.strip_prefix(b).is_some_and(|rest| rest.starts_with('/'))
            || b.strip_prefix(a).is_some_and(|rest| rest.starts_with('/'))
    };
    left.iter()
        .find(|path| right.iter().any(|other| overlaps(path, other)))
        .cloned()
}

/// Returns why `task_id` may not run locally at `observed_at_ms` because of remote-worker leases:
/// it holds an active lease, or its owned paths overlap a task that does (P4-M004).
#[must_use]
pub fn remote_lease_conflict(
    graph: &TaskGraph,
    leases: &LeaseBook,
    task_id: &TaskId,
    observed_at_ms: u64,
) -> Option<String> {
    if let Some(lease) = leases.active_lease_for_task_at(task_id, observed_at_ms) {
        return Some(format!(
            "task {task_id} is leased to worker {} ({}, expires_at_ms={}); release the lease \
             before running it locally",
            lease.worker_id().as_str(),
            lease.lease_id().as_str(),
            lease.expires_at_ms()
        ));
    }
    let task = graph.get(task_id)?.task();
    for lease in leases.active_leases_at(observed_at_ms) {
        let Some(owner) = graph.get(lease.task_id()) else {
            continue;
        };
        if let Some(path) = first_path_overlap(&task.allowed_paths, &owner.task().allowed_paths) {
            return Some(format!(
                "task {task_id} path {path} overlaps task {} leased to worker {} ({})",
                lease.task_id(),
                lease.worker_id().as_str(),
                lease.lease_id().as_str()
            ));
        }
    }
    None
}

/// Refuses a local run of a leased task, or one overlapping a leased task, before side effects.
fn check_remote_leases(
    root: &Path,
    graph: &TaskGraph,
    task_id: &TaskId,
    claim: Option<&LeaseClaim>,
) -> Result<(), SliceError> {
    let leases = FileLeaseStore::for_project_root(root)
        .load()
        .map_err(|error| SliceError::Preflight(format!("remote lease state: {error}")))?;
    let now = wall_clock_ms();
    let Some(claim) = claim else {
        let Some(leases) = leases else {
            return Ok(());
        };
        return match remote_lease_conflict(graph, &leases, task_id, now) {
            Some(reason) => Err(SliceError::Preflight(reason)),
            None => Ok(()),
        };
    };
    let leases = leases.unwrap_or_default();
    let holds = leases
        .active_lease_for_task_at(task_id, now)
        .is_some_and(|lease| {
            lease.lease_id() == &claim.lease_id
                && lease.worker_id() == &claim.worker_id
                && lease.generation() == claim.generation
        });
    if !holds {
        return Err(SliceError::Preflight(format!(
            "lease claim {} (worker {}, generation {}) does not match an active lease for task \
             {task_id}",
            claim.lease_id.as_str(),
            claim.worker_id.as_str(),
            claim.generation
        )));
    }
    // The holder may run its own task, but never over another lease's paths.
    let task = graph
        .get(task_id)
        .ok_or_else(|| SliceError::Preflight("task not found".into()))?
        .task();
    for lease in leases.active_leases_at(now) {
        if lease.task_id() == task_id {
            continue;
        }
        if let Some(owner) = graph.get(lease.task_id()) {
            if let Some(path) = first_path_overlap(&task.allowed_paths, &owner.task().allowed_paths)
            {
                return Err(SliceError::Preflight(format!(
                    "task {task_id} path {path} overlaps task {} leased to worker {} ({})",
                    lease.task_id(),
                    lease.worker_id().as_str(),
                    lease.lease_id().as_str()
                )));
            }
        }
    }
    Ok(())
}

fn validate_launch_policy(
    task: &AgentTask,
    approvals: &[ApprovalBoundary],
) -> Result<(), SliceError> {
    let grants = approvals
        .iter()
        .copied()
        .map(|boundary| ApprovalGrant { boundary })
        .collect::<Vec<_>>();
    let engine = PolicyEngine;
    let request = PolicyRequest {
        capability: Capability::RunLocalCommands,
        paths: task.allowed_paths.clone(),
        approval: None,
    };
    if let PolicyDecision::Denied(error) = engine.evaluate(task, &request, &grants) {
        return Err(SliceError::Preflight(error.to_string()));
    }
    // Post-execution approvals (merge, release, deploy) act on the result and are recorded after
    // review, so they never gate running the agent (P1-M008).
    for boundary in task
        .required_approvals
        .iter()
        .filter(|boundary| !boundary.is_post_execution())
    {
        let request = PolicyRequest {
            capability: Capability::RunLocalCommands,
            paths: Vec::new(),
            approval: Some(*boundary),
        };
        if let PolicyDecision::Denied(error) = engine.evaluate(task, &request, &grants) {
            return Err(SliceError::Preflight(error.to_string()));
        }
    }
    Ok(())
}

fn append_worktree_observation(
    audit_store: &mut FileAuditStore,
    task: &AgentTask,
    worktree: &WorktreeStatus,
    base_commit: &str,
    created: bool,
) -> Result<(), SliceError> {
    let sequence = audit_store
        .records()
        .last()
        .map_or(1, |record| record.event().sequence().saturating_add(1));
    let event = AuditEvent::new(
        sequence,
        format!("worktree-observed-{sequence}"),
        AuditEventKind::WorktreeObserved,
        "orchestrator",
        1,
    )
    .with_task_id(task.task_id.clone())
    .with_field("base_commit", base_commit)
    .with_field("head", worktree.head())
    .with_field("path", worktree.path().to_string_lossy())
    .with_field("mode", if created { "created" } else { "reused" });
    audit_store
        .append(event)
        .map_err(|error| SliceError::Preflight(error.to_string()))?;
    Ok(())
}

/// Runs one task using file-backed task state and audit persistence.
pub fn execute_process_persisted<A: AgentAdapter>(
    root: impl AsRef<Path>,
    task_store: &FileTaskStore,
    audit_store: &mut FileAuditStore,
    task_id: &TaskId,
    adapter: &A,
    approvals: &[agentforge_core::agent::ApprovalBoundary],
) -> Result<ProcessExecution, SliceError> {
    execute_with_claim(
        root.as_ref(),
        task_store,
        audit_store,
        task_id,
        adapter,
        approvals,
        None,
    )
}

fn execute_with_claim<A: AgentAdapter>(
    root: &Path,
    task_store: &FileTaskStore,
    audit_store: &mut FileAuditStore,
    task_id: &TaskId,
    adapter: &A,
    approvals: &[agentforge_core::agent::ApprovalBoundary],
    claim: Option<&LeaseClaim>,
) -> Result<ProcessExecution, SliceError> {
    let mut graph = task_store
        .load()
        .map_err(|e| SliceError::Preflight(e.to_string()))?
        .ok_or_else(|| SliceError::Preflight("task state snapshot is missing".into()))?;
    check_remote_leases(root, &graph, task_id, claim)?;
    match execute_process_attempt(
        root,
        &mut graph,
        task_id,
        adapter,
        approvals,
        audit_store.new_attempt_log(),
    ) {
        Ok(execution) => {
            persist_execution(task_store, audit_store, &graph, &execution.audit)?;
            Ok(execution)
        }
        Err((error, audit)) => {
            if !audit.records().is_empty() {
                persist_execution(task_store, audit_store, &graph, &audit)?;
            }
            Err(error)
        }
    }
}

fn persist_execution(
    task_store: &FileTaskStore,
    audit_store: &mut FileAuditStore,
    graph: &TaskGraph,
    audit: &AuditLog,
) -> Result<(), SliceError> {
    task_store
        .save(graph)
        .map_err(|error| SliceError::Preflight(error.to_string()))?;
    // One batch under the audit append lock: another writer (the CLI, the daemon sweep) may have
    // appended while the agent ran, and the attempt log is renumbered after it (P0-M013).
    audit_store
        .append_batch(
            audit
                .records()
                .iter()
                .map(|record| record.event().clone())
                .collect(),
        )
        .map_err(|error| SliceError::Preflight(error.to_string()))
}

/// Runs a prepared task through the concrete process adapter and durable in-memory state/audit.
///
/// The task is transitioned to `Running` before launch. A successful process remains `Running`
/// until an independent caller records acceptance; adapter failure transitions it to `Failed`.
pub fn execute_process_once<A: AgentAdapter>(
    root: impl AsRef<Path>,
    graph: &mut TaskGraph,
    task_id: &TaskId,
    adapter: &A,
    approvals: &[agentforge_core::agent::ApprovalBoundary],
) -> Result<ProcessExecution, SliceError> {
    execute_process_attempt(root, graph, task_id, adapter, approvals, AuditLog::new())
        .map_err(|(error, _)| error)
}

fn execute_process_attempt<A: AgentAdapter>(
    root: impl AsRef<Path>,
    graph: &mut TaskGraph,
    task_id: &TaskId,
    adapter: &A,
    approvals: &[agentforge_core::agent::ApprovalBoundary],
    mut audit: AuditLog,
) -> Result<ProcessExecution, (SliceError, AuditLog)> {
    let root = root.as_ref().to_path_buf();
    let task = graph
        .records()
        .find(|record| record.id() == task_id)
        .ok_or_else(|| {
            (
                SliceError::Preflight("task not found".into()),
                audit.clone(),
            )
        })?
        .task()
        .clone();
    preflight_repository(&root, &task).map_err(|error| (error, audit.clone()))?;
    // Gate configuration is validated before any run side effect so a malformed profile can
    // never leave a task running without its required evidence.
    let gates = GateProfileStore::new(&root).list().map_err(|error| {
        (
            SliceError::Preflight(format!("gate configuration: {error}")),
            audit.clone(),
        )
    })?;
    let gates = select_gates(&gates, &task).map_err(|error| (error, audit.clone()))?;
    graph
        .transition(task_id, TaskState::Running)
        .map_err(|error| (SliceError::Preflight(error.to_string()), audit.clone()))?;
    append_event(
        &mut audit,
        "task-running",
        AuditEventKind::TaskTransition,
        &task,
        "state",
        "running",
    )
    .map_err(|error| (error, audit.clone()))?;
    append_event(
        &mut audit,
        "agent-started",
        AuditEventKind::AgentStarted,
        &task,
        "adapter",
        adapter.id(),
    )
    .map_err(|error| (error, audit.clone()))?;
    let manager = WorktreeManager::new(&root)
        .map_err(|error| (SliceError::Preflight(error.to_string()), audit.clone()))?;
    let result = adapter.execute(AdapterRequest {
        task: &task,
        worktrees: &manager,
        acknowledged_approvals: approvals,
    });
    match result {
        Ok(report) => {
            let evidence = record_agent_finished(&root, &task, &report, &mut audit, false)
                .map_err(|error| (error, audit.clone()))?;
            let gate_runs = if agent_exited_cleanly(&report) {
                run_gates(&gates, report.worktree_path())
            } else {
                Vec::new()
            };
            record_gate_evidence(graph, task_id, &task, &gate_runs, &mut audit)
                .map_err(|error| (error, audit.clone()))?;
            Ok(ProcessExecution {
                report,
                audit,
                gates: gate_runs,
                evidence,
            })
        }
        Err(error) => {
            let _ = graph.transition(task_id, TaskState::Failed);
            append_event(
                &mut audit,
                "agent-failed",
                AuditEventKind::FailureClassified,
                &task,
                "error",
                &error.to_string(),
            )
            .map_err(|append_error| (append_error, audit.clone()))?;
            Err((SliceError::Policy(error.to_string()), audit))
        }
    }
}

/// Persists the agent's captured output under `.forge/evidence/<task>/` and appends
/// `AgentFinished` with the exit status and log paths. Writing the logs is best-effort: a failure
/// is recorded in the event instead of aborting state persistence.
fn record_agent_finished(
    root: &Path,
    task: &AgentTask,
    report: &ExecutionReport,
    log: &mut AuditLog,
    batch: bool,
) -> Result<AgentEvidence, SliceError> {
    let sequence = log.next_sequence();
    // Audit records hold portable project-relative paths, so separators are always `/`, even on
    // Windows, which accepts them.
    let relative = PathBuf::from(format!("{EVIDENCE_RELATIVE_PATH}/{}", task.task_id));
    let stdout_log = PathBuf::from(format!(
        "{EVIDENCE_RELATIVE_PATH}/{}/{sequence}-stdout.log",
        task.task_id
    ));
    let stderr_log = PathBuf::from(format!(
        "{EVIDENCE_RELATIVE_PATH}/{}/{sequence}-stderr.log",
        task.task_id
    ));
    let written = std::fs::create_dir_all(root.join(&relative))
        .and_then(|()| std::fs::write(root.join(&stdout_log), report.stdout()))
        .and_then(|()| std::fs::write(root.join(&stderr_log), report.stderr()));
    let evidence = match written {
        Ok(()) => AgentEvidence {
            stdout_log: Some(stdout_log),
            stderr_log: Some(stderr_log),
            error: None,
        },
        Err(error) => AgentEvidence {
            error: Some(error.to_string()),
            ..AgentEvidence::default()
        },
    };
    let termination = match report.termination() {
        ExecutionTermination::Exited => "exited",
        ExecutionTermination::TimedOut => "timed_out",
        ExecutionTermination::OutputLimitExceeded => "output_limit_exceeded",
    };
    let exit_code = report
        .exit_code()
        .map_or_else(|| "none".to_owned(), |code| code.to_string());
    let mut fields = vec![
        ("termination", termination.to_owned()),
        ("exit_code", exit_code),
        (
            "output_truncated",
            if report.output_truncated() {
                "true"
            } else {
                "false"
            }
            .to_owned(),
        ),
    ];
    match (&evidence.stdout_log, &evidence.stderr_log, &evidence.error) {
        (Some(stdout), Some(stderr), _) => {
            fields.push(("stdout_log", stdout.to_string_lossy().into_owned()));
            fields.push(("stderr_log", stderr.to_string_lossy().into_owned()));
        }
        (_, _, Some(error)) => fields.push(("evidence_error", audit_text(error))),
        _ => {}
    }
    if batch {
        fields.push(("batch", "true".to_owned()));
    }
    let borrowed = fields
        .iter()
        .map(|(key, value)| (*key, value.as_str()))
        .collect::<Vec<_>>();
    append_fields(
        log,
        "agent-finished",
        AuditEventKind::AgentFinished,
        task,
        &borrowed,
    )?;
    Ok(evidence)
}

/// Selects the project gates a task must pass.
///
/// A task that declares no required gates runs every project gate. Otherwise exactly the declared
/// gates run, in lexical gate-ID order and each once. The first declared gate (in lexical order)
/// without a configured profile is a preflight error.
fn select_gates(
    gates: &[GateDefinition],
    task: &AgentTask,
) -> Result<Vec<GateDefinition>, SliceError> {
    if task.required_gates.is_empty() {
        return Ok(gates.to_vec());
    }
    let required = task
        .required_gates
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    if let Some(missing) = required
        .iter()
        .find(|name| !gates.iter().any(|gate| gate.name() == **name))
    {
        return Err(SliceError::Preflight(format!(
            "required gate is not configured: {missing}"
        )));
    }
    Ok(gates
        .iter()
        .filter(|gate| required.contains(gate.name()))
        .cloned()
        .collect())
}

fn run_gates(gates: &[GateDefinition], worktree: &Path) -> Vec<GateRun> {
    let runner = GateRunner;
    gates
        .iter()
        .map(|gate| match runner.run(gate, worktree) {
            Ok(report) => GateRun::Reported(report),
            Err(error) => GateRun::Errored {
                name: gate.name().to_owned(),
                error: error.to_string(),
            },
        })
        .collect()
}

/// Appends one `GateFinished` event per gate and fails the task when any gate did not pass.
fn record_gate_evidence(
    graph: &mut TaskGraph,
    task_id: &TaskId,
    task: &AgentTask,
    gates: &[GateRun],
    audit: &mut AuditLog,
) -> Result<(), SliceError> {
    for gate in gates {
        let sequence = audit.next_sequence();
        let mut event = AuditEvent::new(
            sequence,
            format!("gate-finished-{sequence}"),
            AuditEventKind::GateFinished,
            "orchestrator",
            1,
        )
        .with_task_id(task.task_id.clone())
        .with_field("gate", gate.name())
        .with_field("outcome", gate.outcome_label())
        .with_field(
            "exit_code",
            gate.exit_code()
                .map_or_else(|| "none".to_owned(), |code| code.to_string()),
        );
        event = match gate {
            GateRun::Reported(report) => event.with_field(
                "output_truncated",
                if report.output_truncated() {
                    "true"
                } else {
                    "false"
                },
            ),
            GateRun::Errored { error, .. } => event.with_field("error", audit_text(error)),
        };
        audit
            .append(event)
            .map_err(|error| SliceError::Preflight(error.to_string()))?;
    }
    if let Some(failed) = gates.iter().find(|gate| !gate.passed()) {
        graph
            .transition(task_id, TaskState::Failed)
            .map_err(|error| SliceError::Preflight(error.to_string()))?;
        let sequence = audit.next_sequence();
        let event = AuditEvent::new(
            sequence,
            format!("gate-failed-{sequence}"),
            AuditEventKind::FailureClassified,
            "orchestrator",
            1,
        )
        .with_task_id(task.task_id.clone())
        .with_field("stage", SliceStage::Gates.as_str())
        .with_field("gate", failed.name())
        .with_field("outcome", failed.outcome_label());
        audit
            .append(event)
            .map_err(|error| SliceError::Preflight(error.to_string()))?;
    }
    Ok(())
}

/// Audit fields must be non-empty, bounded, and free of control characters.
fn audit_text(value: &str) -> String {
    let text: String = value
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .take(512)
        .collect();
    if text.trim().is_empty() {
        "unspecified".to_owned()
    } else {
        text
    }
}

fn append_event(
    log: &mut AuditLog,
    id: &str,
    kind: AuditEventKind,
    task: &AgentTask,
    key: &str,
    value: &str,
) -> Result<(), SliceError> {
    let event = AuditEvent::new(log.next_sequence(), id, kind, "orchestrator", 1)
        .with_task_id(task.task_id.clone())
        .with_field(key, value);
    log.append(event)
        .map(|_| ())
        .map_err(|e| SliceError::Preflight(e.to_string()))
}

/// Runs the ordered, single-agent flow using caller-owned evidence and no ambient state.
pub fn run(task: &AgentTask, evidence: &SliceEvidence) -> Result<VerticalSliceReport, SliceError> {
    let request = PolicyRequest {
        capability: Capability::RunLocalCommands,
        paths: task.allowed_paths.clone(),
        approval: None,
    };
    if let PolicyDecision::Denied(error) = PolicyEngine.evaluate(task, &request, &[]) {
        return Err(SliceError::Policy(error.to_string()));
    }
    if !evidence.worktree_verified {
        return Err(SliceError::MissingEvidence(SliceStage::Worktree));
    }
    if !evidence.agent_reported {
        return Err(SliceError::MissingEvidence(SliceStage::Agent));
    }
    if !evidence.gates_passed {
        return Err(SliceError::MissingEvidence(SliceStage::Gates));
    }
    Ok(VerticalSliceReport {
        task_id: task.task_id.clone(),
        stages: vec![
            SliceStage::Policy,
            SliceStage::Worktree,
            SliceStage::Agent,
            SliceStage::Gates,
            SliceStage::ReviewHandoff,
        ],
        review_ready: true,
    })
}

/// Side-effect boundary supplied by an operator/runtime integration.
pub trait StageExecutor {
    /// Validate policy and approvals before any side effect.
    fn policy(&mut self, task: &AgentTask) -> Result<(), String>;
    /// Create and re-inspect the managed worktree.
    fn worktree(&mut self, task: &AgentTask) -> Result<(), String>;
    /// Invoke the configured bounded adapter.
    fn agent(&mut self, task: &AgentTask) -> Result<(), String>;
    /// Run declared quality gates.
    fn gates(&mut self, task: &AgentTask) -> Result<(), String>;
    /// Append durable audit evidence.
    fn audit(&mut self, task: &AgentTask, stage: SliceStage) -> Result<(), String>;
    /// Emit a review handoff without accepting the result.
    fn review_handoff(&mut self, task: &AgentTask) -> Result<(), String>;
}

/// Executes the ordered runtime boundary supplied by an integration.
pub fn execute_once<E: StageExecutor>(
    task: &AgentTask,
    executor: &mut E,
) -> Result<VerticalSliceReport, SliceError> {
    executor.policy(task).map_err(SliceError::Policy)?;
    executor
        .audit(task, SliceStage::Policy)
        .map_err(SliceError::Policy)?;
    executor.worktree(task).map_err(SliceError::Policy)?;
    executor
        .audit(task, SliceStage::Worktree)
        .map_err(SliceError::Policy)?;
    executor.agent(task).map_err(SliceError::Policy)?;
    executor
        .audit(task, SliceStage::Agent)
        .map_err(SliceError::Policy)?;
    executor.gates(task).map_err(SliceError::Policy)?;
    executor
        .audit(task, SliceStage::Gates)
        .map_err(SliceError::Policy)?;
    executor.review_handoff(task).map_err(SliceError::Policy)?;
    Ok(VerticalSliceReport {
        task_id: task.task_id.clone(),
        stages: vec![
            SliceStage::Policy,
            SliceStage::Worktree,
            SliceStage::Agent,
            SliceStage::Gates,
            SliceStage::ReviewHandoff,
        ],
        review_ready: true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use agentforge_core::agent::{AgentRole, Capability};
    fn task() -> AgentTask {
        let mut t = AgentTask::new("t", "m", AgentRole::Implementer, "work");
        t.capabilities = vec![Capability::RunLocalCommands];
        t.allowed_paths = vec!["src".into()];
        t
    }
    #[test]
    fn stages_are_ordered_and_review_ready() {
        let r = run(
            &task(),
            &SliceEvidence {
                worktree_verified: true,
                agent_reported: true,
                gates_passed: true,
            },
        )
        .unwrap();
        assert!(r.review_ready);
        assert_eq!(r.stages.len(), 5);
    }
    #[test]
    fn failures_stop_before_later_stages() {
        let e = run(
            &task(),
            &SliceEvidence {
                worktree_verified: true,
                agent_reported: false,
                gates_passed: true,
            },
        )
        .unwrap_err();
        assert_eq!(e, SliceError::MissingEvidence(SliceStage::Agent));
    }

    struct Fake {
        events: Vec<SliceStage>,
        fail_policy: bool,
    }
    impl StageExecutor for Fake {
        fn policy(&mut self, _: &AgentTask) -> Result<(), String> {
            if self.fail_policy {
                Err("denied".into())
            } else {
                Ok(())
            }
        }
        fn worktree(&mut self, _: &AgentTask) -> Result<(), String> {
            self.events.push(SliceStage::Worktree);
            Ok(())
        }
        fn agent(&mut self, _: &AgentTask) -> Result<(), String> {
            self.events.push(SliceStage::Agent);
            Ok(())
        }
        fn gates(&mut self, _: &AgentTask) -> Result<(), String> {
            self.events.push(SliceStage::Gates);
            Ok(())
        }
        fn audit(&mut self, _: &AgentTask, stage: SliceStage) -> Result<(), String> {
            self.events.push(stage);
            Ok(())
        }
        fn review_handoff(&mut self, _: &AgentTask) -> Result<(), String> {
            self.events.push(SliceStage::ReviewHandoff);
            Ok(())
        }
    }
    #[test]
    fn execute_once_orders_runtime_boundaries() {
        let mut fake = Fake {
            events: Vec::new(),
            fail_policy: false,
        };
        let report = execute_once(&task(), &mut fake).unwrap();
        assert!(report.review_ready);
        assert_eq!(fake.events.last(), Some(&SliceStage::ReviewHandoff));
    }
    #[test]
    fn execute_once_stops_before_side_effects_when_policy_denies() {
        let mut fake = Fake {
            events: Vec::new(),
            fail_policy: true,
        };
        assert!(execute_once(&task(), &mut fake).is_err());
        assert!(fake.events.is_empty());
    }

    #[test]
    fn repository_preflight_checks_real_git_root_and_policy() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        assert!(preflight_repository(root, &task()).is_ok());
    }
}
