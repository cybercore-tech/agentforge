//! Provider-neutral execution adapters for verified AgentForge task worktrees.
//!
//! Process evidence is intentionally separate from task acceptance. A successful child exit does
//! not change task state or establish that an agent completed its assigned work.

use agentforge_core::agent::{
    AGENT_CONTRACT_VERSION, AgentResult, AgentTask, ApprovalBoundary, Capability, TaskContractError,
};
use agentforge_core::task::{TaskId, TaskIdError};
use agentforge_worktree::{WorktreeError, WorktreeManager, WorktreeStatus};
use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

/// A request to execute an agent task.
#[derive(Clone, Copy, Debug)]
pub struct AdapterRequest<'a> {
    /// The semantic task contract supplied by the orchestrator.
    pub task: &'a AgentTask,
    /// Manager for the repository that owns the task worktree.
    pub worktrees: &'a WorktreeManager,
    /// Human approval boundaries acknowledged by the caller before launch.
    pub acknowledged_approvals: &'a [ApprovalBoundary],
}

/// Stable provider-neutral agent execution interface.
pub trait AgentAdapter {
    /// Returns the adapter's stable operator-facing identity.
    fn id(&self) -> &str;

    /// Executes one preflight-validated task.
    ///
    /// # Errors
    ///
    /// Returns [`AdapterError`] when request validation, worktree inspection, process spawning,
    /// or process supervision fails.
    fn execute(&self, request: AdapterRequest<'_>) -> Result<ExecutionReport, AdapterError>;
}

/// Explicit configuration for a foreground local-process adapter.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProcessAdapterConfig {
    adapter_id: String,
    executable: PathBuf,
    arguments: Vec<String>,
    environment: BTreeMap<String, String>,
    timeout: Duration,
    max_output_bytes: usize,
    interactive: bool,
}

/// Project-local directory containing versioned agent profiles.
pub const AGENT_PROFILE_RELATIVE_PATH: &str = ".forge/agents";

const MAX_PROFILE_BYTES: u64 = 64 * 1024;
const MAX_PROFILE_ARGUMENTS: usize = 64;
const MAX_PROFILE_ENVIRONMENT: usize = 64;
const MAX_PROFILE_TIMEOUT_MS: u64 = 24 * 60 * 60 * 1000;
const MAX_PROFILE_OUTPUT_BYTES: usize = 64 * 1024 * 1024;

/// A bounded, project-local process profile for an agent.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgentProfile {
    id: String,
    config: ProcessAdapterConfig,
}

impl AgentProfile {
    /// Returns the stable profile identifier.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Returns the configured executable.
    #[must_use]
    pub fn executable(&self) -> &Path {
        &self.config.executable
    }

    /// Returns literal child-process arguments.
    #[must_use]
    pub fn arguments(&self) -> &[String] {
        &self.config.arguments
    }

    /// Returns explicit child-process environment values.
    #[must_use]
    pub fn environment(&self) -> &BTreeMap<String, String> {
        &self.config.environment
    }

    /// Returns the configured execution deadline.
    #[must_use]
    pub fn timeout(&self) -> Duration {
        self.config.timeout
    }

    /// Returns the configured output limit.
    #[must_use]
    pub fn max_output_bytes(&self) -> usize {
        self.config.max_output_bytes
    }

    /// Converts this profile into an executable adapter configuration.
    #[must_use]
    pub fn adapter_config(&self) -> ProcessAdapterConfig {
        self.config.clone()
    }
}

/// Loads project-local agent profiles from .forge/agents.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgentProfileStore {
    project_root: PathBuf,
}

impl AgentProfileStore {
    /// Creates a profile store rooted at a project directory.
    #[must_use]
    pub fn new(project_root: impl Into<PathBuf>) -> Self {
        Self {
            project_root: project_root.into(),
        }
    }

    /// Returns the profile directory.
    #[must_use]
    pub fn directory(&self) -> PathBuf {
        self.project_root.join(AGENT_PROFILE_RELATIVE_PATH)
    }

