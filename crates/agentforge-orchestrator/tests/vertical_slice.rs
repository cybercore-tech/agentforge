//! End-to-end ordering checks for the bounded vertical slice.
use agentforge_core::agent::{AgentRole, AgentTask, Capability};
use agentforge_orchestrator::{SliceError, SliceEvidence, SliceStage, run};

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
