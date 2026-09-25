//! Deterministic, read-only operator reports for AgentForge projects.

use agentforge_audit::{AuditEvent, AuditEventKind, AuditStore, FileAuditStore};
use agentforge_core::task::TaskState;
use agentforge_intake::IntakeError;
use agentforge_operator::leases::{LeaseView, WorkerView, list_leases, list_workers};
use agentforge_state::{FileTaskStore, StateError, TaskStore};
use agentforge_worktree::{GitOperation, WorktreeError, WorktreeManager};
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

const AUDIT_RELATIVE_PATH: &str = ".forge/audit.log";
const MAX_RECENT_EVENTS: usize = 8;
const MAX_RENDERED_BYTES: usize = 16 * 1024;
/// Maximum number of recent agent runs shown in the HUD.
pub const MAX_AGENT_RUNS: usize = 5;
/// Maximum number of active leases listed in the HUD (P4-M010).
pub const MAX_LEASES: usize = 16;
/// Maximum characters rendered from one audit field value.
const MAX_FIELD_CHARS: usize = 256;
/// Default watch refresh interval in milliseconds.
pub const DEFAULT_WATCH_INTERVAL_MS: u64 = 1_000;
/// Minimum accepted watch refresh interval in milliseconds.
pub const MIN_WATCH_INTERVAL_MS: u64 = 50;
/// Maximum accepted watch refresh interval in milliseconds.
pub const MAX_WATCH_INTERVAL_MS: u64 = 60_000;
/// Maximum accepted cooked-mode input line length.
pub const MAX_WATCH_INPUT_BYTES: usize = 256;

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

/// Bounded configuration for the live HUD watch loop.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WatchConfig {
    interval_ms: u64,
}

impl WatchConfig {
    /// Creates a configuration, clamping the interval to the documented bounds.
    #[must_use]
    pub const fn new(interval_ms: u64) -> Self {
        let interval_ms = if interval_ms < MIN_WATCH_INTERVAL_MS {
            MIN_WATCH_INTERVAL_MS
        } else if interval_ms > MAX_WATCH_INTERVAL_MS {
            MAX_WATCH_INTERVAL_MS
        } else {
            interval_ms
        };
        Self { interval_ms }
    }

    /// Returns the effective refresh interval in milliseconds.
    #[must_use]
    pub const fn interval_ms(self) -> u64 {
        self.interval_ms
    }
}

impl Default for WatchConfig {
    fn default() -> Self {
        Self::new(DEFAULT_WATCH_INTERVAL_MS)
    }
}

/// One bounded command accepted by the cooked-mode watch loop.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WatchCommand {
    /// Refresh immediately.
    Refresh,
    /// Print command help.
    Help,
    /// Exit successfully.
    Quit,
    /// Ignore an empty line.
    Ignore,
    /// Report one bounded invalid input line.
    Invalid(String),
}

/// Parses one cooked-mode command without shell interpretation.
#[must_use]
pub fn parse_watch_command(input: &str) -> WatchCommand {
    let command = input.trim();
    if command.is_empty() {
        return WatchCommand::Ignore;
    }
    match command.to_ascii_lowercase().as_str() {
        "r" | "refresh" => WatchCommand::Refresh,
        "h" | "help" => WatchCommand::Help,
        "q" | "quit" => WatchCommand::Quit,
        _ => WatchCommand::Invalid(bound_input(command)),
    }
}

/// Returns the stable watch-mode command help text.
#[must_use]
pub const fn watch_help() -> &'static str {
    "watch commands: r/refresh refresh, h/help help, q/quit exit"
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

/// Gate results recorded for one agent run.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct GateSummary {
    /// Number of gates that passed.
    pub passed: usize,
    /// Number of gates recorded for the run.
    pub total: usize,
    /// First gate that did not pass, as `(gate, outcome)`.
    pub first_failure: Option<(String, String)>,
}

