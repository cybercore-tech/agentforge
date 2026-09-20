//! A bounded, provider-neutral single-agent orchestration flow.

use agentforge_adapter::{AdapterRequest, AgentAdapter, ExecutionReport};
use agentforge_audit::{AuditEvent, AuditEventKind, AuditLog};
use agentforge_core::agent::{AgentTask, Capability};
use agentforge_core::task::{TaskGraph, TaskId, TaskState};
use agentforge_policy::{PolicyDecision, PolicyEngine, PolicyRequest};
use agentforge_worktree::WorktreeManager;
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
    let root = root.as_ref().to_path_buf();
    let task = graph
        .records()
        .find(|record| record.id() == task_id)
        .ok_or_else(|| SliceError::Preflight("task not found".into()))?
        .task()
        .clone();
    preflight_repository(&root, &task)?;
    graph
        .transition(task_id, TaskState::Running)
        .map_err(|e| SliceError::Preflight(e.to_string()))?;
    let mut audit = AuditLog::new();
    append_event(
        &mut audit,
        1,
        "task-running",
        AuditEventKind::TaskTransition,
        &task,
        "state",
        "running",
    )?;
    append_event(
        &mut audit,
        2,
        "agent-started",
        AuditEventKind::AgentStarted,
        &task,
        "adapter",
        adapter.id(),
    )?;
    let manager = WorktreeManager::new(&root).map_err(|e| SliceError::Preflight(e.to_string()))?;
    let result = adapter.execute(AdapterRequest {
        task: &task,
        worktrees: &manager,
        acknowledged_approvals: approvals,
    });
    match result {
        Ok(report) => {
            append_event(
                &mut audit,
                3,
                "agent-finished",
                AuditEventKind::AgentFinished,
                &task,
                "termination",
                "observed",
            )?;
            Ok(ProcessExecution { report, audit })
        }
        Err(error) => {
            let _ = graph.transition(task_id, TaskState::Failed);
            append_event(
                &mut audit,
                3,
                "agent-failed",
                AuditEventKind::FailureClassified,
                &task,
                "error",
                &error.to_string(),
            )?;
            Err(SliceError::Policy(error.to_string()))
        }
    }
}

fn append_event(
    log: &mut AuditLog,
    sequence: u64,
    id: &str,
    kind: AuditEventKind,
    task: &AgentTask,
    key: &str,
    value: &str,
) -> Result<(), SliceError> {
    let event = AuditEvent::new(sequence, id, kind, "orchestrator", 1)
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
