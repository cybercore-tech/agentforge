//! Deterministic local process gates with bounded evidence.

use std::collections::{BTreeMap, HashSet};
use std::fmt;
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

/// Explicit definition of one local validation gate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GateDefinition {
    name: String,
    executable: PathBuf,
    arguments: Vec<String>,
    environment: BTreeMap<String, String>,
    timeout: Duration,
    max_output_bytes: usize,
}

impl GateDefinition {
    /// Creates a gate with a 60-second deadline and one MiB output budget.
    #[must_use]
    pub fn new(name: impl Into<String>, executable: impl Into<PathBuf>) -> Self {
        Self {
            name: name.into(),
            executable: executable.into(),
            arguments: Vec::new(),
            environment: BTreeMap::new(),
            timeout: Duration::from_secs(60),
            max_output_bytes: 1024 * 1024,
        }
    }
    /// Adds a literal argument.
    #[must_use]
    pub fn with_argument(mut self, value: impl Into<String>) -> Self {
        self.arguments.push(value.into());
        self
    }
    /// Adds an explicit environment value.
    #[must_use]
    pub fn with_environment(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.environment.insert(key.into(), value.into());
        self
    }
    /// Sets the deadline.
    #[must_use]
    pub fn with_timeout(mut self, value: Duration) -> Self {
        self.timeout = value;
        self
    }
    /// Sets the combined output budget.
    #[must_use]
    pub fn with_max_output_bytes(mut self, value: usize) -> Self {
        self.max_output_bytes = value;
        self
    }
    /// Returns the gate name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }
    /// Returns the absolute executable path.
    #[must_use]
    pub fn executable(&self) -> &Path {
        &self.executable
    }
    /// Returns the literal arguments.
    #[must_use]
    pub fn arguments(&self) -> &[String] {
        &self.arguments
    }
    /// Returns the explicit environment values.
    #[must_use]
    pub fn environment(&self) -> &BTreeMap<String, String> {
        &self.environment
    }
    /// Returns the deadline.
    #[must_use]
    pub fn timeout(&self) -> Duration {
        self.timeout
    }
    /// Returns the combined output budget.
    #[must_use]
    pub fn max_output_bytes(&self) -> usize {
        self.max_output_bytes
    }
    fn validate(&self) -> Result<(), GateError> {
        if self.name.is_empty()
            || !self
                .name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
        {
            return Err(GateError::InvalidDefinition(
                "gate name must be a nonempty identifier",
            ));
        }
        if !self.executable.is_absolute() {
            return Err(GateError::InvalidDefinition(
                "gate executable must be absolute",
            ));
        }
        if self.timeout.is_zero() || self.max_output_bytes == 0 {
            return Err(GateError::InvalidDefinition("gate limits must be nonzero"));
        }
        Ok(())
    }
}

/// Executes gate definitions.
#[derive(Clone, Copy, Debug, Default)]
pub struct GateRunner;

impl GateRunner {
    /// Executes one definition in an existing directory.
    pub fn run(
        &self,
        definition: &GateDefinition,
        directory: impl AsRef<Path>,
    ) -> Result<GateReport, GateError> {
        definition.validate()?;
        let directory = std::fs::canonicalize(directory).map_err(GateError::Io)?;
        if !directory.is_dir() {
            return Err(GateError::InvalidDirectory(directory));
        }
        run_process(definition, &directory)
    }
    /// Executes definitions in input order and rejects duplicate names before any process starts.
    pub fn run_all(
        &self,
        definitions: &[GateDefinition],
        directory: impl AsRef<Path>,
    ) -> Result<Vec<GateReport>, GateError> {
        let mut names = HashSet::new();
        for definition in definitions {
            definition.validate()?;
            if !names.insert(&definition.name) {
                return Err(GateError::DuplicateName(definition.name.clone()));
            }
        }
        definitions
            .iter()
            .map(|definition| self.run(definition, directory.as_ref()))
            .collect()
    }
}

/// Process termination outcome.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GateOutcome {
    /// The child exited with status zero.
    Passed,
    /// The child exited with a nonzero status.
    Failed,
    /// The runner killed the direct child after its deadline elapsed.
    TimedOut,
    /// The runner killed the direct child after the output limit was exceeded.
    OutputLimitExceeded,
}

