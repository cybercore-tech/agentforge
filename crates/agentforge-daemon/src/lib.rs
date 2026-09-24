//! Bounded, local-only orchestration daemon protocol and lifecycle.

use agentforge_adapter::{AgentProfileStore, ProcessAdapter, ProcessAdapterConfig};
use agentforge_audit::FileAuditStore;
use agentforge_core::task::TaskId;
use agentforge_operator::approved_boundaries;
use agentforge_orchestrator::{
    ProcessExecution, execute_process_persisted, launch_process_persisted,
};
use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::net::{IpAddr, SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use std::{ffi::OsStr, thread};

const PROTOCOL: &str = "AFD1";
const MAX_FRAME_BYTES: usize = 16 * 1024;
const MAX_RESPONSE_BYTES: usize = 16 * 1024;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(2);
const START_TIMEOUT: Duration = Duration::from_secs(5);
// Every client writes one complete frame immediately after connecting. Bound the
// wait so a client that connects and stalls cannot block later requests,
// including a cooperative stop, on this single-threaded accept loop.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(1);
// Executions run the agent and its gates before the final response, so they
// have no fixed total deadline here (the agent profile bounds the agent). The
// daemon proves liveness with a keepalive frame every interval, and the client
// gives up only after the idle timeout passes without any frame.
const KEEPALIVE_INTERVAL: Duration = Duration::from_secs(1);
const EXECUTION_IDLE_TIMEOUT: Duration = Duration::from_secs(10);
const DAEMON_DIR: &str = ".forge/daemon";
const ENDPOINT_FILE: &str = "endpoint";
const LOCK_FILE: &str = "lock";

/// Default loopback bind address. Port zero asks the operating system for a free port.
pub const DEFAULT_BIND: SocketAddr = SocketAddr::new(IpAddr::V4(std::net::Ipv4Addr::LOCALHOST), 0);

/// A validated daemon endpoint record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Endpoint {
    /// Loopback address where the daemon accepts requests.
    pub address: SocketAddr,
    /// Process identity recorded when the endpoint was created.
    pub pid: u32,
}

/// Bounded daemon status response.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DaemonStatus {
    /// Endpoint currently serving the project.
    pub endpoint: Endpoint,
}

/// Daemon lifecycle or request failure.
#[derive(Debug)]
pub enum DaemonError {
    /// Filesystem or socket I/O failed.
    Io(io::Error),
    /// The request or endpoint protocol was invalid.
    Protocol(String),
    /// The project has no active daemon endpoint.
    NotRunning,
    /// A daemon already owns the project.
    AlreadyRunning(Endpoint),
    /// Daemon metadata exists but cannot be reached; no automatic adoption is attempted.
    StaleInstance(PathBuf),
    /// Existing orchestration code rejected the request.
    Execution(String),
    /// A spawned daemon did not become ready within the bounded startup window.
    StartTimeout(PathBuf),
    /// A cooperative stop did not clear the daemon endpoint within the bounded window.
    StopTimeout(PathBuf),
    /// The daemon is executing another task; the request was refused without side effects.
    Busy(String),
    /// The daemon stopped responding after accepting an execution request. The task may still
    /// be running or may have finished; its durable state is authoritative.
    ExecutionInterrupted(String),
}

impl fmt::Display for DaemonError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "daemon I/O failed: {error}"),
            Self::Protocol(reason) => write!(formatter, "daemon protocol error: {reason}"),
            Self::NotRunning => formatter.write_str("daemon is not running"),
            Self::AlreadyRunning(endpoint) => {
                write!(formatter, "daemon already running at {}", endpoint.address)
            }
            Self::StaleInstance(path) => write!(
                formatter,
                "stale daemon metadata at {}; remove it only after confirming no daemon owns the project",
                path.display()
            ),
            Self::Execution(reason) => write!(formatter, "daemon execution failed: {reason}"),
            Self::StartTimeout(root) => write!(
                formatter,
                "daemon did not become ready within the startup timeout for {}",
                root.display()
            ),
            Self::StopTimeout(root) => write!(
                formatter,
                "daemon did not stop within the shutdown timeout for {}",
                root.display()
            ),
            Self::Busy(reason) => write!(formatter, "daemon is busy: {reason}"),
            Self::ExecutionInterrupted(reason) => write!(
                formatter,
                "daemon stopped responding during execution ({reason}); the daemon metadata was not changed; check the task with `forge task inspect` and the daemon with `forge daemon status`"
            ),
        }
    }
}

impl std::error::Error for DaemonError {}

impl From<io::Error> for DaemonError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

/// Runs a foreground daemon until a cooperative stop request is received.
pub fn serve(root: impl AsRef<Path>, bind: SocketAddr) -> Result<(), DaemonError> {
    let server = Server::start(root.as_ref(), bind)?;
    server.run()
}

/// Starts the installed forged executable and waits for its loopback endpoint.
pub fn start(root: impl AsRef<Path>, bind: SocketAddr) -> Result<DaemonStatus, DaemonError> {
    start_with_program(root, bind, OsStr::new("forged"))
}

