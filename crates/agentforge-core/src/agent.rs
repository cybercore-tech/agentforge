//! Provider-neutral agent governance contracts.
//!
//! This module defines responsibility, authority, task input, and execution result semantics.
//! It intentionally does not define a wire format or provider-specific adapter behavior.

use std::collections::HashSet;
use std::fmt;

/// Current semantic contract version for [`AgentTask`] and [`AgentResult`].
pub const AGENT_CONTRACT_VERSION: u16 = 1;

/// Canonical responsibility assigned to one agent-task execution.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum AgentRole {
    /// Decomposes approved work into executable tasks.
    Planner,
    /// Defines architecture boundaries and interfaces.
    Architect,
    /// Gathers and summarizes external evidence.
    Researcher,
    /// Produces task-scoped implementation.
    Implementer,
    /// Reproduces behavior and creates regression evidence.
    Tester,
    /// Independently reviews implementation and evidence.
    Reviewer,
    /// Reviews security, privilege, secrets, and threat boundaries.
    SecurityReviewer,
    /// Combines approved task outputs and resolves integration-only conflicts.
    Integrator,
    /// Prepares authorized release artifacts and release operations.
    ReleaseManager,
}

impl AgentRole {
    /// Returns the stable machine-readable identity for this role.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Planner => "planner",
            Self::Architect => "architect",
            Self::Researcher => "researcher",
            Self::Implementer => "implementer",
            Self::Tester => "tester",
            Self::Reviewer => "reviewer",
            Self::SecurityReviewer => "security_reviewer",
            Self::Integrator => "integrator",
            Self::ReleaseManager => "release_manager",
        }
    }
}

impl fmt::Display for AgentRole {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Explicit authority that may be granted to an agent task.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Capability {
    /// Read repository content.
    ReadRepository,
    /// Write only paths owned by the task.
    WriteOwnedPaths,
    /// Execute local commands.
    RunLocalCommands,
    /// Access external network resources.
    UseNetwork,
    /// Read GitHub state.
    ReadGitHub,
    /// Write GitHub state.
    WriteGitHub,
    /// Create, inspect, or retire task worktrees.
    ManageWorktrees,
    /// Read secrets explicitly granted to the task.
    ReadSecrets,
    /// Use MCP-exposed tools.
    UseMcpTools,
    /// Create pull requests.
    CreatePullRequest,
    /// Merge into a protected branch.
    MergeProtectedBranch,
    /// Deploy to staging.
    DeployStaging,
    /// Deploy to production.
    DeployProduction,
}

impl Capability {
    /// Returns the stable machine-readable identity for this capability.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ReadRepository => "read_repository",
            Self::WriteOwnedPaths => "write_owned_paths",
            Self::RunLocalCommands => "run_local_commands",
            Self::UseNetwork => "use_network",
            Self::ReadGitHub => "read_github",
            Self::WriteGitHub => "write_github",
            Self::ManageWorktrees => "manage_worktrees",
            Self::ReadSecrets => "read_secrets",
            Self::UseMcpTools => "use_mcp_tools",
            Self::CreatePullRequest => "create_pull_request",
            Self::MergeProtectedBranch => "merge_protected_branch",
            Self::DeployStaging => "deploy_staging",
            Self::DeployProduction => "deploy_production",
        }
    }
}

impl fmt::Display for Capability {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Human-controlled boundary that an agent task may be required to cross.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ApprovalBoundary {
    /// Activate implementation from an approved plan.
    ActivateImplementationPlan,
    /// Expand work beyond approved scope or ownership.
    ExpandTaskScope,
    /// Add or materially change a third-party dependency.
    ChangeDependencies,
    /// Elevate the task's capabilities or privileges.
    ElevateCapability,
    /// Read or use a secret.
    AccessSecrets,
    /// Perform a destructive data migration.
    DestructiveDataMigration,
    /// Perform an irreversible external state change.
    IrreversibleExternalChange,
    /// Merge into a protected branch.
    MergeProtectedBranch,
    /// Publish a public release.
    PublishRelease,
    /// Deploy to production.
    DeployProduction,
    /// Change governance rules that define approval boundaries.
    ChangeGovernanceRules,
}

impl ApprovalBoundary {
    /// Returns the stable machine-readable identity for this approval boundary.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ActivateImplementationPlan => "activate_implementation_plan",
            Self::ExpandTaskScope => "expand_task_scope",
            Self::ChangeDependencies => "change_dependencies",
            Self::ElevateCapability => "elevate_capability",
            Self::AccessSecrets => "access_secrets",
            Self::DestructiveDataMigration => "destructive_data_migration",
            Self::IrreversibleExternalChange => "irreversible_external_change",
            Self::MergeProtectedBranch => "merge_protected_branch",
            Self::PublishRelease => "publish_release",
            Self::DeployProduction => "deploy_production",
            Self::ChangeGovernanceRules => "change_governance_rules",
        }
    }
}