/// One agent run projected from an `AgentFinished` audit event.
///
/// The fields recorded since P2-M029 are optional, so earlier runs that carry none of them still
/// project cleanly.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgentRunSummary {
    /// Sequence of the `AgentFinished` event.
    pub sequence: u64,
    /// Owning task ID, if recorded.
    pub task_id: Option<String>,
    /// Recorded agent exit code (`none` when the platform provided none).
    pub exit_code: Option<String>,
    /// Recorded termination label.
    pub termination: Option<String>,
    /// Whether captured output was truncated.
    pub output_truncated: bool,
    /// Project-relative stdout evidence log.
    pub stdout_log: Option<String>,
    /// Project-relative stderr evidence log.
    pub stderr_log: Option<String>,
    /// Why the evidence logs could not be written, if they could not.
    pub evidence_error: Option<String>,
    /// Gates recorded for this task after the run and before its next agent start.
    pub gates: GateSummary,
    /// When the run's `AgentStarted` was recorded (wall-clock ms), if timestamped (P0-M015).
    pub started_at_ms: Option<u64>,
    /// When its `AgentFinished` was recorded (wall-clock ms), if timestamped.
    pub finished_at_ms: Option<u64>,
}

impl AgentRunSummary {
    fn from_event(event: &AuditEvent) -> Self {
        let field = |key: &str| event.fields().get(key).map(|value| bound_field(value));
        Self {
            sequence: event.sequence(),
            task_id: event.task_id().map(bound_field),
            exit_code: field("exit_code"),
            termination: field("termination"),
            output_truncated: event
                .fields()
                .get("output_truncated")
                .is_some_and(|value| value == "true"),
            stdout_log: field("stdout_log"),
            stderr_log: field("stderr_log"),
            evidence_error: field("evidence_error"),
            gates: GateSummary::default(),
            started_at_ms: None,
            finished_at_ms: event.has_timestamp().then(|| event.timestamp()),
        }
    }

    /// Run duration in whole seconds, when both ends are timestamped.
    #[must_use]
    pub fn duration_seconds(&self) -> Option<u64> {
        Some(self.finished_at_ms?.checked_sub(self.started_at_ms?)? / 1_000)
    }

    fn record_gate(&mut self, event: &AuditEvent) {
        let outcome = event
            .fields()
            .get("outcome")
            .map_or("unknown", String::as_str);
        self.gates.total += 1;
        if outcome == "passed" {
            self.gates.passed += 1;
        } else if self.gates.first_failure.is_none() {
            let gate = event.fields().get("gate").map_or("unknown", String::as_str);
            self.gates.first_failure = Some((bound_field(gate), bound_field(outcome)));
        }
    }
}

/// Projects the newest [`MAX_AGENT_RUNS`] agent runs, oldest first, from audit events in sequence
/// order.
///
/// Each `AgentFinished` event starts a run. Later `GateFinished` events for the same task belong
/// to that run until the task's next `AgentStarted` or `AgentFinished`.
pub fn agent_runs<'a>(events: impl IntoIterator<Item = &'a AuditEvent>) -> Vec<AgentRunSummary> {
    let mut runs = Vec::new();
    let mut open = BTreeMap::<String, usize>::new();
    let mut started = BTreeMap::<String, u64>::new();
    for event in events {
        match event.kind() {
            AuditEventKind::AgentStarted => {
                if let Some(task) = event.task_id() {
                    open.remove(task);
                    if event.has_timestamp() {
                        started.insert(task.to_owned(), event.timestamp());
                    } else {
                        started.remove(task);
                    }
                }
            }
            AuditEventKind::AgentFinished => {
                let mut run = AgentRunSummary::from_event(event);
                if let Some(task) = event.task_id() {
                    open.insert(task.to_owned(), runs.len());
                    run.started_at_ms = started.remove(task);
                }
                runs.push(run);
            }
            AuditEventKind::GateFinished => {
                if let Some(&index) = event.task_id().and_then(|task| open.get(task)) {
                    runs[index].record_gate(event);
                }
            }
            _ => {}
        }
    }
    let start = runs.len().saturating_sub(MAX_AGENT_RUNS);
    runs.split_off(start)
}