/// Starts a specific daemon executable and waits for bounded readiness.
pub fn start_with_program(
    root: impl AsRef<Path>,
    bind: SocketAddr,
    program: impl AsRef<OsStr>,
) -> Result<DaemonStatus, DaemonError> {
    let root = root.as_ref();
    match status(root) {
        Ok(existing) => return Err(DaemonError::AlreadyRunning(existing.endpoint)),
        Err(DaemonError::NotRunning) => {}
        Err(error) => return Err(error),
    }
    let mut child = spawn_daemon(program.as_ref(), root, bind)?;
    let deadline = Instant::now() + START_TIMEOUT;
    loop {
        match status(root) {
            Ok(status) => return Ok(status),
            Err(DaemonError::NotRunning) => {}
            Err(error) => {
                terminate_owned_child(&mut child);
                return Err(error);
            }
        }
        if let Some(status) = child.try_wait()? {
            let output = child.wait_with_output()?;
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
            return Err(DaemonError::Execution(format!(
                "forged exited during startup with {status}: {stderr}"
            )));
        }
        if Instant::now() >= deadline {
            terminate_owned_child(&mut child);
            return Err(DaemonError::StartTimeout(root.to_path_buf()));
        }
        thread::sleep(Duration::from_millis(25));
    }
}

/// Cooperatively stops and starts the project daemon.
pub fn restart(root: impl AsRef<Path>, bind: SocketAddr) -> Result<DaemonStatus, DaemonError> {
    restart_with_program(root, bind, OsStr::new("forged"))
}

/// Cooperatively stops and starts a specific daemon executable.
pub fn restart_with_program(
    root: impl AsRef<Path>,
    bind: SocketAddr,
    program: impl AsRef<OsStr>,
) -> Result<DaemonStatus, DaemonError> {
    stop(root.as_ref())?;
    start_with_program(root, bind, program)
}

