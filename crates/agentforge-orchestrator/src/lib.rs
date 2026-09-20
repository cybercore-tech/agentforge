//! A bounded, provider-neutral single-agent orchestration flow.

use agentforge_core::agent::{AgentTask, Capability};
use agentforge_policy::{PolicyDecision, PolicyEngine, PolicyRequest};
use std::fmt;

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
}

impl fmt::Display for SliceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Policy(reason) => write!(f, "policy denied vertical slice: {reason}"),
            Self::MissingEvidence(stage) => {
                write!(f, "missing evidence at {} stage", stage.as_str())
            }
        }
    }
}
impl std::error::Error for SliceError {}

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
}
