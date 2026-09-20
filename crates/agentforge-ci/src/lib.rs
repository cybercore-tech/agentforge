//! Read-only CI observation and conservative failure classification.

use std::collections::BTreeMap;
use std::fmt;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

const PROTOCOL_HEADER: &str = "agentforge-ci-v1";

/// A request for evidence about one repository workflow at one exact commit.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CiObservationRequest {
    repository: String,
    workflow: String,
    commit_sha: String,
}

impl CiObservationRequest {
    /// Creates an observation request.
    #[must_use]
    pub fn new(
        repository: impl Into<String>,
        workflow: impl Into<String>,
        commit_sha: impl Into<String>,
    ) -> Self {
        Self {
            repository: repository.into(),
            workflow: workflow.into(),
            commit_sha: commit_sha.into(),
        }
    }

    fn validate(&self) -> Result<String, CiObservationError> {
        validate_field(&self.repository, "repository")?;
        validate_field(&self.workflow, "workflow")?;
        normalize_sha(&self.commit_sha)
    }
}

/// A read-only source of exact-commit CI evidence.
pub trait CiMonitor {
    /// Observes exactly one provider run for `request` in `directory`.
    fn observe_exact(
        &self,
        request: &CiObservationRequest,
        directory: &Path,
    ) -> Result<CiRun, CiObservationError>;
}

/// Explicit configuration for a provider observation command.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommandCiMonitorConfig {
    executable: PathBuf,
    arguments: Vec<String>,
    environment: BTreeMap<String, String>,
    timeout: Duration,
    max_output_bytes: usize,
}

impl CommandCiMonitorConfig {
    /// Creates a configuration with a 60-second deadline and one MiB output budget.
    #[must_use]
    pub fn new(executable: impl Into<PathBuf>) -> Self {
        Self {
            executable: executable.into(),
            arguments: Vec::new(),
            environment: BTreeMap::new(),
            timeout: Duration::from_secs(60),
            max_output_bytes: 1024 * 1024,
        }
    }

    /// Adds one literal provider-command argument.
    #[must_use]
    pub fn with_argument(mut self, argument: impl Into<String>) -> Self {
        self.arguments.push(argument.into());
        self
    }

    /// Adds one explicit provider-command environment value.
    #[must_use]
    pub fn with_environment(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.environment.insert(key.into(), value.into());
        self
    }

    /// Sets the command deadline.
    #[must_use]
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Sets the combined stdout and stderr byte budget.
    #[must_use]
    pub fn with_max_output_bytes(mut self, max_output_bytes: usize) -> Self {
        self.max_output_bytes = max_output_bytes;
        self
    }

    fn validate(&self) -> Result<(), CiObservationError> {
        if !self.executable.is_absolute() {
            return Err(CiObservationError::InvalidConfiguration(
                "provider executable must be absolute",
            ));
        }
        if self.timeout.is_zero() || self.max_output_bytes == 0 {
            return Err(CiObservationError::InvalidConfiguration(
                "provider command limits must be nonzero",
            ));
        }
        for (key, value) in &self.environment {
            validate_field(key, "environment key")?;
            if value.chars().any(char::is_control) {
                return Err(CiObservationError::InvalidConfiguration(
                    "environment value contains a control character",
                ));
            }
        }
        Ok(())
    }
}

/// A direct-command implementation of [`CiMonitor`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommandCiMonitor {
    configuration: CommandCiMonitorConfig,
}

impl CommandCiMonitor {
    /// Creates a direct-command monitor from explicit configuration.
    #[must_use]
    pub fn new(configuration: CommandCiMonitorConfig) -> Self {
        Self { configuration }
    }
}

impl CiMonitor for CommandCiMonitor {
    fn observe_exact(
        &self,
        request: &CiObservationRequest,
        directory: &Path,
    ) -> Result<CiRun, CiObservationError> {
        self.configuration.validate()?;
        let requested_sha = request.validate()?;
        let directory = std::fs::canonicalize(directory).map_err(CiObservationError::Io)?;
        if !directory.is_dir() {
            return Err(CiObservationError::InvalidDirectory(directory));
        }

        let output = run_command(&self.configuration, request, &requested_sha, &directory)?;
        let runs = parse_protocol(&output)?;
        select_exact_run(runs, &requested_sha)
    }
}