fn terminate_owned_child(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

fn spawn_daemon(program: &OsStr, root: &Path, bind: SocketAddr) -> Result<Child, DaemonError> {
    let deadline = Instant::now() + START_TIMEOUT;
    loop {
        match Command::new(program)
            .arg("serve")
            .arg("--root")
            .arg(root)
            .arg("--bind")
            .arg(bind.to_string())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
        {
            Ok(child) => return Ok(child),
            // Windows can retain an executable image lock briefly after a
            // cooperative stop has removed the daemon endpoint. Retry only
            // this transient launch error within the existing startup bound.
            Err(error)
                if error.kind() == io::ErrorKind::PermissionDenied && Instant::now() < deadline =>
            {
                thread::sleep(Duration::from_millis(25));
            }
            Err(error) => return Err(DaemonError::Io(error)),
        }
    }
}

/// Queries the daemon endpoint for one project.
pub fn status(root: impl AsRef<Path>) -> Result<DaemonStatus, DaemonError> {
    let endpoint = read_endpoint(root.as_ref())?;
    let response = send_request(root.as_ref(), Request::Status)?;
    if response != Response::Status {
        return Err(DaemonError::Protocol("unexpected status response".into()));
    }
    Ok(DaemonStatus { endpoint })
}

/// Submits one explicitly configured task to the daemon.
pub fn run_task(
    root: impl AsRef<Path>,
    task_id: &TaskId,
    executable: impl AsRef<Path>,
) -> Result<String, DaemonError> {
    let executable = executable.as_ref();
    if !executable.is_absolute() {
        return Err(DaemonError::Protocol(
            "daemon executable path must be absolute".into(),
        ));
    }
    match send_execution_request(
        root.as_ref(),
        Request::Run {
            task_id: task_id.clone(),
            executable: executable.to_path_buf(),
        },
    )? {
        Response::Run(message) => Ok(message),
        Response::Launch(_) => Err(DaemonError::Protocol("unexpected run response".into())),
        Response::Status => Err(DaemonError::Protocol("unexpected run response".into())),
        Response::Stop => Err(DaemonError::Protocol("unexpected run response".into())),
        Response::Error(message) => Err(DaemonError::Execution(message)),
        Response::Busy(message) => Err(DaemonError::Busy(message)),
        Response::Pending => Err(DaemonError::Protocol("unexpected keepalive".into())),
    }
}

/// Submits one project-local agent profile to the daemon.
pub fn run_profile(
    root: impl AsRef<Path>,
    task_id: &TaskId,
    profile: &str,
) -> Result<String, DaemonError> {
    validate_profile_id(profile)?;
    match send_execution_request(
        root.as_ref(),
        Request::RunProfile {
            task_id: task_id.clone(),
            profile: profile.to_owned(),
        },
    )? {
        Response::Run(message) => Ok(message),
        Response::Launch(_) => Err(DaemonError::Protocol("unexpected run response".into())),
        Response::Status => Err(DaemonError::Protocol("unexpected run response".into())),
        Response::Stop => Err(DaemonError::Protocol("unexpected run response".into())),
        Response::Error(message) => Err(DaemonError::Execution(message)),
        Response::Busy(message) => Err(DaemonError::Busy(message)),
        Response::Pending => Err(DaemonError::Protocol("unexpected keepalive".into())),
    }
}

fn validate_profile_id(profile: &str) -> Result<(), DaemonError> {
    if profile.is_empty()
        || profile.len() > 64
        || !profile
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    {
        return Err(DaemonError::Protocol("daemon profile ID is invalid".into()));
    }
    Ok(())
}

fn validate_protocol_path(path: &Path) -> Result<(), DaemonError> {
    if path
        .to_string_lossy()
        .chars()
        .any(|character| matches!(character, '\n' | '\r' | '\t'))
    {
        return Err(DaemonError::Protocol(
            "daemon path contains control characters".into(),
        ));
    }
    Ok(())
}

fn validate_base_ref(base_ref: &str) -> Result<(), DaemonError> {
    if base_ref.is_empty()
        || base_ref.len() > 256
        || base_ref.starts_with('-')
        || base_ref
            .chars()
            .any(|character| matches!(character, '\n' | '\r' | '\t'))
    {
        return Err(DaemonError::Protocol("daemon base ref is invalid".into()));
    }
    Ok(())
}

/// Submits one explicitly configured task to the daemon with managed worktree preparation.
pub fn launch_task(
    root: impl AsRef<Path>,
    task_id: &TaskId,
    executable: impl AsRef<Path>,
    base_ref: &str,
) -> Result<String, DaemonError> {
    let executable = executable.as_ref();
    validate_protocol_path(executable)?;
    if !executable.is_absolute() {
        return Err(DaemonError::Protocol(
            "daemon executable path must be absolute".into(),
        ));
    }
    validate_base_ref(base_ref)?;
    match send_execution_request(
        root.as_ref(),
        Request::Launch {
            task_id: task_id.clone(),
            executable: executable.to_path_buf(),
            base_ref: base_ref.to_owned(),
        },
    )? {
        Response::Launch(message) => Ok(message),
        Response::Run(_) => Err(DaemonError::Protocol("unexpected launch response".into())),
        Response::Status => Err(DaemonError::Protocol("unexpected launch response".into())),
        Response::Stop => Err(DaemonError::Protocol("unexpected launch response".into())),
        Response::Error(message) => Err(DaemonError::Execution(message)),
        Response::Busy(message) => Err(DaemonError::Busy(message)),
        Response::Pending => Err(DaemonError::Protocol("unexpected keepalive".into())),
    }
}

/// Submits one project-local agent profile with managed worktree preparation.
pub fn launch_profile(
    root: impl AsRef<Path>,
    task_id: &TaskId,
    profile: &str,
    base_ref: &str,
) -> Result<String, DaemonError> {
    validate_profile_id(profile)?;
    validate_base_ref(base_ref)?;
    match send_execution_request(
        root.as_ref(),
        Request::LaunchProfile {
            task_id: task_id.clone(),
            profile: profile.to_owned(),
            base_ref: base_ref.to_owned(),
        },
    )? {
        Response::Launch(message) => Ok(message),
        Response::Run(_) => Err(DaemonError::Protocol("unexpected launch response".into())),
        Response::Status => Err(DaemonError::Protocol("unexpected launch response".into())),
        Response::Stop => Err(DaemonError::Protocol("unexpected launch response".into())),
        Response::Error(message) => Err(DaemonError::Execution(message)),
        Response::Busy(message) => Err(DaemonError::Busy(message)),
        Response::Pending => Err(DaemonError::Protocol("unexpected keepalive".into())),
    }
}

/// Requests a cooperative daemon stop.
pub fn stop(root: impl AsRef<Path>) -> Result<(), DaemonError> {
    let root = root.as_ref();
    match send_request(root, Request::Stop) {
        Ok(Response::Stop) => wait_until_stopped(root),
        Ok(Response::Busy(message)) => Err(DaemonError::Busy(message)),
        // A stop response can be lost while the daemon is already tearing down
        // its listener. Continue observing the endpoint instead of racing a
        // restart against cooperative cleanup.
        Err(DaemonError::StaleInstance(_)) => wait_until_stopped(root),
        Ok(_) => Err(DaemonError::Protocol("unexpected stop response".into())),
        Err(error) => Err(error),
    }
}

fn wait_until_stopped(root: &Path) -> Result<(), DaemonError> {
    let deadline = Instant::now() + START_TIMEOUT;
    let (_, lock_path) = daemon_paths(root);
    loop {
        match status(root) {
            // Teardown removes the endpoint before the lock. Wait for both so an
            // immediate restart never observes the previous daemon's lock.
            Err(DaemonError::NotRunning) if removal_complete(&lock_path)? => return Ok(()),
            Err(DaemonError::NotRunning) => {}
            Ok(_) | Err(DaemonError::StaleInstance(_)) => {}
            // Windows reports a file whose deletion is still pending as
            // "Access is denied"; the endpoint is mid-removal, so keep observing.
            Err(DaemonError::Io(error)) if error.kind() == io::ErrorKind::PermissionDenied => {}
            Err(error) => return Err(error),
        }
        if Instant::now() >= deadline {
            return Err(DaemonError::StopTimeout(root.to_path_buf()));
        }
        thread::sleep(Duration::from_millis(25));
    }
}

/// Returns whether a daemon metadata file is fully removed. A Windows file in the
/// delete-pending state still blocks re-creation and reports `PermissionDenied`
/// instead of `NotFound`, so only `NotFound` counts as removed.
fn removal_complete(path: &Path) -> Result<bool, DaemonError> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(false),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(true),
        Err(error) if error.kind() == io::ErrorKind::PermissionDenied => Ok(false),
        Err(error) => Err(DaemonError::Io(error)),
    }
}

fn daemon_paths(root: &Path) -> (PathBuf, PathBuf) {
    let directory = root.join(DAEMON_DIR);
    (directory.join(ENDPOINT_FILE), directory.join(LOCK_FILE))
}

fn read_endpoint(root: &Path) -> Result<Endpoint, DaemonError> {
    let (endpoint_path, _) = daemon_paths(root);
    let bytes = fs::read(&endpoint_path).map_err(|error| {
        if error.kind() == io::ErrorKind::NotFound {
            DaemonError::NotRunning
        } else {
            DaemonError::Io(error)
        }
    })?;
    parse_endpoint(&bytes).map_err(DaemonError::Protocol)
}