/// Bounded process evidence from a gate run.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GateReport {
    name: String,
    directory: PathBuf,
    outcome: GateOutcome,
    exit_code: Option<i32>,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    output_truncated: bool,
}
impl GateReport {
    /// Returns the gate name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }
    /// Returns the canonical execution directory.
    #[must_use]
    pub fn directory(&self) -> &Path {
        &self.directory
    }
    /// Returns the terminal outcome.
    #[must_use]
    pub const fn outcome(&self) -> GateOutcome {
        self.outcome
    }
    /// Returns an exit code when present.
    #[must_use]
    pub const fn exit_code(&self) -> Option<i32> {
        self.exit_code
    }
    /// Returns raw stdout.
    #[must_use]
    pub fn stdout(&self) -> &[u8] {
        &self.stdout
    }
    /// Returns raw stderr.
    #[must_use]
    pub fn stderr(&self) -> &[u8] {
        &self.stderr
    }
    /// Returns whether output was discarded after the budget was reached.
    #[must_use]
    pub const fn output_truncated(&self) -> bool {
        self.output_truncated
    }
}

/// Gate execution failure.
#[derive(Debug)]
pub enum GateError {
    /// Gate definition validation failed.
    InvalidDefinition(&'static str),
    /// The supplied execution directory was invalid.
    InvalidDirectory(PathBuf),
    /// A batch repeated a gate name.
    DuplicateName(String),
    /// Process or filesystem I/O failed.
    Io(io::Error),
    /// Output capture synchronization failed.
    CapturePoisoned,
    /// An output-draining thread panicked.
    WorkerPanicked,
}
impl fmt::Display for GateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidDefinition(s) => write!(f, "invalid gate definition: {s}"),
            Self::InvalidDirectory(p) => write!(f, "invalid gate directory: {}", p.display()),
            Self::DuplicateName(n) => write!(f, "duplicate gate name: {n}"),
            Self::Io(e) => write!(f, "gate I/O error: {e}"),
            Self::CapturePoisoned => f.write_str("gate capture mutex poisoned"),
            Self::WorkerPanicked => f.write_str("gate output worker panicked"),
        }
    }
}
impl std::error::Error for GateError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        if let Self::Io(e) = self {
            Some(e)
        } else {
            None
        }
    }
}

#[derive(Default)]
struct Capture {
    out: Vec<u8>,
    err: Vec<u8>,
}
fn run_process(definition: &GateDefinition, directory: &Path) -> Result<GateReport, GateError> {
    let mut child = Command::new(&definition.executable)
        .current_dir(directory)
        .args(&definition.arguments)
        .env_clear()
        .envs(&definition.environment)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(GateError::Io)?;
    let capture = Arc::new(Mutex::new(Capture::default()));
    let limited = Arc::new(AtomicBool::new(false));
    let out = drain(
        child.stdout.take().unwrap(),
        Arc::clone(&capture),
        Arc::clone(&limited),
        definition.max_output_bytes,
        true,
    );
    let err = drain(
        child.stderr.take().unwrap(),
        Arc::clone(&capture),
        Arc::clone(&limited),
        definition.max_output_bytes,
        false,
    );
    let start = Instant::now();
    let (mut outcome, code) = loop {
        if let Some(s) = child.try_wait().map_err(GateError::Io)? {
            break (
                if s.success() {
                    GateOutcome::Passed
                } else {
                    GateOutcome::Failed
                },
                s.code(),
            );
        }
        if limited.load(Ordering::Acquire) || start.elapsed() >= definition.timeout {
            let outcome = if limited.load(Ordering::Acquire) {
                GateOutcome::OutputLimitExceeded
            } else {
                GateOutcome::TimedOut
            };
            let _ = child.kill();
            let s = child.wait().map_err(GateError::Io)?;
            break (outcome, s.code());
        }
        thread::sleep(Duration::from_millis(5));
    };
    out.join().map_err(|_| GateError::WorkerPanicked)??;
    err.join().map_err(|_| GateError::WorkerPanicked)??;
    if limited.load(Ordering::Acquire) {
        outcome = GateOutcome::OutputLimitExceeded;
    }
    let c = capture.lock().map_err(|_| GateError::CapturePoisoned)?;
    Ok(GateReport {
        name: definition.name.clone(),
        directory: directory.to_path_buf(),
        outcome,
        exit_code: code,
        stdout: c.out.clone(),
        stderr: c.err.clone(),
        output_truncated: limited.load(Ordering::Acquire),
    })
}
fn drain<R: Read + Send + 'static>(
    mut r: R,
    c: Arc<Mutex<Capture>>,
    l: Arc<AtomicBool>,
    max: usize,
    out: bool,
) -> thread::JoinHandle<Result<(), GateError>> {
    thread::spawn(move || {
        let mut b = [0; 4096];
        loop {
            let n = r.read(&mut b).map_err(GateError::Io)?;
            if n == 0 {
                return Ok(());
            }
            let mut c = c.lock().map_err(|_| GateError::CapturePoisoned)?;
            let used = c.out.len() + c.err.len();
            let take = n.min(max.saturating_sub(used));
            if out {
                c.out.extend_from_slice(&b[..take])
            } else {
                c.err.extend_from_slice(&b[..take])
            };
            if take < n {
                l.store(true, Ordering::Release)
            }
        }
    })
}