    /// Loads one named profile.
    ///
    /// Profiles are UTF-8 key=value documents. Supported keys are version, executable, repeated
    /// argument, repeated env.<KEY>, timeout_ms, and max_output_bytes.
    pub fn load(&self, id: &str) -> Result<AgentProfile, AdapterError> {
        validate_profile_id(id)?;
        let path = self.directory().join(format!("{id}.conf"));
        let metadata = fs::symlink_metadata(&path).map_err(|error| {
            if error.kind() == io::ErrorKind::NotFound {
                AdapterError::Profile(format!("profile not found: {id}"))
            } else {
                AdapterError::Io(error)
            }
        })?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(AdapterError::Profile(format!(
                "profile path is not a regular file: {}",
                path.display()
            )));
        }
        let metadata = fs::metadata(&path)?;
        if metadata.len() > MAX_PROFILE_BYTES {
            return Err(AdapterError::Profile(format!(
                "profile exceeds {MAX_PROFILE_BYTES} bytes: {id}"
            )));
        }
        let text = fs::read_to_string(&path)?;
        parse_profile(id, &text)
    }

    /// Lists all valid profiles in deterministic identifier order.
    pub fn list(&self) -> Result<Vec<AgentProfile>, AdapterError> {
        let directory = self.directory();
        let entries = match fs::read_dir(&directory) {
            Ok(entries) => entries,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(AdapterError::Io(error)),
        };
        let mut ids = Vec::new();
        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|value| value.to_str()) != Some("conf") {
                continue;
            }
            let Some(id) = path.file_stem().and_then(|value| value.to_str()) else {
                return Err(AdapterError::Profile(format!(
                    "profile filename is not UTF-8: {}",
                    path.display()
                )));
            };
            ids.push(id.to_owned());
        }
        ids.sort();
        ids.into_iter().map(|id| self.load(&id)).collect()
    }
}

fn validate_profile_id(id: &str) -> Result<(), AdapterError> {
    if id.is_empty() || id == "." || id == ".." || id.len() > 64 {
        return Err(AdapterError::Profile(format!(
            "profile ID must be 1-64 characters: {id:?}"
        )));
    }
    if !id
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    {
        return Err(AdapterError::Profile(format!(
            "profile ID contains unsupported characters: {id}"
        )));
    }
    Ok(())
}

fn parse_profile(id: &str, text: &str) -> Result<AgentProfile, AdapterError> {
    let mut version = None;
    let mut executable = None;
    let mut arguments = Vec::new();
    let mut environment = BTreeMap::new();
    let mut timeout_ms = None;
    let mut max_output_bytes = None;

    for (line_index, line) in text.lines().enumerate() {
        let line_number = line_index + 1;
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            return Err(AdapterError::Profile(format!(
                "profile {id} line {line_number} must use key=value"
            )));
        };
        if value.contains('\0') {
            return Err(AdapterError::Profile(format!(
                "profile {id} line {line_number} contains NUL"
            )));
        }
        match key {
            "version" => {
                let parsed = value.parse::<u32>().map_err(|_| {
                    AdapterError::Profile(format!(
                        "profile {id} line {line_number} has invalid version"
                    ))
                })?;
                set_once(&mut version, parsed, id, line_number, "version")?;
            }
            "executable" => {
                set_once(
                    &mut executable,
                    value.to_owned(),
                    id,
                    line_number,
                    "executable",
                )?;
            }
            "argument" => {
                if arguments.len() == MAX_PROFILE_ARGUMENTS {
                    return Err(AdapterError::Profile(format!(
                        "profile {id} has more than {MAX_PROFILE_ARGUMENTS} arguments"
                    )));
                }
                arguments.push(value.to_owned());
            }
            "timeout_ms" => {
                let parsed = value.parse::<u64>().map_err(|_| {
                    AdapterError::Profile(format!(
                        "profile {id} line {line_number} has invalid timeout_ms"
                    ))
                })?;
                if parsed == 0 || parsed > MAX_PROFILE_TIMEOUT_MS {
                    return Err(AdapterError::Profile(format!(
                        "profile {id} line {line_number} timeout_ms is outside 1..={MAX_PROFILE_TIMEOUT_MS}"
                    )));
                }
                set_once(&mut timeout_ms, parsed, id, line_number, "timeout_ms")?;
            }
            "max_output_bytes" => {
                let parsed = value.parse::<usize>().map_err(|_| {
                    AdapterError::Profile(format!(
                        "profile {id} line {line_number} has invalid max_output_bytes"
                    ))
                })?;
                if parsed == 0 || parsed > MAX_PROFILE_OUTPUT_BYTES {
                    return Err(AdapterError::Profile(format!(
                        "profile {id} line {line_number} max_output_bytes is outside 1..={MAX_PROFILE_OUTPUT_BYTES}"
                    )));
                }
                set_once(
                    &mut max_output_bytes,
                    parsed,
                    id,
                    line_number,
                    "max_output_bytes",
                )?;
            }
            key if key.starts_with("env.") => {
                let env_key = &key["env.".len()..];
                if env_key.is_empty() || env_key.contains('=') || env_key.contains('\0') {
                    return Err(AdapterError::Profile(format!(
                        "profile {id} line {line_number} has invalid environment key"
                    )));
                }
                if environment.len() == MAX_PROFILE_ENVIRONMENT
                    && !environment.contains_key(env_key)
                {
                    return Err(AdapterError::Profile(format!(
                        "profile {id} has more than {MAX_PROFILE_ENVIRONMENT} environment values"
                    )));
                }
                if environment
                    .insert(env_key.to_owned(), value.to_owned())
                    .is_some()
                {
                    return Err(AdapterError::Profile(format!(
                        "profile {id} repeats environment key {env_key}"
                    )));
                }
            }
            _ => {
                return Err(AdapterError::Profile(format!(
                    "profile {id} line {line_number} has unknown key {key}"
                )));
            }
        }
    }

    if version != Some(1) {
        return Err(AdapterError::Profile(format!(
            "profile {id} requires version=1"
        )));
    }
    let executable = executable
        .ok_or_else(|| AdapterError::Profile(format!("profile {id} is missing executable")))?;
    let executable_path = Path::new(&executable);
    if !executable_path.is_absolute() {
        return Err(AdapterError::Profile(format!(
            "profile {id} executable path must be absolute"
        )));
    }
    if !executable_path.is_file() {
        return Err(AdapterError::Profile(format!(
            "profile {id} executable does not exist or is not a file: {}",
            executable_path.display()
        )));
    }
    let timeout = Duration::from_millis(timeout_ms.unwrap_or(60_000));
    let max_output_bytes = max_output_bytes.unwrap_or(1024 * 1024);
    let mut config = ProcessAdapterConfig::new(id, executable)
        .with_timeout(timeout)
        .with_max_output_bytes(max_output_bytes);
    for argument in arguments {
        config = config.with_argument(argument);
    }
    for (key, value) in environment {
        config = config.with_environment(key, value);
    }
    ProcessAdapter::new(config.clone())?;
    Ok(AgentProfile {
        id: id.to_owned(),
        config,
    })
}