fn send_request(root: &Path, request: Request) -> Result<Response, DaemonError> {
    let endpoint = read_endpoint(root)?;
    let mut stream = TcpStream::connect_timeout(&endpoint.address, CONNECT_TIMEOUT)
        .map_err(|error| map_transport_error(root, error))?;
    stream
        .set_read_timeout(Some(CONNECT_TIMEOUT))
        .map_err(|error| map_transport_error(root, error))?;
    stream
        .set_write_timeout(Some(CONNECT_TIMEOUT))
        .map_err(|error| map_transport_error(root, error))?;
    let frame = request.encode();
    stream
        .write_all(frame.as_bytes())
        .map_err(|error| map_transport_error(root, error))?;
    stream
        .flush()
        .map_err(|error| map_transport_error(root, error))?;
    let response = read_frame(&mut stream).map_err(|error| map_transport_error(root, error))?;
    parse_response(&response).map_err(DaemonError::Protocol)
}

/// Sends an execution request and reads keepalive frames until the final response. Transport
/// failures before the request is written mean the endpoint is unreachable; failures after it
/// mean the daemon stopped responding mid-execution, which is never reported as stale metadata.
fn send_execution_request(root: &Path, request: Request) -> Result<Response, DaemonError> {
    let endpoint = read_endpoint(root)?;
    let mut stream = TcpStream::connect_timeout(&endpoint.address, CONNECT_TIMEOUT)
        .map_err(|error| map_transport_error(root, error))?;
    stream
        .set_write_timeout(Some(CONNECT_TIMEOUT))
        .map_err(|error| map_transport_error(root, error))?;
    stream
        .write_all(request.encode().as_bytes())
        .and_then(|()| stream.flush())
        .map_err(|error| map_transport_error(root, error))?;
    stream
        .set_read_timeout(Some(EXECUTION_IDLE_TIMEOUT))
        .map_err(|error| DaemonError::ExecutionInterrupted(error.to_string()))?;
    loop {
        let frame = read_frame(&mut stream)
            .map_err(|error| DaemonError::ExecutionInterrupted(error.to_string()))?;
        match parse_response(&frame).map_err(DaemonError::Protocol)? {
            Response::Pending => {}
            response => return Ok(response),
        }
    }
}

fn map_transport_error(root: &Path, error: io::Error) -> DaemonError {
    if matches!(
        error.kind(),
        io::ErrorKind::ConnectionRefused
            | io::ErrorKind::ConnectionReset
            | io::ErrorKind::ConnectionAborted
            | io::ErrorKind::BrokenPipe
            | io::ErrorKind::NotConnected
            | io::ErrorKind::TimedOut
            // Unix reports an elapsed socket read timeout as WouldBlock where
            // Windows reports TimedOut; classify both the same way.
            | io::ErrorKind::WouldBlock
            // macOS rejects setsockopt on a connection reset while the daemon drops its listener
            // with EINVAL; Linux accepts it. Every timeout this client passes is a nonzero
            // constant, so here InvalidInput can only mean the transport is gone (P2-M032).
            | io::ErrorKind::InvalidInput
            | io::ErrorKind::NotFound
    ) {
        DaemonError::StaleInstance(daemon_paths(root).0)
    } else {
        DaemonError::Io(error)
    }
}

/// The single execution slot: the task ID of the execution in progress, if any.
type ExecutionSlot = Arc<Mutex<Option<String>>>;

/// Clears the execution slot when the worker finishes, including on panic.
struct SlotGuard(ExecutionSlot);

impl Drop for SlotGuard {
    fn drop(&mut self) {
        if let Ok(mut slot) = self.0.lock() {
            *slot = None;
        }
    }
}

fn slot_holder(slot: &ExecutionSlot) -> Option<String> {
    slot.lock().map_or_else(
        |poisoned| poisoned.into_inner().clone(),
        |guard| guard.clone(),
    )
}

struct Server {
    root: PathBuf,
    active: ExecutionSlot,
    listener: TcpListener,
    endpoint_path: PathBuf,
    lock_path: PathBuf,
    lock: Option<File>,
}

impl Server {
    fn start(root: &Path, bind: SocketAddr) -> Result<Self, DaemonError> {
        if !bind.ip().is_loopback() {
            return Err(DaemonError::Protocol(
                "daemon bind address must be loopback".into(),
            ));
        }
        let root = root.canonicalize().map_err(DaemonError::Io)?;
        agentforge_worktree::WorktreeManager::new(&root).map_err(|error| {
            DaemonError::Execution(format!("repository preflight failed: {error}"))
        })?;
        let (endpoint_path, lock_path) = daemon_paths(&root);
        let directory = endpoint_path
            .parent()
            .ok_or_else(|| DaemonError::Protocol("daemon directory has no parent".into()))?;
        fs::create_dir_all(directory)?;

        let lock = match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&lock_path)
        {
            Ok(mut file) => {
                writeln!(file, "pid={}", std::process::id())?;
                file.sync_all()?;
                file
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                match parse_endpoint(&fs::read(&endpoint_path).unwrap_or_default()) {
                    Ok(endpoint) => {
                        match TcpStream::connect_timeout(&endpoint.address, CONNECT_TIMEOUT) {
                            Ok(_) => return Err(DaemonError::AlreadyRunning(endpoint)),
                            Err(_) => return Err(DaemonError::StaleInstance(lock_path)),
                        }
                    }
                    Err(_) => return Err(DaemonError::StaleInstance(lock_path)),
                }
            }
            Err(error) => return Err(DaemonError::Io(error)),
        };