/// Project-relative directory holding reviewed gate profiles.
pub const GATE_PROFILE_RELATIVE_PATH: &str = ".forge/gates";
const MAX_GATE_PROFILE_BYTES: u64 = 64 * 1024;
const MAX_GATE_ID_BYTES: usize = 64;
const MAX_GATE_ARGUMENTS: usize = 64;
const MAX_GATE_ENVIRONMENT: usize = 64;
const MAX_GATE_TIMEOUT_MS: u64 = 24 * 60 * 60 * 1000;
const MAX_GATE_OUTPUT_BYTES: usize = 64 * 1024 * 1024;

/// Gate profile loading or validation failure.
#[derive(Debug)]
pub enum GateProfileError {
    /// The profile or its identifier is invalid.
    Invalid(String),
    /// Filesystem I/O failed.
    Io(io::Error),
}
impl fmt::Display for GateProfileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid(reason) => write!(f, "invalid gate profile: {reason}"),
            Self::Io(e) => write!(f, "gate profile I/O error: {e}"),
        }
    }
}
impl std::error::Error for GateProfileError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        if let Self::Io(e) = self {
            Some(e)
        } else {
            None
        }
    }
}
impl From<io::Error> for GateProfileError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

/// Loads reviewed, project-local gate profiles from `.forge/gates`.
///
/// Each `<id>.conf` file uses the agent-profile `key=value` form: `version=1`, an absolute
/// `executable`, repeated literal `argument` values, repeated `env.<NAME>` values, and optional
/// `timeout_ms` and `max_output_bytes`. The file stem is the gate name.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GateProfileStore {
    project_root: PathBuf,
}

impl GateProfileStore {
    /// Creates a store rooted at a project directory.
    #[must_use]
    pub fn new(project_root: impl Into<PathBuf>) -> Self {
        Self {
            project_root: project_root.into(),
        }
    }

    /// Returns the gate profile directory.
    #[must_use]
    pub fn directory(&self) -> PathBuf {
        self.project_root.join(GATE_PROFILE_RELATIVE_PATH)
    }

    /// Loads one named gate profile.
    pub fn load(&self, id: &str) -> Result<GateDefinition, GateProfileError> {
        validate_gate_id(id)?;
        let path = self.directory().join(format!("{id}.conf"));
        let metadata = fs::symlink_metadata(&path).map_err(|error| {
            if error.kind() == io::ErrorKind::NotFound {
                GateProfileError::Invalid(format!("gate not found: {id}"))
            } else {
                GateProfileError::Io(error)
            }
        })?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(GateProfileError::Invalid(format!(
                "gate path is not a regular file: {}",
                path.display()
            )));
        }
        if metadata.len() > MAX_GATE_PROFILE_BYTES {
            return Err(GateProfileError::Invalid(format!(
                "gate {id} exceeds {MAX_GATE_PROFILE_BYTES} bytes"
            )));
        }
        let text = fs::read_to_string(&path)?;
        parse_gate_profile(id, &text)
    }

    /// Loads every gate profile in lexical identifier order. A missing directory means the
    /// project declares no gates.
    pub fn list(&self) -> Result<Vec<GateDefinition>, GateProfileError> {
        let entries = match fs::read_dir(self.directory()) {
            Ok(entries) => entries,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(GateProfileError::Io(error)),
        };
        let mut ids = Vec::new();
        for entry in entries {
            let path = entry?.path();
            if path.extension().and_then(|value| value.to_str()) != Some("conf") {
                continue;
            }
            let Some(id) = path.file_stem().and_then(|value| value.to_str()) else {
                return Err(GateProfileError::Invalid(format!(
                    "gate filename is not UTF-8: {}",
                    path.display()
                )));
            };
            ids.push(id.to_owned());
        }
        ids.sort();
        ids.iter().map(|id| self.load(id)).collect()
    }
}