fn set_once<T>(
    slot: &mut Option<T>,
    value: T,
    id: &str,
    line_number: usize,
    key: &str,
) -> Result<(), AdapterError> {
    if slot.is_some() {
        return Err(AdapterError::Profile(format!(
            "profile {id} line {line_number} repeats {key}"
        )));
    }
    *slot = Some(value);
    Ok(())
}

impl ProcessAdapterConfig {
    /// Creates a process configuration with conservative default limits.
    #[must_use]
    pub fn new(adapter_id: impl Into<String>, executable: impl Into<PathBuf>) -> Self {
        Self {
            adapter_id: adapter_id.into(),
            executable: executable.into(),
            arguments: Vec::new(),
            environment: BTreeMap::new(),
            timeout: Duration::from_secs(60),
            max_output_bytes: 1024 * 1024,
            interactive: false,
        }
    }

    /// Adds one literal child-process argument.
    #[must_use]
    pub fn with_argument(mut self, argument: impl Into<String>) -> Self {
        self.arguments.push(argument.into());
        self
    }

    /// Adds one explicit environment value for the child process.
    #[must_use]
    pub fn with_environment(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.environment.insert(key.into(), value.into());
        self
    }

    /// Sets the complete execution deadline.
    #[must_use]
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Sets the shared stdout/stderr byte limit.
    #[must_use]
    pub fn with_max_output_bytes(mut self, max_output_bytes: usize) -> Self {
        self.max_output_bytes = max_output_bytes;
        self
    }

    /// Enables cooked, line-oriented terminal interaction for this direct foreground adapter.
    #[must_use]
    pub fn with_interactive(mut self) -> Self {
        self.interactive = true;
        self
    }

    fn validate(&self) -> Result<(), AdapterError> {
        if self.adapter_id.trim().is_empty() {
            return Err(AdapterError::InvalidConfiguration(
                "adapter ID must not be empty",
            ));
        }
        if !self.executable.is_absolute() {
            return Err(AdapterError::InvalidConfiguration(
                "adapter executable path must be absolute",
            ));
        }
        if self.timeout.is_zero() {
            return Err(AdapterError::InvalidConfiguration(
                "adapter timeout must be nonzero",
            ));
        }
        if self.max_output_bytes == 0 {
            return Err(AdapterError::InvalidConfiguration(
                "adapter output limit must be nonzero",
            ));
        }
        if self
            .environment
            .keys()
            .any(|key| key.is_empty() || key.contains('='))
        {
            return Err(AdapterError::InvalidConfiguration(
                "adapter environment keys must be nonempty and contain no equals sign",
            ));
        }
        Ok(())
    }
}

