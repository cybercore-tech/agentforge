//! Deterministic, read-only operator reports for AgentForge projects.

use agentforge_audit::{AuditStore, FileAuditStore};
use agentforge_core::task::TaskState;
use agentforge_intake::IntakeError;
use agentforge_state::{FileTaskStore, StateError, TaskStore};
use agentforge_worktree::{GitOperation, WorktreeError, WorktreeManager};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

const AUDIT_RELATIVE_PATH: &str = ".forge/audit.log";
const MAX_RECENT_EVENTS: usize = 8;
const MAX_RENDERED_BYTES: usize = 16 * 1024;

/// A bounded source diagnostic.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HudDiagnostic {
    /// Stable source name.
    pub source: String,
    /// Human-readable failure detail.
    pub message: String,
}

impl HudDiagnostic {
    fn new(source: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            source: source.into(),
            message: message.into(),
        }
    }
}

/// Failure while collecting a HUD snapshot.
#[derive(Debug)]
pub enum HudError {
    /// One required source could not be read or verified.
    Source(HudDiagnostic),
}

impl std::fmt::Display for HudError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Source(diagnostic) => {
                write!(
                    formatter,
                    "{} source unavailable: {}",
                    diagnostic.source, diagnostic.message
                )
            }
        }
    }
}

impl std::error::Error for HudError {}

/// Project identity projected from the validated intake bundle.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectSummary {
    /// Human-readable project name.
    pub name: String,
    /// Project mission statement.
    pub mission: String,
    /// Guideline schema version.
    pub guidelines_version: u16,
}

/// Counts of tasks in each durable lifecycle state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TaskSummary {
    /// Whether a durable task snapshot was present.
    pub available: bool,
    /// Number of pending tasks.
    pub pending: usize,
    /// Number of running tasks.
    pub running: usize,
    /// Number of succeeded tasks.
    pub succeeded: usize,
    /// Number of failed tasks.
    pub failed: usize,
    /// Number of blocked tasks.
    pub blocked: usize,
    /// Number of cancelled tasks.
    pub cancelled: usize,
}

impl Default for TaskSummary {
    fn default() -> Self {
        Self {
            available: true,
            pending: 0,
            running: 0,
            succeeded: 0,
            failed: 0,
            blocked: 0,
            cancelled: 0,
        }
    }
}

/// Summary of the verified append-only audit log.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuditSummary {
    /// Number of verified records.
    pub records: usize,
    /// Latest verified sequence, if any.
    pub latest_sequence: Option<u64>,
    /// Stable recent event labels, capped by [`MAX_RECENT_EVENTS`].
    pub recent_events: Vec<String>,
}

/// A verified managed worktree projection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorktreeSummary {
    /// Owning task ID.
    pub task_id: String,
    /// Worktree path.
    pub path: PathBuf,
    /// Whether changes are present.
    pub dirty: bool,
    /// Unresolved operation, if any.
    pub operation: Option<String>,
}

/// Fully collected, bounded HUD state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HudSnapshot {
    /// Project identity.
    pub project: ProjectSummary,
    /// Durable task counts.
    pub tasks: TaskSummary,
    /// Verified audit evidence.
    pub audit: AuditSummary,
    /// Verified managed worktrees in task-ID order.
    pub worktrees: Vec<WorktreeSummary>,
}

/// Collects all required sources without creating or modifying project files.
pub fn collect(root: impl AsRef<Path>) -> Result<HudSnapshot, HudError> {
    let root = root.as_ref();
    let intake = agentforge_intake::load(root)
        .map_err(|error| HudError::Source(HudDiagnostic::new("intake", intake_message(&error))))?;

    let tasks = FileTaskStore::for_project_root(root)
        .load()
        .map_err(|error| HudError::Source(HudDiagnostic::new("task-state", error.to_string())))?
        .ok_or_else(|| {
            HudError::Source(HudDiagnostic::new(
                "task-state",
                "no durable task snapshot exists",
            ))
        })?;
    let mut task_summary = TaskSummary::default();
    for record in tasks.records() {
        match record.state() {
            TaskState::Pending => task_summary.pending += 1,
            TaskState::Running => task_summary.running += 1,
            TaskState::Succeeded => task_summary.succeeded += 1,
            TaskState::Failed => task_summary.failed += 1,
            TaskState::Blocked => task_summary.blocked += 1,
            TaskState::Cancelled => task_summary.cancelled += 1,
        }
    }

    let audit_path = root.join(AUDIT_RELATIVE_PATH);
    if !audit_path.is_file() {
        return Err(HudError::Source(HudDiagnostic::new(
            "audit",
            format!("missing audit log at {}", audit_path.display()),
        )));
    }
    let audit = FileAuditStore::open(&audit_path)
        .map_err(|error| HudError::Source(HudDiagnostic::new("audit", error.to_string())))?;
    let records = audit.records();
    let recent_start = records.len().saturating_sub(MAX_RECENT_EVENTS);
    let recent_events = records[recent_start..]
        .iter()
        .map(|record| {
            let event = record.event();
            format!(
                "#{} {:?}{}",
                event.sequence(),
                event.kind(),
                event
                    .task_id()
                    .map_or(String::new(), |task| format!(" task={task}"))
            )
        })
        .collect();
    let audit_summary = AuditSummary {
        records: records.len(),
        latest_sequence: records.last().map(|record| record.event().sequence()),
        recent_events,
    };

    let manager = WorktreeManager::new(root)
        .map_err(|error| HudError::Source(HudDiagnostic::new("worktree", error.to_string())))?;
    let worktrees = manager
        .list()
        .map_err(|error| HudError::Source(HudDiagnostic::new("worktree", error.to_string())))?
        .into_iter()
        .map(|status| WorktreeSummary {
            task_id: status.task_id().to_string(),
            path: status.path().to_path_buf(),
            dirty: status.is_dirty(),
            operation: status.operation().map(operation_name),
        })
        .collect();

    Ok(HudSnapshot {
        project: ProjectSummary {
            name: intake.blueprint.name,
            mission: intake.blueprint.mission,
            guidelines_version: intake.guidelines.version,
        },
        tasks: task_summary,
        audit: audit_summary,
        worktrees,
    })
}