/// Outcome reported by one agent-task execution.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum TaskOutcome {
    /// The assigned task completed within its granted authority.
    Completed,
    /// Progress stopped because a dependency or external condition blocked the task.
    Blocked,
    /// Progress requires authority or human approval not currently granted.
    EscalationRequired,
    /// Execution failed within the current task scope.
    Failed,
}

impl TaskOutcome {
    /// Returns the stable machine-readable identity for this outcome.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Completed => "completed",
            Self::Blocked => "blocked",
            Self::EscalationRequired => "escalation_required",
            Self::Failed => "failed",
        }
    }
}

/// Structured input contract for one agent execution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgentTask {
    /// Semantic contract version.
    pub contract_version: u16,
    /// Stable task identifier.
    pub task_id: String,
    /// Stable milestone identifier that owns this task.
    pub milestone_id: String,
    /// Primary responsibility assigned to this execution.
    pub primary_role: AgentRole,
    /// Positive task objective.
    pub goal: String,
    /// Explicit exclusions from task scope.
    pub non_goals: Vec<String>,
    /// Task identifiers that must be accepted before this task can proceed.
    pub dependency_task_ids: Vec<String>,
    /// Repository paths the task may modify.
    pub allowed_paths: Vec<String>,
    /// Repository paths the task must not modify.
    pub forbidden_paths: Vec<String>,
    /// Explicit capabilities granted to the task.
    pub capabilities: Vec<Capability>,
    /// Human approval boundaries relevant to this task.
    pub required_approvals: Vec<ApprovalBoundary>,
    /// Quality gates required before the result can be accepted.
    pub required_gates: Vec<String>,
    /// Outputs the task is expected to produce.
    pub expected_outputs: Vec<String>,
    /// Evidence the task is expected to report.
    pub evidence_requirements: Vec<String>,
}

impl AgentTask {
    /// Creates a task with the current contract version and empty optional collections.
    #[must_use]
    pub fn new(
        task_id: impl Into<String>,
        milestone_id: impl Into<String>,
        primary_role: AgentRole,
        goal: impl Into<String>,
    ) -> Self {
        Self {
            contract_version: AGENT_CONTRACT_VERSION,
            task_id: task_id.into(),
            milestone_id: milestone_id.into(),
            primary_role,
            goal: goal.into(),
            non_goals: Vec::new(),
            dependency_task_ids: Vec::new(),
            allowed_paths: Vec::new(),
            forbidden_paths: Vec::new(),
            capabilities: Vec::new(),
            required_approvals: Vec::new(),
            required_gates: Vec::new(),
            expected_outputs: Vec::new(),
            evidence_requirements: Vec::new(),
        }
    }

    /// Validates task invariants that do not require repository state.
    ///
    /// # Errors
    ///
    /// Returns [`TaskContractError`] when identifiers, goal, scope, or capability grants are
    /// structurally invalid.
    pub fn validate(&self) -> Result<(), TaskContractError> {
        if self.contract_version != AGENT_CONTRACT_VERSION {
            return Err(TaskContractError::UnsupportedContractVersion {
                found: self.contract_version,
            });
        }

        if self.task_id.trim().is_empty() {
            return Err(TaskContractError::EmptyTaskId);
        }

        if self.milestone_id.trim().is_empty() {
            return Err(TaskContractError::EmptyMilestoneId);
        }

        if self.goal.trim().is_empty() {
            return Err(TaskContractError::EmptyGoal);
        }

        let allowed: HashSet<&str> = self.allowed_paths.iter().map(String::as_str).collect();
        let forbidden: HashSet<&str> = self.forbidden_paths.iter().map(String::as_str).collect();

        if let Some(path) = allowed.intersection(&forbidden).next() {
            return Err(TaskContractError::ConflictingPathOwnership {
                path: (*path).to_owned(),
            });
        }

        let mut seen_capabilities = HashSet::new();
        for capability in &self.capabilities {
            if !seen_capabilities.insert(*capability) {
                return Err(TaskContractError::DuplicateCapability {
                    capability: *capability,
                });
            }
        }

        Ok(())
    }

