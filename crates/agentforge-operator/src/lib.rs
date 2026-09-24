//! Explicit, audited operator actions over durable AgentForge task state.

pub mod leases;
pub mod worker;

use agentforge_audit::{AuditEvent, AuditEventKind, AuditStore, FileAuditStore};
use agentforge_ci::{
    CiConclusion, CiMonitor, CiObservationRequest, CiProviderStore, CiRun, CiStatus,
    CommandCiMonitor, FailureClassification, FailureClassifier,
};
use agentforge_core::agent::ApprovalBoundary;
use agentforge_core::task::{TaskGraph, TaskId, TaskState};
use agentforge_state::{FileTaskStore, TaskStore};
use agentforge_worktree::{IntegrationReport, WorktreeDiff, WorktreeManager};
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
    /// Recorded approvals as `boundary` or `boundary@<source head>`, in audit order.
    pub recorded_approvals: Vec<String>,
}

/// One recorded approval and the reviewed commit it is bound to.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ApprovalRecord {
    /// Approved boundary.
    pub boundary: ApprovalBoundary,
    /// Task branch head the approval was recorded for (post-execution approvals since P1-M008).
    pub source_head: Option<String>,
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
        let recorded_approvals = approval_records(root.as_ref(), record.id())?
            .into_iter()
            .map(|approval| match approval.source_head {
                Some(head) => format!("{}@{head}", approval.boundary.as_str()),
                None => approval.boundary.as_str().to_owned(),
            })
            .collect();
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
            recorded_approvals,
        });
    }
    if result.is_empty() && selected.is_some() {
        return Err(OperatorError::new("task not found"));
    }
    Ok(result)
}

/// One recorded exact-SHA CI observation with classified failed jobs.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CiObservation {
    /// The single provider run selected for the exact SHA.
    pub run: CiRun,
    /// Classification for each job whose conclusion is `failure`, in provider job order.
    pub classifications: Vec<(String, FailureClassification)>,
}

impl CiObservation {
    /// Returns whether the run completed successfully.
    #[must_use]
    pub fn succeeded(&self) -> bool {
        self.run.conclusion() == Some(CiConclusion::Success)
    }

    /// Returns whether the provider has not reached a terminal conclusion yet.
    #[must_use]
    pub fn pending(&self) -> bool {
        self.run.status() != CiStatus::Completed
    }
}

