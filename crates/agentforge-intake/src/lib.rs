//! Project-local blueprint and guideline intake for AgentForge.
//!
//! Intake documents are deliberately small, versioned, and human-editable. They describe project
//! context and explicit defaults; they never grant authority through prose. Task contracts remain
//! the executable authority boundary owned by `agentforge-core`.

use agentforge_core::agent::{AgentRole, AgentTask, ApprovalBoundary, Capability};
use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

/// Current supported project-blueprint document version.
pub const BLUEPRINT_VERSION: u16 = 1;
/// Current supported guideline document version.
pub const GUIDELINES_VERSION: u16 = 1;
/// Project-relative blueprint path.
pub const BLUEPRINT_RELATIVE_PATH: &str = ".forge/blueprint.conf";
/// Project-relative guideline path.
pub const GUIDELINES_RELATIVE_PATH: &str = ".forge/guidelines.md";
const MAX_BLUEPRINT_BYTES: usize = 64 * 1024;
const MAX_GUIDELINE_BYTES: usize = 256 * 1024;
const MAX_FIELD_BYTES: usize = 4096;
const MAX_LIST_ITEMS: usize = 128;

/// A validated project blueprint with explicit structured defaults.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectBlueprint {
    /// Blueprint schema version.
    pub version: u16,
    /// Human-readable project name.
    pub name: String,
    /// Project mission statement.
    pub mission: String,
    /// Default milestone for newly created tasks, when supplied.
    pub default_milestone: Option<String>,
    /// Default owned paths for newly created tasks.
    pub allowed_paths: Vec<String>,
    /// Default forbidden paths for newly created tasks.
    pub forbidden_paths: Vec<String>,
    /// Explicit default capability grants.
    pub capabilities: Vec<Capability>,
    /// Explicit default approval boundaries.
    pub approvals: Vec<ApprovalBoundary>,
    /// Default quality gates.
    pub gates: Vec<String>,
}

/// A validated guideline document.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GuidelineDocument {
    /// Guideline schema version.
    pub version: u16,
    /// Markdown guideline body after the version marker.
    pub body: String,
}

/// The validated project-local intake bundle.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IntakeBundle {
    /// Structured project blueprint.
    pub blueprint: ProjectBlueprint,
    /// Human-readable guidelines.
    pub guidelines: GuidelineDocument,
}

/// Explicit task fields supplied by an operator.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TaskDraft {
    /// Stable task identifier.
    pub task_id: String,
    /// Milestone identifier owning the task.
    pub milestone_id: Option<String>,
    /// Primary task role.
    pub primary_role: AgentRole,
    /// Positive task goal.
    pub goal: String,
    /// Explicit task exclusions.
    pub non_goals: Vec<String>,
    /// Task dependencies.
    pub dependency_task_ids: Vec<String>,
    /// Task-owned paths; empty uses the blueprint default.
    pub allowed_paths: Vec<String>,
    /// Task-forbidden paths; empty uses the blueprint default.
    pub forbidden_paths: Vec<String>,
    /// Capability grants; empty uses the blueprint default.
    pub capabilities: Vec<Capability>,
    /// Required approvals; empty uses the blueprint default.
    pub required_approvals: Vec<ApprovalBoundary>,
    /// Required gates; empty uses the blueprint default.
    pub required_gates: Vec<String>,
    /// Expected task outputs.
    pub expected_outputs: Vec<String>,
    /// Required evidence.
    pub evidence_requirements: Vec<String>,
}

impl TaskDraft {
    /// Creates a minimal draft with explicit identity, role, and goal.
    #[must_use]
    pub fn new(
        task_id: impl Into<String>,
        primary_role: AgentRole,
        goal: impl Into<String>,
    ) -> Self {
        Self {
            task_id: task_id.into(),
            milestone_id: None,
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
}

/// A source-located validation diagnostic.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidationError {
    /// Source file label.
    pub file: String,
    /// One-based source line, or zero for document-level errors.
    pub line: usize,
    /// Stable human-readable diagnostic.
    pub message: String,
}

impl fmt::Display for ValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.line == 0 {
            write!(formatter, "{}: {}", self.file, self.message)
        } else {
            write!(formatter, "{}:{}: {}", self.file, self.line, self.message)
        }
    }
}

