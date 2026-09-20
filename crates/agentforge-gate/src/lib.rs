//! Deterministic local process gates with bounded evidence.

use std::collections::{BTreeMap, HashSet};
use std::fmt;
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