fn validate_gate_id(id: &str) -> Result<(), GateProfileError> {
    if id.is_empty()
        || id.len() > MAX_GATE_ID_BYTES
        || !id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
    {
        return Err(GateProfileError::Invalid(format!(
            "gate ID must be 1-{MAX_GATE_ID_BYTES} ASCII letters, digits, '-' or '_': {id:?}"
        )));
    }
    Ok(())
}

fn parse_gate_profile(id: &str, text: &str) -> Result<GateDefinition, GateProfileError> {
    let invalid = |line: usize, reason: &str| {
        GateProfileError::Invalid(format!("gate {id} line {line}: {reason}"))
    };
    let mut version = None;
    let mut executable = None;
    let mut arguments = Vec::new();
    let mut environment = BTreeMap::new();
    let mut timeout_ms = None;
    let mut max_output_bytes = None;

    for (index, line) in text.lines().enumerate() {
        let number = index + 1;
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            return Err(invalid(number, "must use key=value"));
        };
        if value.contains('\0') {
            return Err(invalid(number, "contains NUL"));
        }
        match key {
            "version" => {
                let parsed = value
                    .parse::<u32>()
                    .map_err(|_| invalid(number, "invalid version"))?;
                if version.replace(parsed).is_some() {
                    return Err(invalid(number, "repeats version"));
                }
            }
            "executable" => {
                if executable.replace(PathBuf::from(value)).is_some() {
                    return Err(invalid(number, "repeats executable"));
                }
            }
            "argument" => {
                if arguments.len() == MAX_GATE_ARGUMENTS {
                    return Err(invalid(number, "too many arguments"));
                }
                arguments.push(value.to_owned());
            }
            "timeout_ms" => {
                let parsed = value
                    .parse::<u64>()
                    .ok()
                    .filter(|value| (1..=MAX_GATE_TIMEOUT_MS).contains(value))
                    .ok_or_else(|| invalid(number, "timeout_ms is out of range"))?;
                if timeout_ms.replace(parsed).is_some() {
                    return Err(invalid(number, "repeats timeout_ms"));
                }
            }
            "max_output_bytes" => {
                let parsed = value
                    .parse::<usize>()
                    .ok()
                    .filter(|value| (1..=MAX_GATE_OUTPUT_BYTES).contains(value))
                    .ok_or_else(|| invalid(number, "max_output_bytes is out of range"))?;
                if max_output_bytes.replace(parsed).is_some() {
                    return Err(invalid(number, "repeats max_output_bytes"));
                }
            }
            key if key.starts_with("env.") => {
                let name = &key["env.".len()..];
                if name.is_empty() || name.contains('=') || name.contains('\0') {
                    return Err(invalid(number, "invalid environment key"));
                }
                if environment.len() == MAX_GATE_ENVIRONMENT && !environment.contains_key(name) {
                    return Err(invalid(number, "too many environment values"));
                }
                if environment
                    .insert(name.to_owned(), value.to_owned())
                    .is_some()
                {
                    return Err(invalid(number, "repeats an environment key"));
                }
            }
            _ => return Err(invalid(number, &format!("unknown key {key}"))),
        }
    }

    if version != Some(1) {
        return Err(GateProfileError::Invalid(format!(
            "gate {id} requires version=1"
        )));
    }
    let executable = executable
        .ok_or_else(|| GateProfileError::Invalid(format!("gate {id} is missing executable")))?;
    if !executable.is_absolute() || !executable.is_file() {
        return Err(GateProfileError::Invalid(format!(
            "gate {id} executable must be an existing absolute file: {}",
            executable.display()
        )));
    }
    let mut definition = GateDefinition::new(id, executable)
        .with_timeout(Duration::from_millis(timeout_ms.unwrap_or(60_000)))
        .with_max_output_bytes(max_output_bytes.unwrap_or(1024 * 1024));
    for argument in arguments {
        definition = definition.with_argument(argument);
    }
    for (key, value) in environment {
        definition = definition.with_environment(key, value);
    }
    definition
        .validate()
        .map_err(|error| GateProfileError::Invalid(error.to_string()))?;
    Ok(definition)
}