        let listener = match TcpListener::bind(bind) {
            Ok(listener) => listener,
            Err(error) => {
                drop(lock);
                let _ = fs::remove_file(&lock_path);
                return Err(DaemonError::Io(error));
            }
        };
        let endpoint = Endpoint {
            address: listener.local_addr()?,
            pid: std::process::id(),
        };
        write_endpoint(&endpoint_path, &endpoint)?;
        Ok(Self {
            root,
            active: Arc::new(Mutex::new(None)),
            listener,
            endpoint_path,
            lock_path,
            lock: Some(lock),
        })
    }

    fn run(self) -> Result<(), DaemonError> {
        for stream in self.listener.incoming() {
            let mut stream = match stream {
                Ok(stream) => stream,
                Err(error) => return Err(DaemonError::Io(error)),
            };
            if stream.set_read_timeout(Some(REQUEST_TIMEOUT)).is_err()
                || stream.set_write_timeout(Some(CONNECT_TIMEOUT)).is_err()
            {
                continue;
            }
            let request = match read_frame(&mut stream).and_then(|frame| {
                parse_request(&frame)
                    .map_err(|reason| io::Error::new(io::ErrorKind::InvalidData, reason))
            }) {
                Ok(request) => request,
                Err(error) => {
                    let _ = write_response(&mut stream, &Response::Error(error.to_string()));
                    continue;
                }
            };
            match request {
                Request::Status => {
                    let _ = write_response(&mut stream, &Response::Status);
                }
                Request::Stop => {
                    // Never exit while an execution owns task state; the operator retries
                    // once it finishes.
                    if let Some(task) = slot_holder(&self.active) {
                        let _ = write_response(
                            &mut stream,
                            &Response::Busy(format!(
                                "executing task {task}; stop after it finishes"
                            )),
                        );
                        continue;
                    }
                    let _ = write_response(&mut stream, &Response::Stop);
                    return Ok(());
                }
                execution => self.start_execution(execution, stream),
            }
        }
        Ok(())
    }

    /// Hands one execution request to a worker thread if the slot is free. The slot is only
    /// taken on this (accept-loop) thread, so the busy check and `Stop` cannot race.
    fn start_execution(&self, request: Request, mut stream: TcpStream) {
        let task = request
            .task_id()
            .map(ToString::to_string)
            .unwrap_or_default();
        {
            let mut slot = self
                .active
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            if let Some(current) = slot.as_ref() {
                let _ = write_response(
                    &mut stream,
                    &Response::Busy(format!("executing task {current}; retry after it finishes")),
                );
                return;
            }
            *slot = Some(task);
        }
        let guard = SlotGuard(Arc::clone(&self.active));
        let root = self.root.clone();
        thread::spawn(move || {
            let _guard = guard;
            serve_execution(&root, request, stream);
        });
    }
}

/// Runs one execution on a helper thread while streaming keepalive frames to the client, then
/// writes the final response. A client that disconnects does not stop the execution.
fn serve_execution(root: &Path, request: Request, mut stream: TcpStream) {
    let (sender, receiver) = mpsc::channel();
    let execution_root = root.to_path_buf();
    thread::spawn(move || {
        let _ = sender.send(execute_request(&execution_root, request));
    });
    let mut client_connected = true;
    let response = loop {
        match receiver.recv_timeout(KEEPALIVE_INTERVAL) {
            Ok(response) => break response,
            Err(RecvTimeoutError::Timeout) => {
                if client_connected {
                    client_connected = write_response(&mut stream, &Response::Pending).is_ok();
                }
            }
            Err(RecvTimeoutError::Disconnected) => {
                break Response::Error("daemon execution thread panicked".into());
            }
        }
    };
    if client_connected {
        let _ = write_response(&mut stream, &response);
    }
}

fn execute_request(root: &Path, request: Request) -> Response {
    match request {
        Request::Status | Request::Stop => {
            Response::Error("control requests are not executions".into())
        }
        Request::Run {
            task_id,
            executable,
        } => match execute_task(root, &task_id, &executable) {
            Ok(message) => Response::Run(message),
            Err(error) => Response::Error(error.to_string()),
        },
        Request::RunProfile { task_id, profile } => {
            match execute_profile(root, &task_id, &profile) {
                Ok(message) => Response::Run(message),
                Err(error) => Response::Error(error.to_string()),
            }
        }
        Request::Launch {
            task_id,
            executable,
            base_ref,
        } => match execute_launch_task(root, &task_id, &executable, &base_ref) {
            Ok(message) => Response::Launch(message),
            Err(error) => Response::Error(error.to_string()),
        },
        Request::LaunchProfile {
            task_id,
            profile,
            base_ref,
        } => match execute_launch_profile(root, &task_id, &profile, &base_ref) {
            Ok(message) => Response::Launch(message),
            Err(error) => Response::Error(error.to_string()),
        },
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.lock.take();
        let _ = fs::remove_file(&self.endpoint_path);
        let _ = fs::remove_file(&self.lock_path);
    }
}

fn execute_task(root: &Path, task_id: &TaskId, executable: &Path) -> Result<String, DaemonError> {
    let adapter = ProcessAdapter::new(ProcessAdapterConfig::new("daemon-process", executable))
        .map_err(|error| DaemonError::Execution(error.to_string()))?;
    execute_with_adapter(root, task_id, adapter, None)
}

fn execute_profile(root: &Path, task_id: &TaskId, profile: &str) -> Result<String, DaemonError> {
    let profile = AgentProfileStore::new(root)
        .load(profile)
        .map_err(|error| DaemonError::Execution(error.to_string()))?;
    let adapter = ProcessAdapter::new(profile.adapter_config())
        .map_err(|error| DaemonError::Execution(error.to_string()))?;
    execute_with_adapter(root, task_id, adapter, None)
}