/// Intake operation failure.
#[derive(Debug)]
pub enum IntakeError {
    /// Filesystem failure.
    Io(std::io::Error),
    /// One or more deterministic validation diagnostics.
    Validation(Vec<ValidationError>),
    /// A task contract could not be built from explicit input.
    Task(String),
    /// An initialization target already exists.
    AlreadyExists(PathBuf),
}

impl fmt::Display for IntakeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "intake I/O error: {error}"),
            Self::Validation(errors) => {
                write!(
                    formatter,
                    "intake validation failed ({} error(s))",
                    errors.len()
                )
            }
            Self::Task(error) => write!(formatter, "task creation failed: {error}"),
            Self::AlreadyExists(path) => {
                write!(
                    formatter,
                    "refusing to overwrite existing file: {}",
                    path.display()
                )
            }
        }
    }
}

impl std::error::Error for IntakeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Validation(_) | Self::Task(_) | Self::AlreadyExists(_) => None,
        }
    }
}

impl From<std::io::Error> for IntakeError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

/// Returns the project-local blueprint and guideline paths.
#[must_use]
pub fn project_paths(root: impl AsRef<Path>) -> (PathBuf, PathBuf) {
    (
        root.as_ref().join(BLUEPRINT_RELATIVE_PATH),
        root.as_ref().join(GUIDELINES_RELATIVE_PATH),
    )
}

/// Initializes starter intake files without overwriting existing files.
pub fn initialize(root: impl AsRef<Path>) -> Result<(PathBuf, PathBuf), IntakeError> {
    let (blueprint_path, guidelines_path) = project_paths(root);
    if blueprint_path.exists() {
        return Err(IntakeError::AlreadyExists(blueprint_path));
    }
    if guidelines_path.exists() {
        return Err(IntakeError::AlreadyExists(guidelines_path));
    }

    if let Some(parent) = blueprint_path.parent() {
        fs::create_dir_all(parent)?;
    }
    create_new_file(&blueprint_path, starter_blueprint().as_bytes())?;
    if let Err(error) = create_new_file(&guidelines_path, starter_guidelines().as_bytes()) {
        let _ = fs::remove_file(&blueprint_path);
        return Err(error);
    }
    Ok((blueprint_path, guidelines_path))
}

/// Loads and validates both project-local intake documents.
pub fn load(root: impl AsRef<Path>) -> Result<IntakeBundle, IntakeError> {
    let (blueprint_path, guidelines_path) = project_paths(root);
    let blueprint_bytes = fs::read(&blueprint_path)?;
    let guideline_bytes = fs::read(&guidelines_path)?;
    let blueprint = parse_blueprint(&blueprint_path.display().to_string(), &blueprint_bytes)?;
    let guidelines = parse_guidelines(&guidelines_path.display().to_string(), &guideline_bytes)?;
    Ok(IntakeBundle {
        blueprint,
        guidelines,
    })
}

/// Builds a validated task contract from a draft and structured blueprint defaults.
pub fn build_task(
    draft: &TaskDraft,
    blueprint: &ProjectBlueprint,
) -> Result<AgentTask, IntakeError> {
    let milestone_id = draft
        .milestone_id
        .clone()
        .or_else(|| blueprint.default_milestone.clone())
        .ok_or_else(|| {
            IntakeError::Task(
                "milestone is required (use --milestone or set default_milestone)".to_owned(),
            )
        })?;
    let mut task = AgentTask::new(
        draft.task_id.clone(),
        milestone_id,
        draft.primary_role,
        draft.goal.clone(),
    );
    task.non_goals = draft.non_goals.clone();
    task.dependency_task_ids = draft.dependency_task_ids.clone();
    task.allowed_paths = choose(&draft.allowed_paths, &blueprint.allowed_paths);
    task.forbidden_paths = choose(&draft.forbidden_paths, &blueprint.forbidden_paths);
    task.capabilities = choose(&draft.capabilities, &blueprint.capabilities);
    task.required_approvals = choose(&draft.required_approvals, &blueprint.approvals);
    task.required_gates = choose(&draft.required_gates, &blueprint.gates);
    task.expected_outputs = draft.expected_outputs.clone();
    task.evidence_requirements = draft.evidence_requirements.clone();
    task.validate()
        .map_err(|error| IntakeError::Task(error.to_string()))?;
    Ok(task)
}

