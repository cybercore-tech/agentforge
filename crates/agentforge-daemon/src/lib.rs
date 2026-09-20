//! Bounded, local-only orchestration daemon protocol and lifecycle.

use agentforge_adapter::{ProcessAdapter, ProcessAdapterConfig};
use agentforge_audit::FileAuditStore;
use agentforge_core::task::TaskId;
use agentforge_operator::approved_boundaries;
use agentforge_orchestrator::execute_process_persisted;
use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::net::{IpAddr, SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::time::Duration;

const PROTOCOL: &str = "AFD1";
const MAX_FRAME_BYTES: usize = 16 * 1024;
const MAX_RESPONSE_BYTES: usize = 16 * 1024;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(2);
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
    match send_request(
        root.as_ref(),
        Request::Run {
            task_id: task_id.clone(),
            executable: executable.to_path_buf(),
        },
    )? {
        Response::Run(message) => Ok(message),
        Response::Status => Err(DaemonError::Protocol("unexpected run response".into())),
        Response::Stop => Err(DaemonError::Protocol("unexpected run response".into())),
        Response::Error(message) => Err(DaemonError::Execution(message)),
    }
}

/// Requests a cooperative daemon stop.
pub fn stop(root: impl AsRef<Path>) -> Result<(), DaemonError> {
    match send_request(root.as_ref(), Request::Stop)? {
        Response::Stop => Ok(()),
        _ => Err(DaemonError::Protocol("unexpected stop response".into())),
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
    let mut stream =
        TcpStream::connect_timeout(&endpoint.address, CONNECT_TIMEOUT).map_err(|error| {
            if error.kind() == io::ErrorKind::ConnectionRefused
                || error.kind() == io::ErrorKind::TimedOut
                || error.kind() == io::ErrorKind::NotFound
            {
                DaemonError::StaleInstance(daemon_paths(root).0)
            } else {
                DaemonError::Io(error)
            }
        })?;
    stream.set_read_timeout(Some(CONNECT_TIMEOUT))?;
    stream.set_write_timeout(Some(CONNECT_TIMEOUT))?;
    let frame = request.encode();
    stream.write_all(frame.as_bytes())?;
    stream.flush()?;
    let response = read_frame(&mut stream)?;
    parse_response(&response).map_err(DaemonError::Protocol)
}

struct Server {
    root: PathBuf,
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
            let request = match read_frame(&mut stream).and_then(|frame| {
                parse_request(&frame)
                    .map_err(|reason| io::Error::new(io::ErrorKind::InvalidData, reason))
            }) {
                Ok(request) => request,
                Err(error) => {
                    write_response(&mut stream, &Response::Error(error.to_string()))?;
                    continue;
                }
            };
            let stop = matches!(request, Request::Stop);
            let response = self.handle(request);
            write_response(&mut stream, &response)?;
            if stop {
                return Ok(());
            }
        }
        Ok(())
    }

    fn handle(&self, request: Request) -> Response {
        match request {
            Request::Status => Response::Status,
            Request::Stop => Response::Stop,
            Request::Run {
                task_id,
                executable,
            } => match execute_task(&self.root, &task_id, &executable) {
                Ok(message) => Response::Run(message),
                Err(error) => Response::Error(error.to_string()),
            },
        }
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
    let task_store = agentforge_state::FileTaskStore::for_project_root(root);
    let audit_directory = root.join(".forge");
    fs::create_dir_all(&audit_directory)?;
    let mut audit_store = FileAuditStore::open(audit_directory.join("audit.log"))
        .map_err(|error| DaemonError::Execution(error.to_string()))?;
    let approvals = approved_boundaries(root, task_id)
        .map_err(|error| DaemonError::Execution(error.to_string()))?;
    let adapter = ProcessAdapter::new(ProcessAdapterConfig::new("daemon-process", executable))
        .map_err(|error| DaemonError::Execution(error.to_string()))?;
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
        "task={} termination={:?}",
        execution.report.task_id(),
        execution.report.termination()
    ))
}

#[derive(Debug)]
enum Request {
    Status,
    Stop,
    Run {
        task_id: TaskId,
        executable: PathBuf,
    },
}

impl Request {
    fn encode(&self) -> String {
        match self {
            Self::Status => format!("{PROTOCOL}\tSTATUS\n"),
            Self::Stop => format!("{PROTOCOL}\tSTOP\n"),
            Self::Run {
                task_id,
                executable,
            } => format!("{PROTOCOL}\tRUN\t{task_id}\t{}\n", executable.display()),
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
enum Response {
    Status,
    Stop,
    Run(String),
    Error(String),
}

fn write_response(stream: &mut TcpStream, response: &Response) -> io::Result<()> {
    let value = match response {
        Response::Status => format!("{PROTOCOL}\tOK\tSTATUS\n"),
        Response::Stop => format!("{PROTOCOL}\tOK\tSTOP\n"),
        Response::Run(message) => format!("{PROTOCOL}\tOK\tRUN\t{}\n", sanitize(message)),
        Response::Error(message) => format!("{PROTOCOL}\tERR\t{}\n", sanitize(message)),
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
        (Some("ERR"), Some(message)) => Ok(Response::Error(message.to_string())),
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
}