fn execute_launch_task(
    root: &Path,
    task_id: &TaskId,
    executable: &Path,
    base_ref: &str,
) -> Result<String, DaemonError> {
    let adapter = ProcessAdapter::new(ProcessAdapterConfig::new(
        "daemon-launch-process",
        executable,
    ))
    .map_err(|error| DaemonError::Execution(error.to_string()))?;
    execute_with_adapter(root, task_id, adapter, Some(base_ref))
}

fn execute_launch_profile(
    root: &Path,
    task_id: &TaskId,
    profile: &str,
    base_ref: &str,
) -> Result<String, DaemonError> {
    let profile = AgentProfileStore::new(root)
        .load(profile)
        .map_err(|error| DaemonError::Execution(error.to_string()))?;
    let adapter = ProcessAdapter::new(profile.adapter_config())
        .map_err(|error| DaemonError::Execution(error.to_string()))?;
    execute_with_adapter(root, task_id, adapter, Some(base_ref))
}

fn execute_with_adapter(
    root: &Path,
    task_id: &TaskId,
    adapter: ProcessAdapter,
    base_ref: Option<&str>,
) -> Result<String, DaemonError> {
    let task_store = agentforge_state::FileTaskStore::for_project_root(root);
    let audit_directory = root.join(".forge");
    fs::create_dir_all(&audit_directory)?;
    let mut audit_store = FileAuditStore::open(audit_directory.join("audit.log"))
        .map_err(|error| DaemonError::Execution(error.to_string()))?;
    let approvals = approved_boundaries(root, task_id)
        .map_err(|error| DaemonError::Execution(error.to_string()))?;
    match base_ref {
        Some(base_ref) => {
            let launch = launch_process_persisted(
                root,
                &task_store,
                &mut audit_store,
                task_id,
                &adapter,
                &approvals,
                base_ref,
            )
            .map_err(|error| DaemonError::Execution(error.to_string()))?;
            Ok(format!(
                "task={} termination={:?} base={} worktree-created={} path={} {}",
                launch.execution.report.task_id(),
                launch.execution.report.termination(),
                launch.base_commit,
                launch.worktree_created,
                launch.worktree.path().display(),
                gate_summary(&launch.execution)
            ))
        }
        None => {
            let execution = execute_process_persisted(
                root,
                &task_store,
                &mut audit_store,
                task_id,
                &adapter,
                &approvals,
            )
            .map_err(|error| DaemonError::Execution(error.to_string()))?;
            Ok(format!(
                "task={} termination={:?} {}",
                execution.report.task_id(),
                execution.report.termination(),
                gate_summary(&execution)
            ))
        }
    }
}

fn gate_summary(execution: &ProcessExecution) -> String {
    let passed = execution.gates.iter().filter(|gate| gate.passed()).count();
    let exit = execution
        .report
        .exit_code()
        .map_or_else(|| "none".to_owned(), |code| code.to_string());
    let mut summary = format!("agent-exit={exit}");
    if let Some(stdout) = &execution.evidence.stdout_log {
        summary.push_str(&format!(" stdout-log={}", stdout.display()));
    }
    if let Some(stderr) = &execution.evidence.stderr_log {
        summary.push_str(&format!(" stderr-log={}", stderr.display()));
    }
    summary.push_str(&format!(" gates={passed}/{}", execution.gates.len()));
    if let Some(failed) = execution.gates.iter().find(|gate| !gate.passed()) {
        summary.push_str(&format!(
            " failed-gate={} outcome={}",
            failed.name(),
            failed.outcome_label()
        ));
    }
    summary
}

#[derive(Debug)]
enum Request {
    Status,
    Stop,
    Run {
        task_id: TaskId,
        executable: PathBuf,
    },
    RunProfile {
        task_id: TaskId,
        profile: String,
    },
    Launch {
        task_id: TaskId,
        executable: PathBuf,
        base_ref: String,
    },
    LaunchProfile {
        task_id: TaskId,
        profile: String,
        base_ref: String,
    },
}

impl Request {
    fn task_id(&self) -> Option<&TaskId> {
        match self {
            Self::Status | Self::Stop => None,
            Self::Run { task_id, .. }
            | Self::RunProfile { task_id, .. }
            | Self::Launch { task_id, .. }
            | Self::LaunchProfile { task_id, .. } => Some(task_id),
        }
    }

    fn encode(&self) -> String {
        match self {
            Self::Status => format!("{PROTOCOL}\tSTATUS\n"),
            Self::Stop => format!("{PROTOCOL}\tSTOP\n"),
            Self::Run {
                task_id,
                executable,
            } => format!("{PROTOCOL}\tRUN\t{task_id}\t{}\n", executable.display()),
            Self::RunProfile { task_id, profile } => {
                format!("{PROTOCOL}\tRUN_PROFILE\t{task_id}\t{profile}\n")
            }
            Self::Launch {
                task_id,
                executable,
                base_ref,
            } => format!(
                "{PROTOCOL}\tLAUNCH\t{task_id}\t{}\t{base_ref}\n",
                executable.display()
            ),
            Self::LaunchProfile {
                task_id,
                profile,
                base_ref,
            } => format!("{PROTOCOL}\tLAUNCH_PROFILE\t{task_id}\t{profile}\t{base_ref}\n"),
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
enum Response {
    Status,
    Stop,
    Run(String),
    Launch(String),
    Error(String),
    /// Execution keepalive; the final response follows.
    Pending,
    /// Refused because an execution is in progress; nothing was changed.
    Busy(String),
}

fn write_response(stream: &mut TcpStream, response: &Response) -> io::Result<()> {
    let value = match response {
        Response::Status => format!("{PROTOCOL}\tOK\tSTATUS\n"),
        Response::Stop => format!("{PROTOCOL}\tOK\tSTOP\n"),
        Response::Run(message) => format!("{PROTOCOL}\tOK\tRUN\t{}\n", sanitize(message)),
        Response::Launch(message) => format!("{PROTOCOL}\tOK\tLAUNCH\t{}\n", sanitize(message)),
        Response::Error(message) => format!("{PROTOCOL}\tERR\t{}\n", sanitize(message)),
        Response::Pending => format!("{PROTOCOL}\tOK\tPENDING\n"),
        Response::Busy(message) => format!("{PROTOCOL}\tBUSY\t{}\n", sanitize(message)),
    };
    if value.len() > MAX_RESPONSE_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "daemon response exceeds limit",
        ));
    }
    stream.write_all(value.as_bytes())?;
    stream.flush()
}