/// Foreground executable implementation of [`AgentAdapter`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProcessAdapter {
    config: ProcessAdapterConfig,
}

impl ProcessAdapter {
    /// Creates a local-process adapter from explicit configuration.
    ///
    /// # Errors
    ///
    /// Returns [`AdapterError`] if the configuration is invalid.
    pub fn new(config: ProcessAdapterConfig) -> Result<Self, AdapterError> {
        config.validate()?;
        Ok(Self { config })
    }
}

impl AgentAdapter for ProcessAdapter {
    fn id(&self) -> &str {
        &self.config.adapter_id
    }

    fn execute(&self, request: AdapterRequest<'_>) -> Result<ExecutionReport, AdapterError> {
        let status = preflight(request)?;
        let prompt = render_task_prompt(request.task);
        if self.config.interactive {
            execute_interactive_process(&self.config, request.task, status, prompt)
        } else {
            execute_process(&self.config, request.task, status, prompt)
        }
    }
}

/// How a supervised child process terminated.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExecutionTermination {
    /// The child exited before a configured limit was reached.
    Exited,
    /// The adapter killed the direct child after its deadline elapsed.
    TimedOut,
    /// The adapter killed the direct child after combined output reached its byte limit.
    OutputLimitExceeded,
}

/// Bounded evidence from one adapter process execution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExecutionReport {
    task_id: TaskId,
    adapter_id: String,
    worktree_path: PathBuf,
    worktree_head: String,
    termination: ExecutionTermination,
    exit_code: Option<i32>,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    output_truncated: bool,
}

impl ExecutionReport {
    /// Returns the task whose verified worktree was used.
    #[must_use]
    pub fn task_id(&self) -> &TaskId {
        &self.task_id
    }

    /// Returns the executing adapter identity.
    #[must_use]
    pub fn adapter_id(&self) -> &str {
        &self.adapter_id
    }

    /// Returns the verified worktree path at launch.
    #[must_use]
    pub fn worktree_path(&self) -> &Path {
        &self.worktree_path
    }

    /// Returns the exact worktree HEAD recorded at launch.
    #[must_use]
    pub fn worktree_head(&self) -> &str {
        &self.worktree_head
    }

    /// Returns the direct child termination reason.
    #[must_use]
    pub const fn termination(&self) -> ExecutionTermination {
        self.termination
    }

    /// Returns the child exit code when the platform provided one.
    #[must_use]
    pub const fn exit_code(&self) -> Option<i32> {
        self.exit_code
    }

    /// Returns captured raw standard output.
    #[must_use]
    pub fn stdout(&self) -> &[u8] {
        &self.stdout
    }

    /// Returns captured raw standard error.
    #[must_use]
    pub fn stderr(&self) -> &[u8] {
        &self.stderr
    }

    /// Returns whether the shared output limit discarded bytes.
    #[must_use]
    pub const fn output_truncated(&self) -> bool {
        self.output_truncated
    }
}

/// Adapter input or process-supervision failure.
#[derive(Debug)]
pub enum AdapterError {
    /// Process configuration was invalid.
    InvalidConfiguration(&'static str),
    /// A project-local agent profile is missing or malformed.
    Profile(String),
    /// The task contract is structurally invalid.
    InvalidTask(TaskContractError),
    /// Task ID parsing failed.
    InvalidTaskId(TaskIdError),
    /// Task ID does not match its declared milestone.
    TaskMilestoneMismatch {
        /// The task ID supplied by the caller.
        task_id: String,
        /// The milestone declared by the task contract.
        milestone_id: String,
    },
    /// The task lacks the local-command capability required to spawn a child.
    MissingCapability(Capability),
    /// The caller did not acknowledge one declared human approval boundary.
    MissingApproval(ApprovalBoundary),
    /// Worktree inspection failed.
    Worktree(WorktreeError),
    /// The expected managed worktree is absent.
    WorktreeNotFound(TaskId),
    /// The managed worktree is dirty.
    DirtyWorktree(TaskId),
    /// The managed worktree has a pending Git operation.
    UnresolvedGitOperation(TaskId),
    /// Process I/O or synchronization failed.
    Io(io::Error),
    /// An internal output-capture mutex was poisoned.
    CapturePoisoned,
    /// An output-draining or stdin-delivery thread panicked.
    WorkerThreadPanicked,
}

impl fmt::Display for AdapterError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidConfiguration(message) => {
                write!(formatter, "invalid adapter configuration: {message}")
            }
            Self::Profile(message) => write!(formatter, "invalid agent profile: {message}"),
            Self::InvalidTask(error) => write!(formatter, "invalid agent task: {error}"),
            Self::InvalidTaskId(error) => write!(formatter, "invalid agent task ID: {error}"),
            Self::TaskMilestoneMismatch {
                task_id,
                milestone_id,
            } => write!(
                formatter,
                "task ID {task_id} does not match milestone {milestone_id}"
            ),
            Self::MissingCapability(capability) => {
                write!(formatter, "task lacks required capability: {capability}")
            }
            Self::MissingApproval(approval) => write!(
                formatter,
                "task approval has not been acknowledged: {}",
                approval.as_str()
            ),
            Self::Worktree(error) => write!(formatter, "worktree inspection failed: {error}"),
            Self::WorktreeNotFound(task_id) => {
                write!(formatter, "managed worktree not found for {task_id}")
            }
            Self::DirtyWorktree(task_id) => {
                write!(formatter, "managed worktree is dirty: {task_id}")
            }
            Self::UnresolvedGitOperation(task_id) => write!(
                formatter,
                "managed worktree has an unresolved Git operation: {task_id}"
            ),
            Self::Io(error) => write!(formatter, "adapter process I/O failed: {error}"),
            Self::CapturePoisoned => {
                formatter.write_str("adapter output capture mutex was poisoned")
            }
            Self::WorkerThreadPanicked => {
                formatter.write_str("adapter process worker thread panicked")
            }
        }
    }
}