/// Renders a snapshot as stable, bounded plain text.
#[must_use]
pub fn render(snapshot: &HudSnapshot) -> String {
    let mut output = String::new();
    let _ = writeln!(output, "AgentForge HUD");
    let _ = writeln!(output, "project: {}", snapshot.project.name);
    let _ = writeln!(output, "mission: {}", snapshot.project.mission);
    let _ = writeln!(
        output,
        "guidelines_version: {}",
        snapshot.project.guidelines_version
    );
    let _ = writeln!(output, "tasks:");
    let _ = writeln!(output, "  pending: {}", snapshot.tasks.pending);
    let _ = writeln!(output, "  running: {}", snapshot.tasks.running);
    let _ = writeln!(output, "  succeeded: {}", snapshot.tasks.succeeded);
    let _ = writeln!(output, "  failed: {}", snapshot.tasks.failed);
    let _ = writeln!(output, "  blocked: {}", snapshot.tasks.blocked);
    let _ = writeln!(output, "  cancelled: {}", snapshot.tasks.cancelled);
    let _ = writeln!(output, "audit_records: {}", snapshot.audit.records);
    let _ = writeln!(
        output,
        "audit_latest_sequence: {}",
        snapshot
            .audit
            .latest_sequence
            .map_or_else(|| "none".to_owned(), |value| value.to_string())
    );
    let _ = writeln!(output, "audit_recent:");
    for event in &snapshot.audit.recent_events {
        let _ = writeln!(output, "  - {event}");
    }
    let _ = writeln!(output, "worktrees: {}", snapshot.worktrees.len());
    for worktree in &snapshot.worktrees {
        let operation = worktree.operation.as_deref().unwrap_or("none");
        let _ = writeln!(
            output,
            "  - task={} dirty={} operation={} path={}",
            worktree.task_id,
            worktree.dirty,
            operation,
            worktree.path.display()
        );
    }
    if output.len() > MAX_RENDERED_BYTES {
        output.truncate(MAX_RENDERED_BYTES);
        output.push_str("\n[truncated]\n");
    }
    output
}

fn operation_name(operation: GitOperation) -> String {
    match operation {
        GitOperation::Merge => "merge",
        GitOperation::Rebase => "rebase",
        GitOperation::CherryPick => "cherry-pick",
        GitOperation::Revert => "revert",
    }
    .to_owned()
}

fn intake_message(error: &IntakeError) -> String {
    error.to_string()
}

#[allow(dead_code)]
fn _state_message(error: &StateError) -> String {
    error.to_string()
}

#[allow(dead_code)]
fn _worktree_message(error: &WorktreeError) -> String {
    error.to_string()
}

#[cfg(test)]
mod tests {
    use super::{AuditSummary, HudSnapshot, ProjectSummary, TaskSummary, WorktreeSummary, render};
    use std::path::PathBuf;

    fn snapshot() -> HudSnapshot {
        HudSnapshot {
            project: ProjectSummary {
                name: "demo".to_owned(),
                mission: "ship safely".to_owned(),
                guidelines_version: 1,
            },
            tasks: TaskSummary {
                pending: 1,
                running: 2,
                succeeded: 3,
                failed: 4,
                blocked: 5,
                cancelled: 6,
                ..TaskSummary::default()
            },
            audit: AuditSummary {
                records: 2,
                latest_sequence: Some(2),
                recent_events: vec!["#1 TaskCreated".to_owned()],
            },
            worktrees: vec![WorktreeSummary {
                task_id: "task-1".to_owned(),
                path: PathBuf::from(".forge/worktrees/task-1"),
                dirty: false,
                operation: None,
            }],
        }
    }

    #[test]
    fn rendering_is_deterministic() {
        assert_eq!(render(&snapshot()), render(&snapshot()));
    }

    #[test]
    fn rendering_is_bounded() {
        let mut value = snapshot();
        value.project.mission = "x".repeat(32 * 1024);
        assert!(render(&value).len() <= 16 * 1024 + "\n[truncated]\n".len());
    }
}