fn parse_request(frame: &[u8]) -> Result<Request, String> {
    let text = std::str::from_utf8(frame).map_err(|_| "request is not UTF-8".to_string())?;
    let text = text.strip_suffix('\n').unwrap_or(text);
    let fields: Vec<&str> = text.split('\t').collect();
    if fields.first().copied() != Some(PROTOCOL) {
        return Err("unsupported protocol version".into());
    }
    match fields.get(1).copied() {
        Some("STATUS") if fields.len() == 2 => Ok(Request::Status),
        Some("STOP") if fields.len() == 2 => Ok(Request::Stop),
        Some("RUN") if fields.len() == 4 => {
            if fields[3].is_empty()
                || fields[3]
                    .chars()
                    .any(|c| c == '\n' || c == '\r' || c == '\t')
            {
                return Err("executable path is invalid".into());
            }
            let task_id =
                TaskId::parse(fields[2].to_string()).map_err(|error| error.to_string())?;
            let executable = PathBuf::from(fields[3]);
            if !executable.is_absolute() {
                return Err("executable path must be absolute".into());
            }
            Ok(Request::Run {
                task_id,
                executable,
            })
        }
        Some("RUN_PROFILE") if fields.len() == 4 => {
            if fields[3].is_empty()
                || fields[3].len() > 64
                || !fields[3]
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
            {
                return Err("profile ID is invalid".into());
            }
            let task_id =
                TaskId::parse(fields[2].to_string()).map_err(|error| error.to_string())?;
            Ok(Request::RunProfile {
                task_id,
                profile: fields[3].to_owned(),
            })
        }
        Some("LAUNCH") if fields.len() == 5 => {
            if fields[3].is_empty()
                || fields[3]
                    .chars()
                    .any(|character| matches!(character, '\n' | '\r' | '\t'))
            {
                return Err("executable path is invalid".into());
            }
            if !Path::new(fields[3]).is_absolute() {
                return Err("executable path must be absolute".into());
            }
            if fields[4].is_empty()
                || fields[4].len() > 256
                || fields[4].starts_with('-')
                || fields[4]
                    .chars()
                    .any(|character| matches!(character, '\n' | '\r' | '\t'))
            {
                return Err("base ref is invalid".into());
            }
            let task_id =
                TaskId::parse(fields[2].to_string()).map_err(|error| error.to_string())?;
            Ok(Request::Launch {
                task_id,
                executable: PathBuf::from(fields[3]),
                base_ref: fields[4].to_owned(),
            })
        }
        Some("LAUNCH_PROFILE") if fields.len() == 5 => {
            if fields[3].is_empty()
                || fields[3].len() > 64
                || !fields[3]
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
            {
                return Err("profile ID is invalid".into());
            }
            if fields[4].is_empty()
                || fields[4].len() > 256
                || fields[4].starts_with('-')
                || fields[4]
                    .chars()
                    .any(|character| matches!(character, '\n' | '\r' | '\t'))
            {
                return Err("base ref is invalid".into());
            }
            let task_id =
                TaskId::parse(fields[2].to_string()).map_err(|error| error.to_string())?;
            Ok(Request::LaunchProfile {
                task_id,
                profile: fields[3].to_owned(),
                base_ref: fields[4].to_owned(),
            })
        }
        Some(command) => Err(format!("invalid {command} request")),
        None => Err("request command is missing".into()),
    }
}

fn parse_response(frame: &[u8]) -> Result<Response, String> {
    let text = std::str::from_utf8(frame).map_err(|_| "response is not UTF-8".to_string())?;
    let text = text.strip_suffix('\n').unwrap_or(text);
    let mut fields = text.split('\t');
    if fields.next() != Some(PROTOCOL) {
        return Err("unsupported protocol version".into());
    }
    match (fields.next(), fields.next()) {
        (Some("OK"), Some("STATUS")) if fields.next().is_none() => Ok(Response::Status),
        (Some("OK"), Some("STOP")) if fields.next().is_none() => Ok(Response::Stop),
        (Some("OK"), Some("RUN")) => Ok(Response::Run(fields.collect::<Vec<_>>().join("\t"))),
        (Some("OK"), Some("LAUNCH")) => Ok(Response::Launch(fields.collect::<Vec<_>>().join("\t"))),
        (Some("ERR"), Some(message)) => Ok(Response::Error(message.to_string())),
        (Some("OK"), Some("PENDING")) if fields.next().is_none() => Ok(Response::Pending),
        (Some("BUSY"), Some(message)) => Ok(Response::Busy(message.to_string())),
        _ => Err("malformed response".into()),
    }
}