impl std::error::Error for AdapterError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidTask(error) => Some(error),
            Self::InvalidTaskId(error) => Some(error),
            Self::Worktree(error) => Some(error),
            Self::Io(error) => Some(error),
            _ => None,
        }
    }
}

impl From<io::Error> for AdapterError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

/// Renders all task-contract fields into the version-one adapter stdin document.
#[must_use]
pub fn render_task_prompt(task: &AgentTask) -> Vec<u8> {
    let mut output = Vec::new();
    output.extend_from_slice(b"agentforge-task-prompt-v1\n");
    write_field(
        &mut output,
        "contract_version",
        &task.contract_version.to_string(),
    );
    write_field(&mut output, "task_id", &task.task_id);
    write_field(&mut output, "milestone_id", &task.milestone_id);
    write_field(&mut output, "primary_role", task.primary_role.as_str());
    write_field(&mut output, "goal", &task.goal);
    write_list(&mut output, "non_goals", &task.non_goals);
    write_list(
        &mut output,
        "dependency_task_ids",
        &task.dependency_task_ids,
    );
    write_list(&mut output, "allowed_paths", &task.allowed_paths);
    write_list(&mut output, "forbidden_paths", &task.forbidden_paths);
    write_values(
        &mut output,
        "capabilities",
        task.capabilities.iter().map(|value| value.as_str()),
    );
    write_values(
        &mut output,
        "required_approvals",
        task.required_approvals.iter().map(|value| value.as_str()),
    );
    write_list(&mut output, "required_gates", &task.required_gates);
    write_list(&mut output, "expected_outputs", &task.expected_outputs);
    write_list(
        &mut output,
        "evidence_requirements",
        &task.evidence_requirements,
    );
    output
}

/// Validates a separately supplied result is bound to the task but does not accept it.
///
/// # Errors
///
/// Returns [`AdapterError::InvalidTask`] for an invalid task and a binding error otherwise.
pub fn validate_result_binding(task: &AgentTask, result: &AgentResult) -> Result<(), AdapterError> {
    task.validate().map_err(AdapterError::InvalidTask)?;
    if result.contract_version != AGENT_CONTRACT_VERSION || !result.belongs_to(task) {
        return Err(AdapterError::InvalidConfiguration(
            "agent result is not bound to the submitted task contract",
        ));
    }
    Ok(())
}

