//! Integration coverage for capability policy decisions.

use agentforge_core::agent::{AgentRole, AgentTask, ApprovalBoundary, Capability};
use agentforge_policy::{
    ApprovalGrant, PolicyDecision, PolicyEngine, PolicyRequest, PolicyViolation,
};

#[test]
fn role_does_not_change_authority() {
    let mut task = AgentTask::new("t", "m", AgentRole::Reviewer, "read");
    task.capabilities = vec![Capability::ReadRepository];
    let request = PolicyRequest {
        capability: Capability::WriteOwnedPaths,
        paths: vec![],
        approval: None,
    };
    assert!(matches!(
        PolicyEngine.evaluate(&task, &request, &[]),
        PolicyDecision::Denied(PolicyViolation::MissingCapability(_))
    ));
}

#[test]
fn approval_is_explicit_and_deterministic() {
    let mut task = AgentTask::new("t", "m", AgentRole::Implementer, "deploy");
    task.capabilities = vec![Capability::DeployProduction];
    let request = PolicyRequest {
        capability: Capability::DeployProduction,
        paths: vec![],
        approval: Some(ApprovalBoundary::DeployProduction),
    };
    assert!(matches!(
        PolicyEngine.evaluate(&task, &request, &[]),
        PolicyDecision::Denied(PolicyViolation::MissingApproval(_))
    ));
    assert_eq!(
        PolicyEngine.evaluate(
            &task,
            &request,
            &[ApprovalGrant {
                boundary: ApprovalBoundary::DeployProduction
            }]
        ),
        PolicyDecision::Allowed
    );
}
