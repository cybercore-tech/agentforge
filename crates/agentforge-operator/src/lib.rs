//! Explicit, audited operator actions over durable AgentForge task state.

use agentforge_audit::{AuditEvent, AuditEventKind, AuditStore, FileAuditStore};
use agentforge_core::agent::ApprovalBoundary;
use agentforge_core::task::{TaskGraph, TaskId, TaskState};
use agentforge_state::{FileTaskStore, TaskStore};
use std::fmt;
use std::path::Path;

const AUDIT_RELATIVE_PATH: &str = ".forge/audit.log";
const MAX_ACTOR_BYTES: usize = 256;

/// A bounded task projection for operator inspection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TaskInspection {
    /// Stable task identifier.
    pub task_id: String,
    /// Lifecycle state.
    pub state: String,
    /// Lifecycle revision.
    pub revision: u64,
    /// Milestone identity.
    pub milestone: String,
    /// Task goal.
    pub goal: String,
    /// Stable dependencies.
    pub dependencies: Vec<String>,
    /// Required human approvals.
    pub required_approvals: Vec<String>,
    /// Whether the task is currently ready.
    pub ready: bool,
}

/// Operator action failure.
#[derive(Debug)]
pub struct OperatorError(String);

impl OperatorError {
    fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl fmt::Display for OperatorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for OperatorError {}

/// Parses one stable approval-boundary identity.
pub fn parse_approval_boundary(value: &str) -> Option<ApprovalBoundary> {
    [
        ApprovalBoundary::ActivateImplementationPlan,
        ApprovalBoundary::ExpandTaskScope,
        ApprovalBoundary::ChangeDependencies,
        ApprovalBoundary::ElevateCapability,
        ApprovalBoundary::AccessSecrets,
        ApprovalBoundary::DestructiveDataMigration,
        ApprovalBoundary::IrreversibleExternalChange,
        ApprovalBoundary::MergeProtectedBranch,
        ApprovalBoundary::PublishRelease,
        ApprovalBoundary::DeployProduction,
        ApprovalBoundary::ChangeGovernanceRules,
    ]
    .into_iter()
    .find(|boundary| boundary.as_str() == value)
}

/// Inspects all tasks or one selected task in deterministic order.
pub fn inspect_tasks(
    root: impl AsRef<Path>,
    selected: Option<&TaskId>,
) -> Result<Vec<TaskInspection>, OperatorError> {
    let graph = load_graph(root.as_ref())?;
    let ready = graph
        .ready_task_ids()
        .map_err(|error| OperatorError::new(error.to_string()))?;
    let mut result = Vec::new();
    for record in graph.records() {
        if selected.is_some_and(|task_id| task_id != record.id()) {
            continue;
        }
        result.push(TaskInspection {
            task_id: record.id().to_string(),
            state: record.state().as_str().to_owned(),
            revision: record.revision(),
            milestone: record.task().milestone_id.clone(),
            goal: record.task().goal.clone(),
            dependencies: record.task().dependency_task_ids.clone(),
            required_approvals: record
                .task()
                .required_approvals
                .iter()
                .map(|approval| approval.as_str().to_owned())
                .collect(),
            ready: ready.iter().any(|task_id| task_id == record.id()),
        });
    }
    if result.is_empty() && selected.is_some() {
        return Err(OperatorError::new("task not found"));
    }
    Ok(result)
}

/// Records one explicit approval for a task-required boundary.
pub fn approve_task(
    root: impl AsRef<Path>,
    task_id: &TaskId,
    boundary: ApprovalBoundary,
    actor: &str,
) -> Result<(), OperatorError> {
    validate_actor(actor)?;
    let graph = load_graph(root.as_ref())?;
    let task = graph
        .get(task_id)
        .ok_or_else(|| OperatorError::new("task not found"))?;
    if !task.task().required_approvals.contains(&boundary) {
        return Err(OperatorError::new(
            "approval boundary is not required by task",
        ));
    }
    let mut audit = open_existing_audit(root.as_ref())?;
    if audit.records().iter().any(|record| {
        record.event().kind() == AuditEventKind::ApprovalRecorded
            && record.event().task_id() == Some(task_id.as_str())
            && record.event().fields().get("boundary").map(String::as_str)
                == Some(boundary.as_str())
    }) {
        return Ok(());
    }
    let sequence = next_sequence(&audit);
    let event = AuditEvent::new(
        sequence,
        format!("operator-approval-{sequence}"),
        AuditEventKind::ApprovalRecorded,
        actor,
        1,
    )
    .with_task_id(task_id.as_str())
    .with_field("boundary", boundary.as_str());
    audit
        .append(event)
        .map_err(|error| OperatorError::new(error.to_string()))?;
    Ok(())
}

/// Applies one validated lifecycle action and records its audit evidence.
pub fn transition_task(
    root: impl AsRef<Path>,
    task_id: &TaskId,
    next: TaskState,
    actor: &str,
) -> Result<u64, OperatorError> {
    validate_actor(actor)?;
    let root = root.as_ref();
    let store = FileTaskStore::for_project_root(root);
    let mut graph = store
        .load()
        .map_err(|error| OperatorError::new(error.to_string()))?
        .ok_or_else(|| OperatorError::new("task state snapshot is missing"))?;
    let previous = graph
        .get(task_id)
        .ok_or_else(|| OperatorError::new("task not found"))?
        .state();
    graph
        .transition(task_id, next)
        .map_err(|error| OperatorError::new(error.to_string()))?;
    let revision = graph
        .get(task_id)
        .expect("transition retained task")
        .revision();
    let mut audit = open_existing_audit(root)?;
    let sequence = next_sequence(&audit);
    let event = AuditEvent::new(
        sequence,
        format!("operator-transition-{sequence}"),
        AuditEventKind::TaskTransition,
        actor,
        1,
    )
    .with_task_id(task_id.as_str())
    .with_field("from", previous.as_str())
    .with_field("to", next.as_str());
    store
        .save(&graph)
        .map_err(|error| OperatorError::new(error.to_string()))?;
    audit
        .append(event)
        .map_err(|error| OperatorError::new(error.to_string()))?;
    Ok(revision)
}

/// Returns verified approvals for one task, excluding unrelated or malformed events.
pub fn approved_boundaries(
    root: impl AsRef<Path>,
    task_id: &TaskId,
) -> Result<Vec<ApprovalBoundary>, OperatorError> {
    let path = root.as_ref().join(AUDIT_RELATIVE_PATH);
    if !path.is_file() {
        return Ok(Vec::new());
    }
    let audit =
        FileAuditStore::open(path).map_err(|error| OperatorError::new(error.to_string()))?;
    let mut values = Vec::new();
    for record in audit.records() {
        let event = record.event();
        if event.kind() != AuditEventKind::ApprovalRecorded
            || event.task_id() != Some(task_id.as_str())
        {
            continue;
        }
        if let Some(boundary) = event
            .fields()
            .get("boundary")
            .and_then(|value| parse_approval_boundary(value))
        {
            if !values.contains(&boundary) {
                values.push(boundary);
            }
        }
    }
    values.sort_by_key(|boundary| boundary.as_str());
    Ok(values)
}

fn load_graph(root: &Path) -> Result<TaskGraph, OperatorError> {
    FileTaskStore::for_project_root(root)
        .load()
        .map_err(|error| OperatorError::new(error.to_string()))?
        .ok_or_else(|| OperatorError::new("task state snapshot is missing"))
}

fn open_existing_audit(root: &Path) -> Result<FileAuditStore, OperatorError> {
    let path = root.join(AUDIT_RELATIVE_PATH);
    if !path.is_file() {
        return Err(OperatorError::new("audit log is missing"));
    }
    FileAuditStore::open(path).map_err(|error| OperatorError::new(error.to_string()))
}

fn next_sequence(audit: &FileAuditStore) -> u64 {
    audit
        .records()
        .last()
        .map_or(1, |record| record.event().sequence() + 1)
}

fn validate_actor(actor: &str) -> Result<(), OperatorError> {
    if actor.trim().is_empty()
        || actor.len() > MAX_ACTOR_BYTES
        || actor.chars().any(char::is_control)
    {
        return Err(OperatorError::new(
            "actor must be non-empty, bounded, and printable",
        ));
    }
    Ok(())
}