fn choose<T: Clone>(specific: &[T], defaults: &[T]) -> Vec<T> {
    if specific.is_empty() {
        defaults.to_vec()
    } else {
        specific.to_vec()
    }
}

fn create_new_file(path: &Path, bytes: &[u8]) -> Result<(), IntakeError> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| {
            if error.kind() == std::io::ErrorKind::AlreadyExists {
                IntakeError::AlreadyExists(path.to_owned())
            } else {
                IntakeError::Io(error)
            }
        })?;
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(())
}

fn parse_blueprint(file: &str, bytes: &[u8]) -> Result<ProjectBlueprint, IntakeError> {
    if bytes.len() > MAX_BLUEPRINT_BYTES {
        return Err(validation(file, 0, "blueprint exceeds 64 KiB"));
    }
    let text = std::str::from_utf8(bytes)
        .map_err(|_| validation(file, 0, "blueprint must be valid UTF-8"))?;
    let mut version = None;
    let mut name = None;
    let mut mission = None;
    let mut default_milestone = None;
    let mut allowed_paths = Vec::new();
    let mut forbidden_paths = Vec::new();
    let mut capabilities: Vec<Capability> = Vec::new();
    let mut approvals: Vec<ApprovalBoundary> = Vec::new();
    let mut gates = Vec::new();
    let mut errors = Vec::new();

    for (index, raw) in text.lines().enumerate() {
        let line = index + 1;
        let value = raw.trim();
        if value.is_empty() || value.starts_with('#') {
            continue;
        }
        let Some((key, raw_value)) = value.split_once('=') else {
            errors.push(diag(file, line, "expected key=value"));
            continue;
        };
        let key = key.trim();
        let value = raw_value.trim();
        if value.len() > MAX_FIELD_BYTES {
            errors.push(diag(file, line, "field exceeds 4096 bytes"));
            continue;
        }
        if value.is_empty() && !matches!(key, "default_milestone") {
            errors.push(diag(file, line, "value must not be empty"));
            continue;
        }
        match key {
            "version" => set_once(
                &mut version,
                value.parse().ok(),
                file,
                line,
                "version",
                &mut errors,
            ),
            "name" => set_once_string(&mut name, value, file, line, "name", &mut errors),
            "mission" => set_once_string(&mut mission, value, file, line, "mission", &mut errors),
            "default_milestone" => set_once_optional(
                &mut default_milestone,
                value,
                file,
                line,
                "default_milestone",
                &mut errors,
            ),
            "allowed_path" => push_bounded(&mut allowed_paths, value, file, line, &mut errors),
            "forbidden_path" => push_bounded(&mut forbidden_paths, value, file, line, &mut errors),
            "capability" => match capability_from_name(value) {
                Some(item) => push_bounded(&mut capabilities, item, file, line, &mut errors),
                None => errors.push(diag(file, line, "unknown capability")),
            },
            "approval" => match approval_from_name(value) {
                Some(item) => push_bounded(&mut approvals, item, file, line, &mut errors),
                None => errors.push(diag(file, line, "unknown approval boundary")),
            },
            "gate" => push_bounded(&mut gates, value, file, line, &mut errors),
            _ => errors.push(diag(file, line, "unknown blueprint key")),
        }
    }
    if version != Some(BLUEPRINT_VERSION) {
        errors.push(diag(file, 0, "version must be 1"));
    }
    if name.is_none() {
        errors.push(diag(file, 0, "name is required"));
    }
    if mission.is_none() {
        errors.push(diag(file, 0, "mission is required"));
    }
    for path in &allowed_paths {
        if forbidden_paths.contains(path) {
            errors.push(diag(
                file,
                0,
                &format!("path is both allowed and forbidden: {path}"),
            ));
        }
    }
    let mut seen_capabilities = Vec::new();
    for capability in &capabilities {
        if seen_capabilities.contains(capability) {
            errors.push(diag(
                file,
                0,
                &format!("capability granted more than once: {capability}"),
            ));
        } else {
            seen_capabilities.push(*capability);
        }
    }
    let mut seen_approvals = Vec::new();
    for approval in &approvals {
        if seen_approvals.contains(approval) {
            errors.push(diag(
                file,
                0,
                &format!("approval declared more than once: {}", approval.as_str()),
            ));
        } else {
            seen_approvals.push(*approval);
        }
    }
    if !errors.is_empty() {
        return Err(IntakeError::Validation(errors));
    }
    Ok(ProjectBlueprint {
        version: BLUEPRINT_VERSION,
        name: name.expect("validated name"),
        mission: mission.expect("validated mission"),
        default_milestone,
        allowed_paths,
        forbidden_paths,
        capabilities,
        approvals,
        gates,
    })
}