/// A provider CI run bound to a reported exact head SHA.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CiRun {
    provider_id: String,
    head_sha: String,
    status: CiStatus,
    conclusion: Option<CiConclusion>,
    jobs: Vec<CiJob>,
}

impl CiRun {
    /// Returns the provider's immutable run identifier.
    #[must_use]
    pub fn provider_id(&self) -> &str {
        &self.provider_id
    }

    /// Returns the normalized full head SHA reported by the provider.
    #[must_use]
    pub fn head_sha(&self) -> &str {
        &self.head_sha
    }

    /// Returns the provider run status.
    #[must_use]
    pub const fn status(&self) -> CiStatus {
        self.status
    }

    /// Returns the provider conclusion when the run is terminal.
    #[must_use]
    pub const fn conclusion(&self) -> Option<CiConclusion> {
        self.conclusion
    }

    /// Returns jobs in provider protocol order.
    #[must_use]
    pub fn jobs(&self) -> &[CiJob] {
        &self.jobs
    }
}

/// A provider job belonging to a [`CiRun`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CiJob {
    provider_id: String,
    name: String,
    status: CiStatus,
    conclusion: Option<CiConclusion>,
    failure_excerpt: String,
}

impl CiJob {
    /// Returns the provider's immutable job identifier.
    #[must_use]
    pub fn provider_id(&self) -> &str {
        &self.provider_id
    }

    /// Returns the provider job name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the provider job status.
    #[must_use]
    pub const fn status(&self) -> CiStatus {
        self.status
    }

    /// Returns the provider conclusion when the job is terminal.
    #[must_use]
    pub const fn conclusion(&self) -> Option<CiConclusion> {
        self.conclusion
    }

    /// Returns the bounded, verbatim protocol failure excerpt.
    #[must_use]
    pub fn failure_excerpt(&self) -> &str {
        &self.failure_excerpt
    }
}

/// Provider execution status.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CiStatus {
    /// The provider has queued the run or job.
    Queued,
    /// The provider is executing the run or job.
    InProgress,
    /// The provider has reached a terminal conclusion.
    Completed,
}

/// Provider terminal conclusion.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CiConclusion {
    /// The provider reported success.
    Success,
    /// The provider reported failure.
    Failure,
    /// The provider cancelled execution.
    Cancelled,
    /// The provider skipped execution.
    Skipped,
    /// The provider timed out execution.
    TimedOut,
    /// The provider requires further action.
    ActionRequired,
    /// The provider reported a neutral result.
    Neutral,
}

/// Deterministic category assigned to failed CI evidence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FailureCategory {
    /// Test or asserted behavior failed.
    SemanticTest,
    /// Source failed to compile or type-check.
    CompilationType,
    /// Formatter or linter rejected the source.
    FormattingLint,
    /// Generated output or serialized content was corrupt.
    GeneratedContentCorruption,
    /// Toolchain, registry, or dependency resolution failed.
    DependencyToolchain,
    /// Documentation or repository text policy failed.
    DocumentationTextPolicy,
    /// Repository workflow or governance validation failed.
    WorkflowGovernance,
    /// Provider, runner, or network infrastructure failed.
    Infrastructure,
    /// Evidence did not justify a more specific category.
    Unknown,
}

/// A stable classifier result for one job.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FailureClassification {
    category: FailureCategory,
    matched_marker: Option<&'static str>,
}

impl FailureClassification {
    /// Returns the conservative failure category.
    #[must_use]
    pub const fn category(&self) -> FailureCategory {
        self.category
    }

    /// Returns the stable marker that selected the category, when one matched.
    #[must_use]
    pub const fn matched_marker(&self) -> Option<&'static str> {
        self.matched_marker
    }
}

/// Classifies immutable failed-job evidence without authorizing repair.
#[derive(Clone, Copy, Debug, Default)]
pub struct FailureClassifier;

impl FailureClassifier {
    /// Classifies `job` using documented category precedence.
    #[must_use]
    pub fn classify(job: &CiJob) -> FailureClassification {
        if job.conclusion != Some(CiConclusion::Failure) {
            return unknown();
        }
        let excerpt = job.failure_excerpt.to_ascii_lowercase();
        for (category, markers) in CLASSIFICATION_RULES {
            if let Some(marker) = markers.iter().find(|marker| excerpt.contains(**marker)) {
                return FailureClassification {
                    category: *category,
                    matched_marker: Some(marker),
                };
            }
        }
        unknown()
    }
}