    /// Returns whether this task explicitly has the requested capability.
    #[must_use]
    pub fn has_capability(&self, capability: Capability) -> bool {
        self.capabilities.contains(&capability)
    }
}

/// Structural validation failure for an [`AgentTask`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TaskContractError {
    /// The task uses an unsupported semantic contract version.
    UnsupportedContractVersion {
        /// Version found on the task.
        found: u16,
    },
    /// The task identifier is empty.
    EmptyTaskId,
    /// The milestone identifier is empty.
    EmptyMilestoneId,
    /// The task goal is empty.
    EmptyGoal,
    /// One path appears in both allowed and forbidden ownership.
    ConflictingPathOwnership {
        /// Conflicting path.
        path: String,
    },
    /// One capability was granted more than once.
    DuplicateCapability {
        /// Duplicated capability.
        capability: Capability,
    },
}

impl fmt::Display for TaskContractError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedContractVersion { found } => {
                write!(
                    formatter,
                    "unsupported agent contract version {found}; expected {AGENT_CONTRACT_VERSION}"
                )
            }
            Self::EmptyTaskId => formatter.write_str("task ID must not be empty"),
            Self::EmptyMilestoneId => formatter.write_str("milestone ID must not be empty"),
            Self::EmptyGoal => formatter.write_str("task goal must not be empty"),
            Self::ConflictingPathOwnership { path } => {
                write!(formatter, "path is both allowed and forbidden: {path}")
            }
            Self::DuplicateCapability { capability } => {
                write!(formatter, "capability granted more than once: {capability}")
            }
        }
    }
}

impl std::error::Error for TaskContractError {}

/// Structured output contract for one agent execution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgentResult {
    /// Semantic contract version.
    pub contract_version: u16,
    /// Identifier of the originating task.
    pub task_id: String,
    /// Execution outcome.
    pub outcome: TaskOutcome,
    /// Human-readable summary.
    pub summary: String,
    /// Repository paths changed by the execution.
    pub changed_paths: Vec<String>,
    /// Commit identity when the execution produced a commit.
    pub commit: Option<String>,
    /// Quality gates executed by the worker.
    pub gates_run: Vec<String>,
    /// Tests added or changed by the worker.
    pub tests_changed: Vec<String>,
    /// Evidence produced by the worker.
    pub evidence: Vec<String>,
    /// Risks that remain unresolved.
    pub unresolved_risks: Vec<String>,
    /// Requested escalation when more authority or approval is required.
    pub requested_escalation: Option<String>,
    /// Notes for the next role or operator.
    pub handoff_notes: Vec<String>,
}

impl AgentResult {
    /// Creates a result tied to one originating task.
    #[must_use]
    pub fn new(
        task_id: impl Into<String>,
        outcome: TaskOutcome,
        summary: impl Into<String>,
    ) -> Self {
        Self {
            contract_version: AGENT_CONTRACT_VERSION,
            task_id: task_id.into(),
            outcome,
            summary: summary.into(),
            changed_paths: Vec::new(),
            commit: None,
            gates_run: Vec::new(),
            tests_changed: Vec::new(),
            evidence: Vec::new(),
            unresolved_risks: Vec::new(),
            requested_escalation: None,
            handoff_notes: Vec::new(),
        }
    }

    /// Returns whether this result belongs to the provided task.
    #[must_use]
    pub fn belongs_to(&self, task: &AgentTask) -> bool {
        self.contract_version == task.contract_version && self.task_id == task.task_id
    }
}

#[cfg(test)]
mod tests {
    use super::{
        AGENT_CONTRACT_VERSION, AgentResult, AgentRole, AgentTask, ApprovalBoundary, Capability,
        TaskContractError, TaskOutcome,
    };

    #[test]
    fn canonical_roles_have_stable_identities() {
        let roles = [
            (AgentRole::Planner, "planner"),
            (AgentRole::Architect, "architect"),
            (AgentRole::Researcher, "researcher"),
            (AgentRole::Implementer, "implementer"),
            (AgentRole::Tester, "tester"),
            (AgentRole::Reviewer, "reviewer"),
            (AgentRole::SecurityReviewer, "security_reviewer"),
            (AgentRole::Integrator, "integrator"),
            (AgentRole::ReleaseManager, "release_manager"),
        ];

        for (role, expected) in roles {
            assert_eq!(role.as_str(), expected);
            assert_eq!(role.to_string(), expected);
        }
    }