/// One registered worker as the coordinator sees it (P4-M010).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkerSummary {
    /// Worker ID.
    pub worker_id: String,
    /// Declared platform.
    pub platform: String,
    /// Declared lease capacity.
    pub max_leases: u16,
    /// Active leases at the observation time.
    pub active_leases: usize,
    /// When the worker last did something recorded in the audit log (an event whose actor is
    /// `worker:<id>`: a claim, renewal, release, or a same-host run), if ever.
    pub last_seen_ms: Option<u64>,
    /// What that last recorded event was (the lease action, or the event kind).
    pub last_action: Option<String>,
}

/// One active lease (P4-M010).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LeaseSummary {
    /// Lease ID.
    pub lease_id: String,
    /// Leased task.
    pub task_id: String,
    /// Owning worker.
    pub worker_id: String,
    /// Task lease generation.
    pub generation: u64,
    /// Expiry on the coordinator's clock.
    pub expires_at_ms: u64,
}

/// Leases by effective state at the observation time (P4-M010).
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct LeaseTotals {
    /// Active leases.
    pub active: usize,
    /// Expired leases, including active ones past expiry that no sweep has recorded yet.
    pub expired: usize,
    /// Released leases.
    pub released: usize,
}

/// Projects registered workers with their last recorded activity from audit events in sequence
/// order. Only timestamped events count as sightings.
pub fn worker_summaries<'a>(
    workers: &[WorkerView],
    events: impl IntoIterator<Item = &'a AuditEvent>,
) -> Vec<WorkerSummary> {
    let mut seen = BTreeMap::<&str, (u64, String)>::new();
    for event in events {
        let Some(worker) = event.actor().strip_prefix("worker:") else {
            continue;
        };
        if !event.has_timestamp() {
            continue;
        }
        let action = event
            .fields()
            .get("action")
            .filter(|_| event.kind() == AuditEventKind::LeaseRecorded)
            .map_or_else(
                || format!("{:?}", event.kind()),
                |action| bound_field(action),
            );
        seen.insert(worker, (event.timestamp(), action));
    }
    workers
        .iter()
        .map(|view| {
            let id = view.descriptor.worker_id().as_str();
            let last = seen.get(id);
            WorkerSummary {
                worker_id: id.to_owned(),
                platform: bound_field(view.descriptor.platform()),
                max_leases: view.descriptor.max_concurrent_leases(),
                active_leases: view.active_leases,
                last_seen_ms: last.map(|(at, _)| *at),
                last_action: last.map(|(_, action)| action.clone()),
            }
        })
        .collect()
}

/// Totals every lease by effective state and lists the active ones, capped at [`MAX_LEASES`].
#[must_use]
pub fn lease_summaries(leases: &[LeaseView]) -> (LeaseTotals, Vec<LeaseSummary>) {
    let mut totals = LeaseTotals::default();
    let mut active = Vec::new();
    for lease in leases {
        match lease.state.as_str() {
            "active" => {
                totals.active += 1;
                if active.len() < MAX_LEASES {
                    active.push(LeaseSummary {
                        lease_id: bound_field(&lease.lease_id),
                        task_id: bound_field(&lease.task_id),
                        worker_id: bound_field(&lease.worker_id),
                        generation: lease.generation,
                        expires_at_ms: lease.expires_at_ms,
                    });
                }
            }
            "expired" => totals.expired += 1,
            _ => totals.released += 1,
        }
    }
    (totals, active)
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
    /// Most recent agent runs, oldest first, capped by [`MAX_AGENT_RUNS`].
    pub agent_runs: Vec<AgentRunSummary>,
    /// Registered workers in worker-ID order (P4-M010).
    pub workers: Vec<WorkerSummary>,
    /// Lease totals by effective state.
    pub lease_totals: LeaseTotals,
    /// Active leases in lease-ID order, capped at [`MAX_LEASES`].
    pub leases: Vec<LeaseSummary>,
    /// The observation time used for lease states and expiry (wall-clock ms).
    pub observed_at_ms: u64,
}