const CLASSIFICATION_RULES: &[(FailureCategory, &[&str])] = &[
    (
        FailureCategory::GeneratedContentCorruption,
        &["generated content", "checksum mismatch", "corrupt snapshot"],
    ),
    (
        FailureCategory::DocumentationTextPolicy,
        &["text policy", "markdown policy", "documentation policy"],
    ),
    (
        FailureCategory::WorkflowGovernance,
        &[
            "plan policy",
            "active plan",
            "repository structure",
            "workflow policy",
        ],
    ),
    (
        FailureCategory::FormattingLint,
        &["cargo fmt", "rustfmt", "clippy", "lint"],
    ),
    (
        FailureCategory::SemanticTest,
        &["test failed", "assertion failed", "panicked at"],
    ),
    (
        FailureCategory::CompilationType,
        &["could not compile", "error[e", "mismatched types", "rustc"],
    ),
    (
        FailureCategory::DependencyToolchain,
        &[
            "failed to download",
            "crates.io",
            "toolchain",
            "lock file needs to be updated",
        ],
    ),
    (
        FailureCategory::Infrastructure,
        &[
            "network timeout",
            "connection refused",
            "service unavailable",
            "rate limit",
            "runner unavailable",
        ],
    ),
];

fn unknown() -> FailureClassification {
    FailureClassification {
        category: FailureCategory::Unknown,
        matched_marker: None,
    }
}

/// Observation configuration, process, or protocol failure.
#[derive(Debug)]
pub enum CiObservationError {
    /// Caller-supplied monitor configuration was invalid.
    InvalidConfiguration(&'static str),
    /// Caller-supplied request data was invalid.
    InvalidRequest(&'static str),
    /// The supplied working directory was invalid.
    InvalidDirectory(PathBuf),
    /// The provider command exceeded its deadline.
    TimedOut,
    /// The provider command exceeded its combined output limit.
    OutputLimitExceeded,
    /// The provider command exited unsuccessfully.
    CommandFailed(Option<i32>),
    /// Protocol output was not valid UTF-8.
    InvalidUtf8,
    /// Protocol output was malformed or unsafe.
    InvalidProtocol(&'static str),
    /// No provider run had the requested exact SHA.
    ExactRunNotFound,
    /// More than one provider run had the requested exact SHA.
    AmbiguousExactRun,
    /// Process or filesystem I/O failed.
    Io(io::Error),
    /// A required child pipe was unavailable.
    MissingPipe,
    /// Output capture synchronization failed.
    CapturePoisoned,
    /// An output-draining worker panicked.
    WorkerPanicked,
}

impl fmt::Display for CiObservationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidConfiguration(message) => {
                write!(f, "invalid CI monitor configuration: {message}")
            }
            Self::InvalidRequest(message) => write!(f, "invalid CI observation request: {message}"),
            Self::InvalidDirectory(path) => {
                write!(f, "invalid CI observation directory: {}", path.display())
            }
            Self::TimedOut => f.write_str("CI observation command timed out"),
            Self::OutputLimitExceeded => {
                f.write_str("CI observation command exceeded output limit")
            }
            Self::CommandFailed(code) => {
                write!(f, "CI observation command failed with exit code {code:?}")
            }
            Self::InvalidUtf8 => f.write_str("CI observation protocol was not valid UTF-8"),
            Self::InvalidProtocol(message) => {
                write!(f, "invalid CI observation protocol: {message}")
            }
            Self::ExactRunNotFound => f.write_str("no CI run matched the requested exact SHA"),
            Self::AmbiguousExactRun => {
                f.write_str("multiple CI runs matched the requested exact SHA")
            }
            Self::Io(error) => write!(f, "CI observation I/O error: {error}"),
            Self::MissingPipe => {
                f.write_str("CI observation command did not provide a required pipe")
            }
            Self::CapturePoisoned => f.write_str("CI observation capture mutex poisoned"),
            Self::WorkerPanicked => f.write_str("CI observation output worker panicked"),
        }
    }
}

impl std::error::Error for CiObservationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        if let Self::Io(error) = self {
            Some(error)
        } else {
            None
        }
    }
}