fn preflight(request: AdapterRequest<'_>) -> Result<WorktreeStatus, AdapterError> {
    request.task.validate().map_err(AdapterError::InvalidTask)?;
    let task_id =
        TaskId::parse(request.task.task_id.clone()).map_err(AdapterError::InvalidTaskId)?;
    let expected_prefix = format!("{}-T", request.task.milestone_id);
    let Some(sequence) = task_id.as_str().strip_prefix(&expected_prefix) else {
        return Err(AdapterError::TaskMilestoneMismatch {
            task_id: request.task.task_id.clone(),
            milestone_id: request.task.milestone_id.clone(),
        });
    };
    if sequence
        .parse::<u32>()
        .ok()
        .filter(|value| *value > 0)
        .is_none()
    {
        return Err(AdapterError::TaskMilestoneMismatch {
            task_id: request.task.task_id.clone(),
            milestone_id: request.task.milestone_id.clone(),
        });
    }
    if !request.task.has_capability(Capability::RunLocalCommands) {
        return Err(AdapterError::MissingCapability(
            Capability::RunLocalCommands,
        ));
    }
    for approval in &request.task.required_approvals {
        if !request.acknowledged_approvals.contains(approval) {
            return Err(AdapterError::MissingApproval(*approval));
        }
    }
    let status = request
        .worktrees
        .inspect(&task_id)
        .map_err(AdapterError::Worktree)?
        .ok_or_else(|| AdapterError::WorktreeNotFound(task_id.clone()))?;
    if status.is_dirty() {
        return Err(AdapterError::DirtyWorktree(task_id));
    }
    if status.operation().is_some() {
        return Err(AdapterError::UnresolvedGitOperation(task_id));
    }
    Ok(status)
}

fn execute_process(
    config: &ProcessAdapterConfig,
    task: &AgentTask,
    status: WorktreeStatus,
    prompt: Vec<u8>,
) -> Result<ExecutionReport, AdapterError> {
    let mut child = Command::new(&config.executable)
        .current_dir(status.path())
        .args(&config.arguments)
        .env_clear()
        .envs(&config.environment)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let stdin = child
        .stdin
        .take()
        .ok_or(AdapterError::InvalidConfiguration(
            "child stdin was not piped",
        ))?;
    let stdout = child
        .stdout
        .take()
        .ok_or(AdapterError::InvalidConfiguration(
            "child stdout was not piped",
        ))?;
    let stderr = child
        .stderr
        .take()
        .ok_or(AdapterError::InvalidConfiguration(
            "child stderr was not piped",
        ))?;
    let capture = Arc::new(Mutex::new(Capture::default()));
    let output_limited = Arc::new(AtomicBool::new(false));
    let stdout_thread = drain(
        stdout,
        Arc::clone(&capture),
        Arc::clone(&output_limited),
        config.max_output_bytes,
        Stream::Stdout,
    );
    let stderr_thread = drain(
        stderr,
        Arc::clone(&capture),
        Arc::clone(&output_limited),
        config.max_output_bytes,
        Stream::Stderr,
    );
    let stdin_thread = thread::spawn(move || {
        let mut stdin = stdin;
        let _ = stdin.write_all(&prompt);
    });
    let (termination, exit_code) = wait_for_child(&mut child, config.timeout, &output_limited)?;
    stdin_thread
        .join()
        .map_err(|_| AdapterError::WorkerThreadPanicked)?;
    stdout_thread
        .join()
        .map_err(|_| AdapterError::WorkerThreadPanicked)??;
    stderr_thread
        .join()
        .map_err(|_| AdapterError::WorkerThreadPanicked)??;
    let capture = capture.lock().map_err(|_| AdapterError::CapturePoisoned)?;
    Ok(ExecutionReport {
        task_id: TaskId::parse(task.task_id.clone()).map_err(AdapterError::InvalidTaskId)?,
        adapter_id: config.adapter_id.clone(),
        worktree_path: status.path().to_path_buf(),
        worktree_head: status.head().to_owned(),
        termination,
        exit_code,
        stdout: capture.stdout.clone(),
        stderr: capture.stderr.clone(),
        output_truncated: output_limited.load(Ordering::Acquire),
    })
}