/// Observes exactly one CI run for a commit through the project's reviewed provider, classifies
/// its failed jobs, and records the evidence. Nothing is recorded when observation fails, and
/// classification never changes task state.
pub fn observe_ci(
    root: impl AsRef<Path>,
    request: &CiObservationRequest,
    task_id: Option<&TaskId>,
) -> Result<CiObservation, OperatorError> {
    let root = root.as_ref();
    if let Some(task_id) = task_id {
        if load_graph(root)?.get(task_id).is_none() {
            return Err(OperatorError::new("task not found"));
        }
    }
    // Observation is often the first evidence in a project, so the log is created on first
    // use as the run paths do; an uninitialized project still fails closed.
    if !root.join(".forge").is_dir() {
        return Err(OperatorError::new(
            "project is not initialized: .forge directory is missing",
        ));
    }
    let mut audit = FileAuditStore::open(root.join(AUDIT_RELATIVE_PATH))
        .map_err(|error| OperatorError::new(error.to_string()))?;
    let config = CiProviderStore::new(root)
        .load()
        .map_err(|error| OperatorError::new(error.to_string()))?;
    let run = CommandCiMonitor::new(config)
        .observe_exact(request, root)
        .map_err(|error| OperatorError::new(error.to_string()))?;
    let classifications = run
        .jobs()
        .iter()
        .filter(|job| job.conclusion() == Some(CiConclusion::Failure))
        .map(|job| (job.name().to_owned(), FailureClassifier::classify(job)))
        .collect::<Vec<_>>();

    let with_task = |event: AuditEvent| match task_id {
        Some(task_id) => event.with_task_id(task_id.as_str()),
        None => event,
    };
    let sequence = next_sequence(&audit);
    let observed = with_task(
        AuditEvent::new(
            sequence,
            format!("ci-observed-{sequence}"),
            AuditEventKind::CiObserved,
            "operator",
            1,
        )
        .with_field("repository", audit_text(request.repository()))
        .with_field("workflow", audit_text(request.workflow()))
        .with_field("sha", run.head_sha())
        .with_field("run", audit_text(run.provider_id()))
        .with_field("status", run.status().as_str())
        .with_field(
            "conclusion",
            run.conclusion().map_or("none", CiConclusion::as_str),
        ),
    );
    audit
        .append(observed)
        .map_err(|error| OperatorError::new(error.to_string()))?;
    for (job, classification) in &classifications {
        let sequence = next_sequence(&audit);
        let event = with_task(
            AuditEvent::new(
                sequence,
                format!("ci-failure-classified-{sequence}"),
                AuditEventKind::FailureClassified,
                "operator",
                1,
            )
            .with_field("stage", "ci")
            .with_field("sha", run.head_sha())
            .with_field("job", audit_text(job))
            .with_field("category", classification.category().as_str())
            .with_field("marker", classification.matched_marker().unwrap_or("none")),
        );
        audit
            .append(event)
            .map_err(|error| OperatorError::new(error.to_string()))?;
    }
    Ok(CiObservation {
        run,
        classifications,
    })
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

/// Records one explicit approval for a task-required boundary.
///
/// Pre-execution approvals are recorded once. Post-execution approvals (merge, release,
/// deployment) require an accepted task with a managed worktree and are bound to its branch head;
/// the bound head is returned. Approving the same head again records nothing, and approving after
/// the head changed records a new approval.
pub fn approve_task(
    root: impl AsRef<Path>,
    task_id: &TaskId,
    boundary: ApprovalBoundary,
    actor: &str,
) -> Result<Option<String>, OperatorError> {
    validate_actor(actor)?;
    let root = root.as_ref();
    let graph = load_graph(root)?;
    let task = graph
        .get(task_id)
        .ok_or_else(|| OperatorError::new("task not found"))?;
    if !task.task().required_approvals.contains(&boundary) {
        return Err(OperatorError::new(
            "approval boundary is not required by task",
        ));
    }
    let source_head = if boundary.is_post_execution() {
        if task.state() != TaskState::Succeeded {
            return Err(OperatorError::new(format!(
                "{} is approved after review: the task is {}; review with `forge task diff`, \
                 run `forge task accept`, then approve",
                boundary.as_str(),
                task.state().as_str()
            )));
        }
        let manager =
            WorktreeManager::new(root).map_err(|error| OperatorError::new(error.to_string()))?;
        let status = manager
            .inspect(task_id)
            .map_err(|error| OperatorError::new(error.to_string()))?
            .ok_or_else(|| {
                OperatorError::new(format!(
                    "{} needs the task's managed worktree to bind the reviewed commit",
                    boundary.as_str()
                ))
            })?;
        Some(status.head().to_owned())
    } else {
        None
    };
    if approval_records(root, task_id)?
        .iter()
        .any(|record| record.boundary == boundary && record.source_head == source_head)
    {
        return Ok(source_head);
    }
    let mut audit = open_project_audit(root)?;
    let sequence = next_sequence(&audit);
    let mut event = AuditEvent::new(
        sequence,
        format!("operator-approval-{sequence}"),
        AuditEventKind::ApprovalRecorded,
        actor,
        1,
    )
    .with_task_id(task_id.as_str())
    .with_field("boundary", boundary.as_str());
    if let Some(head) = &source_head {
        event = event.with_field("source_head", head.as_str());
    }
    audit
        .append(event)
        .map_err(|error| OperatorError::new(error.to_string()))?;
    Ok(source_head)
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
    let mut audit = open_project_audit(root)?;
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

/// Returns every recorded approval for one task in audit order, with its bound head.
pub fn approval_records(
    root: impl AsRef<Path>,
    task_id: &TaskId,
) -> Result<Vec<ApprovalRecord>, OperatorError> {
    let path = root.as_ref().join(AUDIT_RELATIVE_PATH);
    if !path.is_file() {
        return Ok(Vec::new());
    }
    let audit =
        FileAuditStore::open(path).map_err(|error| OperatorError::new(error.to_string()))?;
    Ok(audit
        .records()
        .iter()
        .map(|record| record.event())
        .filter(|event| {
            event.kind() == AuditEventKind::ApprovalRecorded
                && event.task_id() == Some(task_id.as_str())
        })
        .filter_map(|event| {
            let boundary = parse_approval_boundary(event.fields().get("boundary")?)?;
            Some(ApprovalRecord {
                boundary,
                source_head: event.fields().get("source_head").cloned(),
            })
        })
        .collect())
}

/// Inspects a task's managed worktree against the currently checked-out target branch.
pub fn inspect_task_diff(
    root: impl AsRef<Path>,
    task_id: &TaskId,
) -> Result<WorktreeDiff, OperatorError> {
    let manager = WorktreeManager::new(root.as_ref())
        .map_err(|error| OperatorError::new(error.to_string()))?;
    let target = manager
        .current_branch()
        .map_err(|error| OperatorError::new(error.to_string()))?;
    manager
        .diff(task_id, &target)
        .map_err(|error| OperatorError::new(error.to_string()))
}

/// Integrates one succeeded, explicitly approved task into a checked-out target branch.
pub fn integrate_task(
    root: impl AsRef<Path>,
    task_id: &TaskId,
    target_branch: &str,
    actor: &str,
) -> Result<IntegrationReport, OperatorError> {
    validate_actor(actor)?;
    let root = root.as_ref();
    let graph = load_graph(root)?;
    let task = graph
        .get(task_id)
        .ok_or_else(|| OperatorError::new("task not found"))?;
    if task.state() != TaskState::Succeeded {
        return Err(OperatorError::new(
            "task must be succeeded before integration",
        ));
    }
    if !task
        .task()
        .has_capability(agentforge_core::agent::Capability::MergeProtectedBranch)
    {
        return Err(OperatorError::new(
            "task lacks merge_protected_branch capability",
        ));
    }
    if !task
        .task()
        .required_approvals
        .contains(&ApprovalBoundary::MergeProtectedBranch)
    {
        return Err(OperatorError::new(
            "task does not require merge_protected_branch approval",
        ));
    }
    let manager =
        WorktreeManager::new(root).map_err(|error| OperatorError::new(error.to_string()))?;
    let current_head = manager
        .inspect(task_id)
        .map_err(|error| OperatorError::new(error.to_string()))?
        .map(|status| status.head().to_owned())
        .ok_or_else(|| OperatorError::new("task has no managed worktree to integrate"))?;
    let merge_approvals = approval_records(root, task_id)?
        .into_iter()
        .filter(|record| record.boundary == ApprovalBoundary::MergeProtectedBranch)
        .collect::<Vec<_>>();
    let approved_head = merge_approvals
        .iter()
        .filter_map(|record| record.source_head.as_deref())
        .find(|head| *head == current_head);
    let Some(approved_head) = approved_head else {
        let message = match merge_approvals.last() {
            None => "merge_protected_branch approval is missing; review, accept, and approve \
                     the task first"
                .to_owned(),
            Some(ApprovalRecord {
                source_head: None, ..
            }) => "merge_protected_branch approval is not bound to a reviewed commit (recorded \
                   before P1-M008); review the task and approve it again"
                .to_owned(),
            Some(ApprovalRecord {
                source_head: Some(head),
                ..
            }) => format!(
                "merge_protected_branch was approved for {head}, but the task branch is now at \
                 {current_head}; review the new commits and approve again"
            ),
        };
        return Err(OperatorError::new(message));
    };
    let report = manager
        .integrate_expecting(task_id, target_branch, approved_head)
        .map_err(|error| OperatorError::new(error.to_string()))?;

    let mut audit = open_project_audit(root)?;
    let already_recorded = audit.records().iter().any(|record| {
        let event = record.event();
        event.kind() == AuditEventKind::IntegrationRecorded
            && event.task_id() == Some(task_id.as_str())
            && event.fields().get("target_branch").map(String::as_str) == Some(target_branch)
            && event.fields().get("source_head").map(String::as_str) == Some(report.source_head())
    });
    if !already_recorded {
        let sequence = next_sequence(&audit);
        let event = AuditEvent::new(
            sequence,
            format!("operator-integration-{sequence}"),
            AuditEventKind::IntegrationRecorded,
            actor,
            1,
        )
        .with_task_id(task_id.as_str())
        .with_field("source_branch", report.source_branch())
        .with_field("source_head", report.source_head())
        .with_field("target_branch", report.target_branch())
        .with_field("target_before", report.target_before())
        .with_field("target_after", report.target_after())
        .with_field(
            "outcome",
            if report.already_integrated() {
                "already_integrated"
            } else {
                "fast_forwarded"
            },
        );
        audit
            .append(event)
            .map_err(|error| OperatorError::new(error.to_string()))?;
    }
    Ok(report)
}

fn load_graph(root: &Path) -> Result<TaskGraph, OperatorError> {
    FileTaskStore::for_project_root(root)
        .load()
        .map_err(|error| OperatorError::new(error.to_string()))?
        .ok_or_else(|| OperatorError::new("task state snapshot is missing"))
}

/// Opens the project audit log, creating it on first use (P0-M013, finding 10).
///
/// `forge init` and `forge task create` do not create the log, so the first audited operator
/// action does. The task snapshot must already exist, so a log is never created outside an
/// initialized project.
pub(crate) fn open_project_audit(root: &Path) -> Result<FileAuditStore, OperatorError> {
    let path = root.join(AUDIT_RELATIVE_PATH);
    if !path.is_file() {
        if !FileTaskStore::for_project_root(root).path().is_file() {
            return Err(OperatorError::new(
                "task state snapshot is missing; refusing to create an audit log",
            ));
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| OperatorError::new(error.to_string()))?;
        }
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