#[derive(Default)]
struct Capture {
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

fn run_command(
    configuration: &CommandCiMonitorConfig,
    request: &CiObservationRequest,
    requested_sha: &str,
    directory: &Path,
) -> Result<Vec<u8>, CiObservationError> {
    let mut child = Command::new(&configuration.executable)
        .current_dir(directory)
        .args(&configuration.arguments)
        .env_clear()
        .envs(&configuration.environment)
        .env("AGENTFORGE_CI_REPOSITORY", &request.repository)
        .env("AGENTFORGE_CI_WORKFLOW", &request.workflow)
        .env("AGENTFORGE_CI_SHA", requested_sha)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(CiObservationError::Io)?;
    let stdout = child.stdout.take().ok_or(CiObservationError::MissingPipe)?;
    let stderr = child.stderr.take().ok_or(CiObservationError::MissingPipe)?;
    let capture = Arc::new(Mutex::new(Capture::default()));
    let limited = Arc::new(AtomicBool::new(false));
    let stdout_worker = drain(
        stdout,
        Arc::clone(&capture),
        Arc::clone(&limited),
        configuration.max_output_bytes,
        true,
    );
    let stderr_worker = drain(
        stderr,
        Arc::clone(&capture),
        Arc::clone(&limited),
        configuration.max_output_bytes,
        false,
    );
    let start = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Ok(status),
            Ok(None)
                if limited.load(Ordering::Acquire) || start.elapsed() >= configuration.timeout =>
            {
                let limited_before_kill = limited.load(Ordering::Acquire);
                let _ = child.kill();
                let status = child.wait().map_err(CiObservationError::Io)?;
                if limited_before_kill {
                    break Err(CiObservationError::OutputLimitExceeded);
                }
                let _ = status;
                break Err(CiObservationError::TimedOut);
            }
            Ok(None) => thread::sleep(Duration::from_millis(5)),
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                break Err(CiObservationError::Io(error));
            }
        }
    };
    stdout_worker
        .join()
        .map_err(|_| CiObservationError::WorkerPanicked)??;
    stderr_worker
        .join()
        .map_err(|_| CiObservationError::WorkerPanicked)??;
    if limited.load(Ordering::Acquire) {
        return Err(CiObservationError::OutputLimitExceeded);
    }
    let status = status?;
    if !status.success() {
        return Err(CiObservationError::CommandFailed(status.code()));
    }
    let capture = capture
        .lock()
        .map_err(|_| CiObservationError::CapturePoisoned)?;
    Ok(capture.stdout.clone())
}

fn drain<R: Read + Send + 'static>(
    mut reader: R,
    capture: Arc<Mutex<Capture>>,
    limited: Arc<AtomicBool>,
    max_output_bytes: usize,
    stdout: bool,
) -> thread::JoinHandle<Result<(), CiObservationError>> {
    thread::spawn(move || {
        let mut buffer = [0_u8; 4096];
        loop {
            let count = reader.read(&mut buffer).map_err(CiObservationError::Io)?;
            if count == 0 {
                return Ok(());
            }
            let mut capture = capture
                .lock()
                .map_err(|_| CiObservationError::CapturePoisoned)?;
            let used = capture.stdout.len() + capture.stderr.len();
            let take = count.min(max_output_bytes.saturating_sub(used));
            if stdout {
                capture.stdout.extend_from_slice(&buffer[..take]);
            } else {
                capture.stderr.extend_from_slice(&buffer[..take]);
            }
            if take < count {
                limited.store(true, Ordering::Release);
            }
        }
    })
}