/// Collects all required sources without creating or modifying project files.
pub fn collect(root: impl AsRef<Path>) -> Result<HudSnapshot, HudError> {
    collect_at(root, agentforge_operator::leases::now_ms())
}

/// [`collect`] at a fixed observation time, which decides lease states and expiry (P4-M010).
pub fn collect_at(root: impl AsRef<Path>, now_ms: u64) -> Result<HudSnapshot, HudError> {
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
                "#{} {:?}{}{}",
                event.sequence(),
                event.kind(),
                event
                    .task_id()
                    .map_or(String::new(), |task| format!(" task={task}")),
                if event.has_timestamp() {
                    format!(" at={}", utc_timestamp(event.timestamp()))
                } else {
                    String::new()
                }
            )
        })
        .collect();
    let audit_summary = AuditSummary {
        records: records.len(),
        latest_sequence: records.last().map(|record| record.event().sequence()),
        recent_events,
    };
    let agent_runs = agent_runs(records.iter().map(|record| record.event()));
    let remote = |error: agentforge_operator::OperatorError| {
        HudError::Source(HudDiagnostic::new("remote-workers", error.to_string()))
    };
    let workers = worker_summaries(
        &list_workers(root, now_ms).map_err(remote)?,
        records.iter().map(|record| record.event()),
    );
    let (lease_totals, leases) = lease_summaries(&list_leases(root, now_ms).map_err(remote)?);

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
        agent_runs,
        workers,
        lease_totals,
        leases,
        observed_at_ms: now_ms,
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
    let _ = writeln!(output, "agent_runs:");
    if snapshot.agent_runs.is_empty() {
        let _ = writeln!(output, "  - none");
    }
    for run in &snapshot.agent_runs {
        let _ = writeln!(output, "  - {}", agent_run_line(run));
    }
    let _ = writeln!(output, "workers:");
    if snapshot.workers.is_empty() {
        let _ = writeln!(output, "  - none");
    }
    for worker in &snapshot.workers {
        let last_seen = match (worker.last_seen_ms, &worker.last_action) {
            (Some(at), Some(action)) => format!("{}({action})", utc_timestamp(at)),
            _ => "never".to_owned(),
        };
        let _ = writeln!(
            output,
            "  - {} platform={} leases={}/{} last-seen={last_seen}",
            worker.worker_id, worker.platform, worker.active_leases, worker.max_leases
        );
    }
    let totals = snapshot.lease_totals;
    let _ = writeln!(
        output,
        "leases: active={} expired={} released={}",
        totals.active, totals.expired, totals.released
    );
    for lease in &snapshot.leases {
        let _ = writeln!(
            output,
            "  - {} task={} worker={} gen={} expires-in={}s",
            lease.lease_id,
            lease.task_id,
            lease.worker_id,
            lease.generation,
            lease.expires_at_ms.saturating_sub(snapshot.observed_at_ms) / 1_000
        );
    }
    if totals.active > snapshot.leases.len() {
        let _ = writeln!(
            output,
            "  - ... {} more active",
            totals.active - snapshot.leases.len()
        );
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

fn agent_run_line(run: &AgentRunSummary) -> String {
    let mut line = format!(
        "#{} task={} agent-exit={}",
        run.sequence,
        run.task_id.as_deref().unwrap_or("none"),
        run.exit_code.as_deref().unwrap_or("unknown")
    );
    if let Some(termination) = &run.termination {
        let _ = write!(line, " termination={termination}");
    }
    if run.output_truncated {
        line.push_str(" output-truncated=true");
    }
    if let Some(seconds) = run.duration_seconds() {
        let _ = write!(line, " duration={seconds}s");
    }
    let _ = write!(line, " gates={}/{}", run.gates.passed, run.gates.total);
    if let Some((gate, outcome)) = &run.gates.first_failure {
        let _ = write!(line, " failed-gate={gate}:{outcome}");
    }
    if let Some(stdout) = &run.stdout_log {
        let _ = write!(line, " stdout={stdout}");
    }
    if let Some(stderr) = &run.stderr_log {
        let _ = write!(line, " stderr={stderr}");
    }
    if let Some(error) = &run.evidence_error {
        let _ = write!(line, " evidence-error={error}");
    }
    line
}

/// Formats wall-clock milliseconds as `YYYY-MM-DDTHH:MM:SSZ` (UTC), std only (P0-M015).
#[must_use]
pub fn utc_timestamp(milliseconds: u64) -> String {
    let seconds = milliseconds / 1_000;
    let (days, rest) = (seconds / 86_400, seconds % 86_400);
    let (hour, minute, second) = (rest / 3_600, rest % 3_600 / 60, rest % 60);
    // Civil date from days since 1970-01-01 (Howard Hinnant's algorithm).
    let z = i64::try_from(days).unwrap_or(i64::MAX / 2) + 719_468;
    let era = z.div_euclid(146_097);
    let day_of_era = z - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_index = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_index + 2) / 5 + 1;
    let month = if month_index < 10 {
        month_index + 3
    } else {
        month_index - 9
    };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
}