fn read_frame(stream: &mut TcpStream) -> io::Result<Vec<u8>> {
    let mut frame = Vec::new();
    let mut byte = [0_u8; 1];
    loop {
        let count = stream.read(&mut byte)?;
        if count == 0 {
            if frame.is_empty() {
                return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "empty frame"));
            }
            break;
        }
        if byte[0] == b'\n' {
            break;
        }
        frame.push(byte[0]);
        if frame.len() >= MAX_FRAME_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "frame exceeds limit",
            ));
        }
    }
    Ok(frame)
}

fn write_endpoint(path: &Path, endpoint: &Endpoint) -> Result<(), DaemonError> {
    let temporary = path.with_extension(format!("tmp-{}", std::process::id()));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)?;
    writeln!(file, "version=1")?;
    writeln!(file, "pid={}", endpoint.pid)?;
    writeln!(file, "address={}", endpoint.address)?;
    file.sync_all()?;
    fs::rename(&temporary, path)?;
    Ok(())
}

fn parse_endpoint(bytes: &[u8]) -> Result<Endpoint, String> {
    let text = std::str::from_utf8(bytes).map_err(|_| "endpoint is not UTF-8".to_string())?;
    let mut version = None;
    let mut pid = None;
    let mut address = None;
    for line in text.lines() {
        let Some((key, value)) = line.split_once('=') else {
            return Err("endpoint line is malformed".into());
        };
        match key {
            "version" => {
                version = Some(
                    value
                        .parse::<u16>()
                        .map_err(|_| "invalid endpoint version")?,
                )
            }
            "pid" => pid = Some(value.parse::<u32>().map_err(|_| "invalid endpoint pid")?),
            "address" => {
                address = Some(
                    value
                        .parse::<SocketAddr>()
                        .map_err(|_| "invalid endpoint address")?,
                )
            }
            _ => return Err("endpoint contains an unknown field".into()),
        }
    }
    if version != Some(1) {
        return Err("unsupported endpoint version".into());
    }
    let address = address.ok_or_else(|| "endpoint address is missing".to_string())?;
    if !address.ip().is_loopback() {
        return Err("endpoint address is not loopback".into());
    }
    Ok(Endpoint {
        address,
        pid: pid.ok_or_else(|| "endpoint pid is missing".to_string())?,
    })
}

fn sanitize(value: &str) -> String {
    value.replace(['\r', '\n', '\t'], " ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protocol_rejects_non_loopback_and_unbounded_shapes() {
        assert!(parse_request(b"AFD1\tSTATUS").is_ok());
        assert!(parse_request(b"AFD1\tRUN\tbad/id\t/tmp/tool").is_err());
        assert!(parse_endpoint(b"version=1\npid=1\naddress=0.0.0.0:1\n").is_err());
    }

    #[test]
    fn endpoint_round_trip_is_versioned() {
        let endpoint = parse_endpoint(b"version=1\npid=42\naddress=127.0.0.1:43123\n").unwrap();
        assert_eq!(endpoint.pid, 42);
        assert_eq!(endpoint.address.port(), 43123);
        assert_eq!(parse_response(b"AFD1\tOK\tSTATUS\n"), Ok(Response::Status));
    }

    #[test]
    fn launch_protocol_round_trip_preserves_base_and_mode() {
        let executable = if cfg!(windows) {
            PathBuf::from(r"C:\agent.exe")
        } else {
            PathBuf::from("/tmp/agent")
        };
        let frame = format!("AFD1\tLAUNCH\ttask\t{}\tHEAD\n", executable.display());
        let request = parse_request(frame.as_bytes()).unwrap();
        assert!(matches!(
            request,
            Request::Launch {
                base_ref,
                executable: parsed_executable,
                ..
            } if base_ref == "HEAD" && parsed_executable == executable
        ));
        assert_eq!(
            parse_response(b"AFD1\tOK\tLAUNCH\ttask=task termination=Exited\n"),
            Ok(Response::Launch("task=task termination=Exited".into()))
        );
        assert!(parse_request(b"AFD1\tLAUNCH\ttask\t/tmp/agent\t-bad\n").is_err());
    }

    #[test]
    fn transport_loss_errors_are_stale_but_real_failures_are_not() {
        let root = Path::new("/project");
        for kind in [
            io::ErrorKind::InvalidInput,
            io::ErrorKind::ConnectionReset,
            io::ErrorKind::ConnectionRefused,
            io::ErrorKind::WouldBlock,
            io::ErrorKind::TimedOut,
        ] {
            assert!(
                matches!(
                    map_transport_error(root, io::Error::from(kind)),
                    DaemonError::StaleInstance(_)
                ),
                "{kind:?}"
            );
        }
        assert!(matches!(
            map_transport_error(root, io::Error::from(io::ErrorKind::PermissionDenied)),
            DaemonError::Io(_)
        ));
    }

    #[test]
    fn keepalive_and_busy_frames_round_trip() {
        assert_eq!(
            parse_response(b"AFD1\tOK\tPENDING\n"),
            Ok(Response::Pending)
        );
        assert_eq!(
            parse_response(b"AFD1\tBUSY\texecuting task T; retry after it finishes\n"),
            Ok(Response::Busy(
                "executing task T; retry after it finishes".into()
            ))
        );
        assert!(parse_response(b"AFD1\tOK\tPENDING\textra\n").is_err());
        assert!(parse_response(b"AFD1\tOK\tWAITING\n").is_err());
    }
}