fn execute_interactive_process(
    config: &ProcessAdapterConfig,
    task: &AgentTask,
    status: WorktreeStatus,
    prompt: Vec<u8>,
) -> Result<ExecutionReport, AdapterError> {
    let mut child = Command::new(&config.executable)
        .current_dir(status.path())
        .args(&config.arguments)
        .env_clear()
        .envs(&config.environment)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let child_stdin = child
        .stdin
        .take()
        .ok_or(AdapterError::InvalidConfiguration(
            "child stdin was not piped",
        ))?;
    let stdout = child
        .stdout
        .take()
        .ok_or(AdapterError::InvalidConfiguration(
            "child stdout was not piped",
        ))?;
    let stderr = child
        .stderr
        .take()
        .ok_or(AdapterError::InvalidConfiguration(
            "child stderr was not piped",
        ))?;
    let capture = Arc::new(Mutex::new(Capture::default()));
    let output_limited = Arc::new(AtomicBool::new(false));
    let stdout_thread = drain_interactive(
        stdout,
        Arc::clone(&capture),
        Arc::clone(&output_limited),
        config.max_output_bytes,
        Stream::Stdout,
    );
    let stderr_thread = drain_interactive(
        stderr,
        Arc::clone(&capture),
        Arc::clone(&output_limited),
        config.max_output_bytes,
        Stream::Stderr,
    );
    let input_thread = thread::spawn(move || {
        let mut child_stdin = child_stdin;
        child_stdin.write_all(&prompt)?;
        child_stdin.flush()?;
        let stdin = io::stdin();
        let mut stdin = stdin.lock();
        let mut buffer = [0_u8; 4096];
        loop {
            let length = stdin.read(&mut buffer)?;
            if length == 0 {
                return Ok::<(), io::Error>(());
            }
            child_stdin.write_all(&buffer[..length])?;
            child_stdin.flush()?;
        }
    });
    let (termination, exit_code) = wait_for_child(&mut child, config.timeout, &output_limited)?;
    if input_thread.is_finished() {
        input_thread
            .join()
            .map_err(|_| AdapterError::WorkerThreadPanicked)??;
    }
    stdout_thread
        .join()
        .map_err(|_| AdapterError::WorkerThreadPanicked)??;
    stderr_thread
        .join()
        .map_err(|_| AdapterError::WorkerThreadPanicked)??;
    let capture = capture.lock().map_err(|_| AdapterError::CapturePoisoned)?;
    Ok(ExecutionReport {
        task_id: TaskId::parse(task.task_id.clone()).map_err(AdapterError::InvalidTaskId)?,
        adapter_id: config.adapter_id.clone(),
        worktree_path: status.path().to_path_buf(),
        worktree_head: status.head().to_owned(),
        termination,
        exit_code,
        stdout: capture.stdout.clone(),
        stderr: capture.stderr.clone(),
        output_truncated: output_limited.load(Ordering::Acquire),
    })
}

fn wait_for_child(
    child: &mut Child,
    timeout: Duration,
    output_limited: &AtomicBool,
) -> Result<(ExecutionTermination, Option<i32>), AdapterError> {
    let start = Instant::now();
    loop {
        if let Some(status) = child.try_wait()? {
            return Ok((ExecutionTermination::Exited, status.code()));
        }
        let termination = if output_limited.load(Ordering::Acquire) {
            Some(ExecutionTermination::OutputLimitExceeded)
        } else if start.elapsed() >= timeout {
            Some(ExecutionTermination::TimedOut)
        } else {
            None
        };
        if let Some(termination) = termination {
            let _ = child.kill();
            let status = child.wait()?;
            return Ok((termination, status.code()));
        }
        thread::sleep(Duration::from_millis(5));
    }
}

#[derive(Default)]
struct Capture {
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

#[derive(Clone, Copy)]
enum Stream {
    Stdout,
    Stderr,
}

fn drain<R: Read + Send + 'static>(
    mut reader: R,
    capture: Arc<Mutex<Capture>>,
    output_limited: Arc<AtomicBool>,
    limit: usize,
    stream: Stream,
) -> thread::JoinHandle<Result<(), AdapterError>> {
    thread::spawn(move || {
        let mut buffer = [0_u8; 8192];
        loop {
            let length = reader.read(&mut buffer)?;
            if length == 0 {
                return Ok(());
            }
            let mut capture = capture.lock().map_err(|_| AdapterError::CapturePoisoned)?;
            let used = capture.stdout.len() + capture.stderr.len();
            let remaining = limit.saturating_sub(used);
            let accepted = length.min(remaining);
            match stream {
                Stream::Stdout => capture.stdout.extend_from_slice(&buffer[..accepted]),
                Stream::Stderr => capture.stderr.extend_from_slice(&buffer[..accepted]),
            }
            if accepted < length {
                output_limited.store(true, Ordering::Release);
            }
        }
    })
}

fn drain_interactive<R: Read + Send + 'static>(
    mut reader: R,
    capture: Arc<Mutex<Capture>>,
    output_limited: Arc<AtomicBool>,
    limit: usize,
    stream: Stream,
) -> thread::JoinHandle<Result<(), AdapterError>> {
    thread::spawn(move || {
        let mut buffer = [0_u8; 8192];
        let mut terminal: Box<dyn Write + Send> = match stream {
            Stream::Stdout => Box::new(io::stdout()),
            Stream::Stderr => Box::new(io::stderr()),
        };
        loop {
            let length = reader.read(&mut buffer)?;
            if length == 0 {
                return Ok(());
            }
            terminal.write_all(&buffer[..length])?;
            terminal.flush()?;
            let mut capture = capture.lock().map_err(|_| AdapterError::CapturePoisoned)?;
            let used = capture.stdout.len() + capture.stderr.len();
            let remaining = limit.saturating_sub(used);
            let accepted = length.min(remaining);
            match stream {
                Stream::Stdout => capture.stdout.extend_from_slice(&buffer[..accepted]),
                Stream::Stderr => capture.stderr.extend_from_slice(&buffer[..accepted]),
            }
            if accepted < length {
                output_limited.store(true, Ordering::Release);
            }
        }
    })
}

