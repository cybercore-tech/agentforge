//! A bounded, provider-neutral single-agent orchestration flow.

use agentforge_adapter::{AdapterRequest, AgentAdapter, ExecutionReport, ExecutionTermination};
use agentforge_audit::{AuditEvent, AuditEventKind, AuditLog};
use agentforge_audit::{AuditStore, FileAuditStore};
use agentforge_core::agent::{AgentTask, ApprovalBoundary, Capability};
use agentforge_core::task::{TaskGraph, TaskId, TaskState};
use agentforge_gate::{GateDefinition, GateOutcome, GateProfileStore, GateReport, GateRunner};
use agentforge_policy::{ApprovalGrant, PolicyDecision, PolicyEngine, PolicyRequest};
use agentforge_state::{FileTaskStore, TaskStore};
use agentforge_worktree::{WorktreeManager, WorktreeSpec, WorktreeStatus};
use std::fmt;
use std::path::Path;

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
}

impl ProcessExecution {
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
    let root = root.as_ref().to_path_buf();
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

    let manager =
        WorktreeManager::new(&root).map_err(|error| SliceError::Preflight(error.to_string()))?;
    let base_commit = manager
        .resolve_base(base_ref)
        .map_err(|error| SliceError::Preflight(error.to_string()))?;
    let (worktree, worktree_created) = match manager
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
            (status, false)
        }
        None => {
            let status = manager
                .create(&WorktreeSpec::new(task_id.clone(), base_ref))
                .map_err(|error| SliceError::Preflight(error.to_string()))?;
            (status, true)
        }
    };

    append_worktree_observation(
        audit_store,
        &task,
        &worktree,
        &base_commit,
        worktree_created,
    )?;
    let execution =
        execute_process_persisted(&root, task_store, audit_store, task_id, adapter, approvals)?;
    Ok(ForegroundLaunch {
        execution,
        base_commit,
        worktree,
        worktree_created,
    })
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
    for boundary in &task.required_approvals {
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
    let mut graph = task_store
        .load()
        .map_err(|e| SliceError::Preflight(e.to_string()))?
        .ok_or_else(|| SliceError::Preflight("task state snapshot is missing".into()))?;
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
    for record in audit.records() {
        audit_store
            .append(record.event().clone())
            .map_err(|error| SliceError::Preflight(error.to_string()))?;
    }
    Ok(())
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
            append_event(
                &mut audit,
                "agent-finished",
                AuditEventKind::AgentFinished,
                &task,
                "termination",
                "observed",
            )
            .map_err(|error| (error, audit.clone()))?;
            let agent_succeeded = report.termination() == ExecutionTermination::Exited
                && report.exit_code() == Some(0);
            let gate_runs = if agent_succeeded {
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