fn parse_guidelines(file: &str, bytes: &[u8]) -> Result<GuidelineDocument, IntakeError> {
    if bytes.len() > MAX_GUIDELINE_BYTES {
        return Err(validation(file, 0, "guidelines exceed 256 KiB"));
    }
    let text = std::str::from_utf8(bytes)
        .map_err(|_| validation(file, 0, "guidelines must be valid UTF-8"))?;
    let mut lines = text.lines();
    let Some(header) = lines.next() else {
        return Err(validation(
            file,
            1,
            "guidelines require '# AgentForge Guidelines v1'",
        ));
    };
    if header.trim() != "# AgentForge Guidelines v1" {
        return Err(validation(
            file,
            1,
            "guidelines require '# AgentForge Guidelines v1'",
        ));
    }
    let body = lines.collect::<Vec<_>>().join("\n");
    if body.trim().is_empty() {
        return Err(validation(file, 2, "guideline body must not be empty"));
    }
    Ok(GuidelineDocument {
        version: GUIDELINES_VERSION,
        body,
    })
}

fn set_once<T>(
    slot: &mut Option<T>,
    value: Option<T>,
    file: &str,
    line: usize,
    key: &str,
    errors: &mut Vec<ValidationError>,
) {
    if slot.is_some() {
        errors.push(diag(file, line, &format!("duplicate {key}")));
    } else if let Some(value) = value {
        *slot = Some(value);
    } else {
        errors.push(diag(file, line, &format!("invalid {key}")));
    }
}

fn set_once_string(
    slot: &mut Option<String>,
    value: &str,
    file: &str,
    line: usize,
    key: &str,
    errors: &mut Vec<ValidationError>,
) {
    if slot.is_some() {
        errors.push(diag(file, line, &format!("duplicate {key}")));
    } else {
        *slot = Some(value.to_owned());
    }
}

fn set_once_optional(
    slot: &mut Option<String>,
    value: &str,
    file: &str,
    line: usize,
    key: &str,
    errors: &mut Vec<ValidationError>,
) {
    if slot.is_some() {
        errors.push(diag(file, line, &format!("duplicate {key}")));
    } else if !value.is_empty() {
        *slot = Some(value.to_owned());
    }
}

fn push_bounded<T>(
    items: &mut Vec<T>,
    value: impl Into<T>,
    file: &str,
    line: usize,
    errors: &mut Vec<ValidationError>,
) {
    if items.len() >= MAX_LIST_ITEMS {
        errors.push(diag(file, line, "list exceeds 128 items"));
    } else {
        items.push(value.into());
    }
}

fn validation(file: &str, line: usize, message: &str) -> IntakeError {
    IntakeError::Validation(vec![diag(file, line, message)])
}

fn diag(file: &str, line: usize, message: &str) -> ValidationError {
    ValidationError {
        file: file.to_owned(),
        line,
        message: message.to_owned(),
    }
}

/// Parses a stable capability name from a blueprint or CLI value.
#[must_use]
pub fn capability_from_name(value: &str) -> Option<Capability> {
    Some(match value {
        "read_repository" => Capability::ReadRepository,
        "write_owned_paths" => Capability::WriteOwnedPaths,
        "run_local_commands" => Capability::RunLocalCommands,
        "use_network" => Capability::UseNetwork,
        "read_github" => Capability::ReadGitHub,
        "write_github" => Capability::WriteGitHub,
        "manage_worktrees" => Capability::ManageWorktrees,
        "read_secrets" => Capability::ReadSecrets,
        "use_mcp_tools" => Capability::UseMcpTools,
        "create_pull_request" => Capability::CreatePullRequest,
        "merge_protected_branch" => Capability::MergeProtectedBranch,
        "deploy_staging" => Capability::DeployStaging,
        "deploy_production" => Capability::DeployProduction,
        _ => return None,
    })
}