fn write_field(output: &mut Vec<u8>, name: &str, value: &str) {
    output.extend_from_slice(name.as_bytes());
    output.extend_from_slice(b" ");
    output.extend_from_slice(value.len().to_string().as_bytes());
    output.extend_from_slice(b"\n");
    output.extend_from_slice(value.as_bytes());
    output.extend_from_slice(b"\n");
}

fn write_list(output: &mut Vec<u8>, name: &str, values: &[String]) {
    write_values(output, name, values.iter().map(String::as_str));
}

fn write_values<'a>(output: &mut Vec<u8>, name: &str, values: impl Iterator<Item = &'a str>) {
    let values: Vec<&str> = values.collect();
    output.extend_from_slice(name.as_bytes());
    output.extend_from_slice(b" ");
    output.extend_from_slice(values.len().to_string().as_bytes());
    output.extend_from_slice(b"\n");
    for value in values {
        write_field(output, "item", value);
    }
}

#[cfg(test)]
mod tests {
    use super::{
        AdapterError, AdapterRequest, AgentAdapter, AgentProfileStore, ExecutionReport,
        render_task_prompt, validate_result_binding,
    };
    use agentforge_core::agent::{AgentResult, AgentRole, AgentTask, TaskOutcome};
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn prompt_preserves_multiline_unicode_contract_data() {
        let mut task = AgentTask::new(
            "P0-M006-T0001",
            "P0-M006",
            AgentRole::Implementer,
            "make café\nready",
        );
        task.non_goals.push("do not change: α".to_owned());
        let text = String::from_utf8(render_task_prompt(&task)).unwrap();
        assert!(text.starts_with("agentforge-task-prompt-v1\n"));
        assert!(text.contains("make café\nready"));
        assert!(text.contains("do not change: α"));
    }

    #[test]
    fn result_must_bind_to_the_submitted_task() {
        let task = AgentTask::new("P0-M006-T0001", "P0-M006", AgentRole::Implementer, "work");
        let result = AgentResult::new("P0-M006-T0002", TaskOutcome::Completed, "claimed");
        assert!(matches!(
            validate_result_binding(&task, &result),
            Err(AdapterError::InvalidConfiguration(_))
        ));
    }

    #[test]
    fn matching_result_remains_a_valid_claim() {
        let task = AgentTask::new("P0-M006-T0001", "P0-M006", AgentRole::Implementer, "work");
        let result = AgentResult::new("P0-M006-T0001", TaskOutcome::Completed, "claimed");
        assert!(validate_result_binding(&task, &result).is_ok());
    }

    #[test]
    fn profile_store_loads_bounded_literal_configuration() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "agentforge-profile-test-{}-{suffix}",
            std::process::id()
        ));
        let directory = root.join(".forge/agents");
        fs::create_dir_all(&directory).unwrap();
        let executable = std::env::current_exe().unwrap();
        fs::write(
            directory.join("fixture.conf"),
            format!(
                "version=1\nexecutable={}\nargument=literal value\nenv.FORGE_TEST=value=still-literal\ntimeout_ms=250\nmax_output_bytes=2048\n",
                executable.display()
            ),
        )
        .unwrap();

        let profile = AgentProfileStore::new(&root).load("fixture").unwrap();
        assert_eq!(profile.id(), "fixture");
        assert_eq!(profile.arguments(), &["literal value".to_owned()]);
        assert_eq!(
            profile.environment().get("FORGE_TEST"),
            Some(&"value=still-literal".to_owned())
        );
        assert_eq!(profile.timeout(), std::time::Duration::from_millis(250));
        assert_eq!(profile.max_output_bytes(), 2048);

        fs::remove_dir_all(root).unwrap();
    }

    struct FakeAdapter;
    impl AgentAdapter for FakeAdapter {
        fn id(&self) -> &str {
            "fake"
        }
        fn execute(&self, _request: AdapterRequest<'_>) -> Result<ExecutionReport, AdapterError> {
            Err(AdapterError::InvalidConfiguration("fixture"))
        }
    }

    #[test]
    fn trait_is_object_safe() {
        let adapter: &dyn AgentAdapter = &FakeAdapter;
        assert_eq!(adapter.id(), "fake");
    }
}