fn parse_protocol(output: &[u8]) -> Result<Vec<CiRun>, CiObservationError> {
    let output = std::str::from_utf8(output).map_err(|_| CiObservationError::InvalidUtf8)?;
    let mut lines = output.lines();
    if lines.next() != Some(PROTOCOL_HEADER) {
        return Err(CiObservationError::InvalidProtocol(
            "missing or unsupported header",
        ));
    }
    let mut runs = Vec::new();
    let mut jobs = Vec::new();
    for line in lines {
        if line.is_empty() {
            return Err(CiObservationError::InvalidProtocol("empty record"));
        }
        let fields: Vec<&str> = line.split('\t').collect();
        match fields.first().copied() {
            Some("run") if fields.len() == 5 => {
                validate_field(fields[1], "run identifier")?;
                let head_sha = normalize_sha(fields[2])?;
                let status = parse_status(fields[3])?;
                let conclusion = parse_conclusion(fields[4])?;
                validate_status_conclusion(status, conclusion)?;
                runs.push(CiRun {
                    provider_id: fields[1].to_owned(),
                    head_sha,
                    status,
                    conclusion,
                    jobs: Vec::new(),
                });
            }
            Some("job") if fields.len() == 7 => {
                validate_field(fields[1], "job run identifier")?;
                validate_field(fields[2], "job identifier")?;
                validate_field(fields[3], "job name")?;
                validate_excerpt(fields[6])?;
                let status = parse_status(fields[4])?;
                let conclusion = parse_conclusion(fields[5])?;
                validate_status_conclusion(status, conclusion)?;
                jobs.push((
                    fields[1],
                    CiJob {
                        provider_id: fields[2].to_owned(),
                        name: fields[3].to_owned(),
                        status,
                        conclusion,
                        failure_excerpt: fields[6].to_owned(),
                    },
                ));
            }
            _ => return Err(CiObservationError::InvalidProtocol("invalid record shape")),
        }
    }
    for (run_id, job) in jobs {
        let Some(run) = runs.iter_mut().find(|run| run.provider_id == run_id) else {
            return Err(CiObservationError::InvalidProtocol(
                "job references unknown run",
            ));
        };
        run.jobs.push(job);
    }
    Ok(runs)
}

fn select_exact_run(runs: Vec<CiRun>, requested_sha: &str) -> Result<CiRun, CiObservationError> {
    let mut exact = runs.into_iter().filter(|run| run.head_sha == requested_sha);
    let Some(run) = exact.next() else {
        return Err(CiObservationError::ExactRunNotFound);
    };
    if exact.next().is_some() {
        return Err(CiObservationError::AmbiguousExactRun);
    }
    Ok(run)
}

fn validate_field(value: &str, name: &'static str) -> Result<(), CiObservationError> {
    if value.is_empty() || value.chars().any(char::is_control) {
        return Err(CiObservationError::InvalidRequest(match name {
            "repository" => "repository must be nonempty and contain no controls",
            "workflow" => "workflow must be nonempty and contain no controls",
            _ => "protocol field must be nonempty and contain no controls",
        }));
    }
    Ok(())
}

fn validate_excerpt(value: &str) -> Result<(), CiObservationError> {
    if value.chars().any(char::is_control) {
        return Err(CiObservationError::InvalidProtocol(
            "failure excerpt contains a control character",
        ));
    }
    Ok(())
}

fn normalize_sha(value: &str) -> Result<String, CiObservationError> {
    if value.len() != 40 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(CiObservationError::InvalidRequest(
            "commit SHA must contain exactly 40 hexadecimal characters",
        ));
    }
    Ok(value.to_ascii_lowercase())
}

fn parse_status(value: &str) -> Result<CiStatus, CiObservationError> {
    match value {
        "queued" => Ok(CiStatus::Queued),
        "in_progress" => Ok(CiStatus::InProgress),
        "completed" => Ok(CiStatus::Completed),
        _ => Err(CiObservationError::InvalidProtocol("unknown status")),
    }
}

fn parse_conclusion(value: &str) -> Result<Option<CiConclusion>, CiObservationError> {
    match value {
        "-" => Ok(None),
        "success" => Ok(Some(CiConclusion::Success)),
        "failure" => Ok(Some(CiConclusion::Failure)),
        "cancelled" => Ok(Some(CiConclusion::Cancelled)),
        "skipped" => Ok(Some(CiConclusion::Skipped)),
        "timed_out" => Ok(Some(CiConclusion::TimedOut)),
        "action_required" => Ok(Some(CiConclusion::ActionRequired)),
        "neutral" => Ok(Some(CiConclusion::Neutral)),
        _ => Err(CiObservationError::InvalidProtocol("unknown conclusion")),
    }
}

fn validate_status_conclusion(
    status: CiStatus,
    conclusion: Option<CiConclusion>,
) -> Result<(), CiObservationError> {
    match (status, conclusion) {
        (CiStatus::Completed, Some(_)) | (CiStatus::Queued | CiStatus::InProgress, None) => Ok(()),
        _ => Err(CiObservationError::InvalidProtocol(
            "status and conclusion disagree",
        )),
    }
}