    #[test]
    fn canonical_capabilities_have_stable_identities() {
        let capabilities = [
            (Capability::ReadRepository, "read_repository"),
            (Capability::WriteOwnedPaths, "write_owned_paths"),
            (Capability::RunLocalCommands, "run_local_commands"),
            (Capability::UseNetwork, "use_network"),
            (Capability::ReadGitHub, "read_github"),
            (Capability::WriteGitHub, "write_github"),
            (Capability::ManageWorktrees, "manage_worktrees"),
            (Capability::ReadSecrets, "read_secrets"),
            (Capability::UseMcpTools, "use_mcp_tools"),
            (Capability::CreatePullRequest, "create_pull_request"),
            (Capability::MergeProtectedBranch, "merge_protected_branch"),
            (Capability::DeployStaging, "deploy_staging"),
            (Capability::DeployProduction, "deploy_production"),
        ];

        for (capability, expected) in capabilities {
            assert_eq!(capability.as_str(), expected);
            assert_eq!(capability.to_string(), expected);
        }
    }

    #[test]
    fn task_constructor_uses_current_contract_version() {
        let task = AgentTask::new(
            "P0-M002-I001",
            "P0-M002",
            AgentRole::Implementer,
            "Implement provider-neutral task contracts.",
        );

        assert_eq!(task.contract_version, AGENT_CONTRACT_VERSION);
        assert_eq!(task.primary_role, AgentRole::Implementer);
        assert!(task.validate().is_ok());
    }

    #[test]
    fn task_capabilities_are_explicit() {
        let mut task = AgentTask::new(
            "P0-M002-I002",
            "P0-M002",
            AgentRole::Implementer,
            "Test explicit capability grants.",
        );

        assert!(!task.has_capability(Capability::WriteOwnedPaths));

        task.capabilities.push(Capability::ReadRepository);

        assert!(task.has_capability(Capability::ReadRepository));
        assert!(!task.has_capability(Capability::WriteOwnedPaths));
    }

    #[test]
    fn task_rejects_overlapping_allowed_and_forbidden_paths() {
        let mut task = AgentTask::new(
            "P0-M002-I003",
            "P0-M002",
            AgentRole::Implementer,
            "Test ownership validation.",
        );

        task.allowed_paths.push("src/**".to_owned());
        task.forbidden_paths.push("src/**".to_owned());

        assert_eq!(
            task.validate(),
            Err(TaskContractError::ConflictingPathOwnership {
                path: "src/**".to_owned(),
            })
        );
    }

    #[test]
    fn task_rejects_duplicate_capability_grants() {
        let mut task = AgentTask::new(
            "P0-M002-I004",
            "P0-M002",
            AgentRole::Reviewer,
            "Test capability validation.",
        );

        task.capabilities = vec![Capability::ReadRepository, Capability::ReadRepository];

        assert_eq!(
            task.validate(),
            Err(TaskContractError::DuplicateCapability {
                capability: Capability::ReadRepository,
            })
        );
    }

    #[test]
    fn result_is_bound_to_originating_task() {
        let task = AgentTask::new(
            "P0-M002-I005",
            "P0-M002",
            AgentRole::Tester,
            "Test task/result identity.",
        );

        let result = AgentResult::new(
            "P0-M002-I005",
            TaskOutcome::Completed,
            "Regression tests completed.",
        );

        assert!(result.belongs_to(&task));
    }

    #[test]
    fn result_does_not_carry_authority_grants() {
        let result = AgentResult::new(
            "P0-M002-I006",
            TaskOutcome::EscalationRequired,
            "Production deployment approval is required.",
        );

        assert_eq!(result.contract_version, AGENT_CONTRACT_VERSION);
        assert_eq!(result.outcome, TaskOutcome::EscalationRequired);
        assert!(result.requested_escalation.is_none());
    }

    #[test]
    fn approval_boundaries_have_stable_identities() {
        assert_eq!(
            ApprovalBoundary::ActivateImplementationPlan.as_str(),
            "activate_implementation_plan"
        );
        assert_eq!(
            ApprovalBoundary::MergeProtectedBranch.as_str(),
            "merge_protected_branch"
        );
        assert_eq!(
            ApprovalBoundary::DeployProduction.as_str(),
            "deploy_production"
        );
        assert_eq!(
            ApprovalBoundary::ChangeGovernanceRules.as_str(),
            "change_governance_rules"
        );
    }
}