/// Bounds one audit value to a single line of at most [`MAX_FIELD_CHARS`] characters.
fn bound_field(value: &str) -> String {
    let mut bounded = value
        .chars()
        .take(MAX_FIELD_CHARS)
        .map(|character| {
            if character.is_control() {
                ' '
            } else {
                character
            }
        })
        .collect::<String>();
    if value.chars().count() > MAX_FIELD_CHARS {
        bounded.push('…');
    }
    bounded
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

fn bound_input(input: &str) -> String {
    let mut bounded = input
        .chars()
        .take(MAX_WATCH_INPUT_BYTES)
        .collect::<String>();
    if input.chars().count() > MAX_WATCH_INPUT_BYTES {
        bounded.push('…');
    }
    bounded
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
    use super::{
        AuditSummary, HudSnapshot, LeaseTotals, MAX_AGENT_RUNS, MAX_LEASES, ProjectSummary,
        TaskSummary, WatchCommand, WatchConfig, WorktreeSummary, agent_runs, lease_summaries,
        parse_watch_command, render, worker_summaries,
    };
    use agentforge_audit::{AuditEvent, AuditEventKind};
    use std::path::PathBuf;

    fn event(sequence: u64, kind: AuditEventKind, task: &str) -> AuditEvent {
        AuditEvent::new(sequence, format!("event-{sequence}"), kind, "test", 1).with_task_id(task)
    }

    fn finished(sequence: u64, task: &str, exit_code: &str) -> AuditEvent {
        event(sequence, AuditEventKind::AgentFinished, task)
            .with_field("termination", "exited")
            .with_field("exit_code", exit_code)
            .with_field("output_truncated", "false")
            .with_field(
                "stdout_log",
                format!(".forge/evidence/{task}/{sequence}-stdout.log"),
            )
            .with_field(
                "stderr_log",
                format!(".forge/evidence/{task}/{sequence}-stderr.log"),
            )
    }

    fn gate(sequence: u64, task: &str, name: &str, outcome: &str) -> AuditEvent {
        event(sequence, AuditEventKind::GateFinished, task)
            .with_field("gate", name)
            .with_field("outcome", outcome)
    }

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
            agent_runs: Vec::new(),
            workers: Vec::new(),
            lease_totals: LeaseTotals::default(),
            leases: Vec::new(),
            observed_at_ms: 0,
        }
    }

    fn worker_view(id: &str, active_leases: usize) -> agentforge_operator::leases::WorkerView {
        use agentforge_core::remote::{RemoteWorkerDescriptor, RemoteWorkerId, WorkerCapability};
        agentforge_operator::leases::WorkerView {
            descriptor: RemoteWorkerDescriptor::new(
                RemoteWorkerId::parse(id).expect("id"),
                "linux-x86_64",
                vec![WorkerCapability::parse("rust").expect("capability")],
                2,
            )
            .expect("descriptor"),
            active_leases,
        }
    }

    fn lease_view(
        id: &str,
        state: &str,
        expires_at_ms: u64,
    ) -> agentforge_operator::leases::LeaseView {
        agentforge_operator::leases::LeaseView {
            lease_id: id.to_owned(),
            task_id: format!("{id}-task"),
            worker_id: "remote-a".to_owned(),
            generation: 3,
            issued_at_ms: 0,
            expires_at_ms,
            state: state.to_owned(),
        }
    }

    #[test]
    fn workers_show_capacity_and_last_sighting_by_their_own_events() {
        let at = 1_790_315_054_000;
        let lease = |sequence, actor: &str, action: &str, timestamp| {
            AuditEvent::new(
                sequence,
                format!("lease-{sequence}"),
                AuditEventKind::LeaseRecorded,
                actor,
                timestamp,
            )
            .with_field("action", action)
            .with_field("worker_id", "remote-a")
        };
        let events = [
            lease(1, "operator", "granted", at),
            lease(2, "worker:remote-a", "claimed", at + 1_000),
            lease(3, "worker:remote-a", "renewed", at + 61_000),
            // An operator grant names the worker but is not a sighting of it.
            lease(4, "operator", "granted", at + 90_000),
            // Legacy events without a time are not sightings.
            lease(5, "worker:remote-b", "claimed", 1),
            AuditEvent::new(
                6,
                "s-6",
                AuditEventKind::AgentStarted,
                "worker:remote-c",
                at,
            ),
        ];
        let workers = worker_summaries(
            &[
                worker_view("remote-a", 1),
                worker_view("remote-b", 0),
                worker_view("remote-c", 0),
            ],
            &events,
        );
        assert_eq!(workers[0].last_seen_ms, Some(at + 61_000));
        assert_eq!(workers[0].last_action.as_deref(), Some("renewed"));
        assert_eq!(workers[1].last_seen_ms, None, "untimestamped");
        assert_eq!(workers[2].last_action.as_deref(), Some("AgentStarted"));

        let mut value = snapshot();
        value.workers = workers;
        let rendered = render(&value);
        assert!(
            rendered.contains(
                "workers:\n  - remote-a platform=linux-x86_64 leases=1/2 \
                 last-seen=2026-09-25T05:45:15Z(renewed)\n  - remote-b platform=linux-x86_64 \
                 leases=0/2 last-seen=never\n"
            ),
            "{rendered}"
        );
    }

    #[test]
    fn leases_show_totals_and_active_expiry() {
        let now = 1_790_315_054_000;
        let (totals, active) = lease_summaries(&[
            lease_view("L1", "active", now + 90_500),
            lease_view("L2", "expired", now - 1),
            lease_view("L3", "released", now),
            lease_view("L4", "released", now),
        ]);
        assert_eq!(
            totals,
            LeaseTotals {
                active: 1,
                expired: 1,
                released: 2
            }
        );
        let mut value = snapshot();
        value.lease_totals = totals;
        value.leases = active;
        value.observed_at_ms = now;
        let rendered = render(&value);
        assert!(
            rendered.contains(
                "leases: active=1 expired=1 released=2\n  - L1 task=L1-task worker=remote-a gen=3 \
                 expires-in=90s\n"
            ),
            "{rendered}"
        );
    }

    #[test]
    fn lease_listing_is_capped_and_says_so() {
        let views = (0..MAX_LEASES + 3)
            .map(|index| lease_view(&format!("L{index:02}"), "active", 5_000))
            .collect::<Vec<_>>();
        let (totals, active) = lease_summaries(&views);
        assert_eq!(totals.active, MAX_LEASES + 3);
        assert_eq!(active.len(), MAX_LEASES);
        let mut value = snapshot();
        value.lease_totals = totals;
        value.leases = active;
        assert!(render(&value).contains("  - ... 3 more active\n"));
    }

    #[test]
    fn an_empty_project_shows_no_workers_or_leases() {
        let rendered = render(&snapshot());
        assert!(
            rendered
                .contains("workers:\n  - none\nleases: active=0 expired=0 released=0\nworktrees:"),
            "{rendered}"
        );
    }

    #[test]
    fn agent_runs_summarize_exit_codes_gates_and_evidence() {
        let events = [
            event(1, AuditEventKind::AgentStarted, "task-1"),
            finished(2, "task-1", "0"),
            gate(3, "task-1", "workspace", "passed"),
            event(4, AuditEventKind::AgentStarted, "task-2"),
            finished(5, "task-2", "3"),
            gate(6, "task-2", "lint", "passed"),
            gate(7, "task-2", "test", "failed"),
            gate(8, "task-2", "docs", "timed_out"),
            event(9, AuditEventKind::FailureClassified, "task-2"),
            event(10, AuditEventKind::AgentStarted, "task-1"),
            gate(11, "task-1", "workspace", "failed"),
        ];
        let runs = agent_runs(&events);
        assert_eq!(runs.len(), 2);

        assert_eq!(runs[0].sequence, 2);
        assert_eq!(runs[0].task_id.as_deref(), Some("task-1"));
        assert_eq!(runs[0].exit_code.as_deref(), Some("0"));
        assert_eq!(runs[0].termination.as_deref(), Some("exited"));
        assert_eq!((runs[0].gates.passed, runs[0].gates.total), (1, 1));
        assert_eq!(runs[0].gates.first_failure, None);
        assert_eq!(
            runs[0].stdout_log.as_deref(),
            Some(".forge/evidence/task-1/2-stdout.log")
        );
        assert_eq!(
            runs[0].stderr_log.as_deref(),
            Some(".forge/evidence/task-1/2-stderr.log")
        );

        assert_eq!(runs[1].exit_code.as_deref(), Some("3"));
        assert_eq!((runs[1].gates.passed, runs[1].gates.total), (1, 3));
        assert_eq!(
            runs[1].gates.first_failure,
            Some(("test".to_owned(), "failed".to_owned()))
        );

        let mut value = snapshot();
        value.agent_runs = runs;
        let rendered = render(&value);
        assert!(rendered.contains(
            "agent_runs:\n  - #2 task=task-1 agent-exit=0 termination=exited gates=1/1 \
             stdout=.forge/evidence/task-1/2-stdout.log stderr=.forge/evidence/task-1/2-stderr.log\n"
        ));
        assert!(rendered.contains(
            "  - #5 task=task-2 agent-exit=3 termination=exited gates=1/3 failed-gate=test:failed \
             stdout=.forge/evidence/task-2/5-stdout.log stderr=.forge/evidence/task-2/5-stderr.log\n"
        ));
        let audit_recent = rendered.find("audit_recent:").expect("audit_recent");
        let agent_runs_start = rendered.find("agent_runs:").expect("agent_runs");
        let worktrees = rendered.find("worktrees:").expect("worktrees");
        assert!(audit_recent < agent_runs_start && agent_runs_start < worktrees);
    }

    #[test]
    fn utc_timestamps_are_formatted_exactly() {
        use super::utc_timestamp;
        assert_eq!(utc_timestamp(0), "1970-01-01T00:00:00Z");
        assert_eq!(utc_timestamp(951_782_400_000), "2000-02-29T00:00:00Z");
        assert_eq!(utc_timestamp(1_790_315_054_525), "2026-09-25T05:44:14Z");
        assert_eq!(utc_timestamp(4_102_444_799_000), "2099-12-31T23:59:59Z");
    }

    #[test]
    fn agent_runs_show_durations_only_when_timestamped() {
        let start = 1_790_315_054_000;
        let events = [
            AuditEvent::new(1, "s-1", AuditEventKind::AgentStarted, "test", start)
                .with_task_id("task-1"),
            AuditEvent::new(
                2,
                "f-2",
                AuditEventKind::AgentFinished,
                "test",
                start + 434_000,
            )
            .with_task_id("task-1")
            .with_field("exit_code", "0"),
            event(3, AuditEventKind::AgentStarted, "task-2"),
            event(4, AuditEventKind::AgentFinished, "task-2").with_field("exit_code", "0"),
        ];
        let runs = agent_runs(&events);
        assert_eq!(runs[0].duration_seconds(), Some(434));
        assert_eq!(
            runs[1].duration_seconds(),
            None,
            "legacy placeholder timestamps"
        );
        let mut value = snapshot();
        value.agent_runs = runs;
        let rendered = render(&value);
        assert!(
            rendered.contains("#2 task=task-1 agent-exit=0 duration=434s gates=0/0"),
            "{rendered}"
        );
        assert!(
            rendered.contains("#4 task=task-2 agent-exit=0 gates=0/0"),
            "{rendered}"
        );
    }

    #[test]
    fn agent_runs_keep_only_the_newest() {
        let events = (1..=u64::try_from(MAX_AGENT_RUNS).expect("bound") + 3)
            .map(|sequence| finished(sequence, "task-1", "0"))
            .collect::<Vec<_>>();
        let runs = agent_runs(&events);
        assert_eq!(runs.len(), MAX_AGENT_RUNS);
        assert_eq!(runs[0].sequence, 4);
        assert_eq!(
            runs.last().map(|run| run.sequence),
            Some(u64::try_from(MAX_AGENT_RUNS).expect("bound") + 3)
        );
    }

    #[test]
    fn pre_exit_field_runs_render_unknown_exit() {
        let events = [event(7, AuditEventKind::AgentFinished, "legacy")];
        let runs = agent_runs(&events);
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].exit_code, None);
        let mut value = snapshot();
        value.agent_runs = runs;
        assert!(render(&value).contains("  - #7 task=legacy agent-exit=unknown gates=0/0\n"));
    }

    #[test]
    fn agent_runs_report_evidence_errors_and_truncation() {
        let events = [event(3, AuditEventKind::AgentFinished, "task-1")
            .with_field("exit_code", "none")
            .with_field("termination", "timed_out")
            .with_field("output_truncated", "true")
            .with_field("evidence_error", "disk full\nretry")];
        let mut value = snapshot();
        value.agent_runs = agent_runs(&events);
        assert!(render(&value).contains(
            "  - #3 task=task-1 agent-exit=none termination=timed_out output-truncated=true \
             gates=0/0 evidence-error=disk full retry\n"
        ));
    }

    #[test]
    fn empty_agent_runs_render_none() {
        assert!(render(&snapshot()).contains("agent_runs:\n  - none\n"));
    }

    #[test]
    fn agent_run_rendering_is_deterministic_and_bounded() {
        let long = "x".repeat(8 * 1024);
        let events = (1..=u64::try_from(MAX_AGENT_RUNS).expect("bound"))
            .map(|sequence| {
                event(sequence, AuditEventKind::AgentFinished, &long)
                    .with_field("exit_code", "1")
                    .with_field("stdout_log", long.as_str())
                    .with_field("stderr_log", long.as_str())
                    .with_field("evidence_error", long.as_str())
            })
            .collect::<Vec<_>>();
        let mut value = snapshot();
        value.agent_runs = agent_runs(&events);
        let rendered = render(&value);
        assert_eq!(rendered, render(&value));
        assert!(rendered.len() <= 16 * 1024 + "\n[truncated]\n".len());
        assert!(rendered.contains("agent_runs:\n  - #1 task=xxx"));
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

    #[test]
    fn watch_commands_and_intervals_are_bounded() {
        assert_eq!(parse_watch_command(" R "), WatchCommand::Refresh);
        assert_eq!(parse_watch_command("help"), WatchCommand::Help);
        assert_eq!(parse_watch_command("q"), WatchCommand::Quit);
        assert_eq!(parse_watch_command(""), WatchCommand::Ignore);
        assert!(matches!(
            parse_watch_command("unknown"),
            WatchCommand::Invalid(_)
        ));
        assert_eq!(
            WatchConfig::new(0).interval_ms(),
            super::MIN_WATCH_INTERVAL_MS
        );
        assert_eq!(
            WatchConfig::new(u64::MAX).interval_ms(),
            super::MAX_WATCH_INTERVAL_MS
        );
    }
}