/// Parses a stable approval-boundary name from a blueprint or CLI value.
#[must_use]
pub fn approval_from_name(value: &str) -> Option<ApprovalBoundary> {
    Some(match value {
        "activate_implementation_plan" => ApprovalBoundary::ActivateImplementationPlan,
        "expand_task_scope" => ApprovalBoundary::ExpandTaskScope,
        "change_dependencies" => ApprovalBoundary::ChangeDependencies,
        "elevate_capability" => ApprovalBoundary::ElevateCapability,
        "access_secrets" => ApprovalBoundary::AccessSecrets,
        "destructive_data_migration" => ApprovalBoundary::DestructiveDataMigration,
        "irreversible_external_change" => ApprovalBoundary::IrreversibleExternalChange,
        "merge_protected_branch" => ApprovalBoundary::MergeProtectedBranch,
        "publish_release" => ApprovalBoundary::PublishRelease,
        "deploy_production" => ApprovalBoundary::DeployProduction,
        "change_governance_rules" => ApprovalBoundary::ChangeGovernanceRules,
        _ => return None,
    })
}

/// Parses a stable agent-role name from a CLI value.
#[must_use]
pub fn role_from_name(value: &str) -> Option<AgentRole> {
    Some(match value {
        "planner" => AgentRole::Planner,
        "architect" => AgentRole::Architect,
        "researcher" => AgentRole::Researcher,
        "implementer" => AgentRole::Implementer,
        "tester" => AgentRole::Tester,
        "reviewer" => AgentRole::Reviewer,
        "security_reviewer" => AgentRole::SecurityReviewer,
        "integrator" => AgentRole::Integrator,
        "release_manager" => AgentRole::ReleaseManager,
        _ => return None,
    })
}

fn starter_blueprint() -> String {
    "# AgentForge Project Blueprint v1\nversion=1\nname=My Project\nmission=Describe the project outcome.\ndefault_milestone=P1-M003\nallowed_path=src\nforbidden_path=.env\ncapability=read_repository\ncapability=write_owned_paths\ngate=full\n".to_owned()
}

fn starter_guidelines() -> String {
    "# AgentForge Guidelines v1\n\n- Keep changes within the task contract.\n- Record evidence for every consequential boundary.\n- Ask for approval when authority is missing.\n".to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temporary_root() -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("agentforge-intake-{suffix}"));
        fs::create_dir_all(&path).expect("directory");
        path
    }

    #[test]
    fn initialization_is_non_overwriting_and_validates() {
        let root = temporary_root();
        let (blueprint, guidelines) = initialize(&root).expect("initialize");
        let bundle = load(&root).expect("load");
        assert_eq!(bundle.blueprint.version, BLUEPRINT_VERSION);
        assert_eq!(bundle.guidelines.version, GUIDELINES_VERSION);
        assert!(matches!(
            initialize(&root),
            Err(IntakeError::AlreadyExists(_))
        ));
        assert!(blueprint.exists());
        assert!(guidelines.exists());
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn malformed_blueprint_reports_source_location() {
        let error =
            parse_blueprint("blueprint.conf", b"version=1\nunknown=x\n").expect_err("invalid");
        assert!(
            matches!(error, IntakeError::Validation(errors) if errors.iter().any(|e| e.line == 2))
        );
    }

    #[test]
    fn task_uses_only_structured_defaults() {
        let blueprint = ProjectBlueprint {
            version: 1,
            name: "x".into(),
            mission: "y".into(),
            default_milestone: Some("P1-M003".into()),
            allowed_paths: vec!["src".into()],
            forbidden_paths: vec![".env".into()],
            capabilities: vec![Capability::ReadRepository],
            approvals: Vec::new(),
            gates: vec!["full".into()],
        };
        let task = build_task(
            &TaskDraft::new("T1", AgentRole::Implementer, "edit"),
            &blueprint,
        )
        .expect("task");
        assert_eq!(task.allowed_paths, vec!["src"]);
        assert_eq!(task.capabilities, vec![Capability::ReadRepository]);
        assert_eq!(task.required_gates, vec!["full"]);
    }
}
