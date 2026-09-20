//! Deterministic, provider-neutral capability policy evaluation.

use agentforge_core::agent::{
    AGENT_CONTRACT_VERSION, AgentRole, AgentTask, ApprovalBoundary, Capability,
};
use std::fmt;

/// A requested operation evaluated against one task's explicit authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PolicyRequest {
    /// Capability required by the operation.
    pub capability: Capability,
    /// Repository paths touched by the operation.
    pub paths: Vec<String>,
    /// Approval boundary required by the operation, if any.
    pub approval: Option<ApprovalBoundary>,
}

/// Human or system approval evidence supplied by the caller.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ApprovalGrant {
    /// Boundary that was approved.
    pub boundary: ApprovalBoundary,
}

/// Stable policy outcome.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PolicyDecision {
    /// The requested operation is within the supplied task authority.
    Allowed,
    /// The operation must stop and obtain additional authority.
    Denied(PolicyViolation),
}

/// Deterministic reason an operation was not allowed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PolicyViolation {
    /// Task contract is unsupported.
    UnsupportedContractVersion(u16),
    /// Capability was not explicitly granted.
    MissingCapability(Capability),
    /// A path is outside owned scope.
    PathOutsideScope(String),
    /// A path is explicitly forbidden.
    ForbiddenPath(String),
    /// Required human approval was not supplied.
    MissingApproval(ApprovalBoundary),
    /// Task has structurally invalid grants.
    InvalidTask(String),
}

impl fmt::Display for PolicyViolation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedContractVersion(v) => write!(f, "unsupported contract version: {v}"),
            Self::MissingCapability(c) => write!(f, "missing capability: {c}"),
            Self::PathOutsideScope(p) => write!(f, "path outside allowed scope: {p}"),
            Self::ForbiddenPath(p) => write!(f, "forbidden path: {p}"),
            Self::MissingApproval(a) => write!(f, "missing approval: {}", a.as_str()),
            Self::InvalidTask(e) => write!(f, "invalid task: {e}"),
        }
    }
}

impl std::error::Error for PolicyViolation {}

/// Pure policy evaluator. It performs no process, filesystem, network, or environment access.
#[derive(Clone, Copy, Debug, Default)]
pub struct PolicyEngine;

impl PolicyEngine {
    /// Evaluates one request using only task fields and explicit approval evidence.
    #[must_use]
    pub fn evaluate(
        &self,
        task: &AgentTask,
        request: &PolicyRequest,
        approvals: &[ApprovalGrant],
    ) -> PolicyDecision {
        if task.contract_version != AGENT_CONTRACT_VERSION {
            return PolicyDecision::Denied(PolicyViolation::UnsupportedContractVersion(
                task.contract_version,
            ));
        }
        if let Err(error) = task.validate() {
            return PolicyDecision::Denied(PolicyViolation::InvalidTask(error.to_string()));
        }
        if !task.has_capability(request.capability) {
            return PolicyDecision::Denied(PolicyViolation::MissingCapability(request.capability));
        }
        for path in &request.paths {
            if task.forbidden_paths.iter().any(|p| path_matches(p, path)) {
                return PolicyDecision::Denied(PolicyViolation::ForbiddenPath(path.clone()));
            }
            if !task.allowed_paths.is_empty()
                && !task.allowed_paths.iter().any(|p| path_matches(p, path))
            {
                return PolicyDecision::Denied(PolicyViolation::PathOutsideScope(path.clone()));
            }
        }
        if let Some(boundary) = request.approval {
            if !approvals.iter().any(|grant| grant.boundary == boundary) {
                return PolicyDecision::Denied(PolicyViolation::MissingApproval(boundary));
            }
        }
        PolicyDecision::Allowed
    }
}

fn path_matches(scope: &str, path: &str) -> bool {
    let scope = normalize(scope);
    let path = normalize(path);
    path == scope
        || path
            .strip_prefix(&scope)
            .is_some_and(|rest| rest.starts_with('/'))
}

fn normalize(path: &str) -> String {
    let mut out = String::new();
    for part in path.split('/') {
        if part.is_empty() || part == "." {
            continue;
        }
        if part == ".." {
            if let Some((prefix, _)) = out.rsplit_once('/') {
                out = prefix.to_owned();
            } else {
                out.clear();
            }
            continue;
        }
        if !out.is_empty() {
            out.push('/');
        }
        out.push_str(part);
    }
    out
}

/// Returns the role for callers that need to record responsibility alongside a decision.
#[must_use]
pub const fn role_name(role: AgentRole) -> &'static str {
    role.as_str()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn task() -> AgentTask {
        let mut t = AgentTask::new("t", "m", AgentRole::Implementer, "edit");
        t.capabilities = vec![Capability::WriteOwnedPaths];
        t.allowed_paths = vec!["src".into()];
        t.forbidden_paths = vec!["src/secrets".into()];
        t
    }
    #[test]
    fn allows_owned_path() {
        let t = task();
        assert_eq!(
            PolicyEngine.evaluate(
                &t,
                &PolicyRequest {
                    capability: Capability::WriteOwnedPaths,
                    paths: vec!["src/lib.rs".into()],
                    approval: None
                },
                &[]
            ),
            PolicyDecision::Allowed
        );
    }
    #[test]
    fn denies_missing_capability() {
        let t = task();
        assert!(matches!(
            PolicyEngine.evaluate(
                &t,
                &PolicyRequest {
                    capability: Capability::UseNetwork,
                    paths: vec![],
                    approval: None
                },
                &[]
            ),
            PolicyDecision::Denied(PolicyViolation::MissingCapability(_))
        ));
    }
    #[test]
    fn denies_forbidden_and_missing_approval() {
        let t = task();
        let e = PolicyEngine;
        assert!(matches!(
            e.evaluate(
                &t,
                &PolicyRequest {
                    capability: Capability::WriteOwnedPaths,
                    paths: vec!["src/secrets/x".into()],
                    approval: None
                },
                &[]
            ),
            PolicyDecision::Denied(PolicyViolation::ForbiddenPath(_))
        ));
    }
}
