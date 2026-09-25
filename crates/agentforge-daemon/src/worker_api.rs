//! Authenticated worker API for workers on other machines (P4-M007, ADR-0050).
//!
//! `forged` serves protocol `AFW1` on a loopback address from `.forge/worker-api.conf`. It never
//! listens on a network interface: remote workers reach it through a GhostPort tunnel (Noise KK,
//! pinned keys), and every request also carries the worker's AgentForge secret, so each action is
//! authenticated as exactly one registered worker and limited to that worker's leases.
//!
//! One request per connection. A request is one line,
//! `AFW1\t<VERB>\t<worker-id>\t<secret>[\t<argument>...]\n`, with verbs `CLAIM`,
//! `RENEW <lease> <ttl-ms>`, and `RELEASE <lease>`. Responses are one line; a successful claim is
//! followed by the task contract document (its length is in the line).

use crate::{ExecutionSlot, SlotGuard};
use agentforge_adapter::MAX_TASK_PROMPT_BYTES;
use agentforge_operator::leases::{
    claim_lease_as, load_workers, now_ms, release_lease_as, renew_lease_as,
};
use agentforge_operator::secrets::{load_worker_secret, secrets_match};
use agentforge_operator::worker::next_claimable;
use std::fmt;
use std::fs;
use std::io::{self, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

/// Protocol identifier.
pub const WORKER_PROTOCOL: &str = "AFW1";
/// Project-relative worker API configuration.
pub const WORKER_API_CONFIG_RELATIVE_PATH: &str = ".forge/worker-api.conf";
const MAX_REQUEST_BYTES: usize = 1024;
const MAX_RESPONSE_LINE_BYTES: usize = 4 * 1024;
const IO_TIMEOUT: Duration = Duration::from_secs(5);
/// How long a worker waits for the coordinator to import a result (gates run there).
const IMPORT_TIMEOUT: Duration = Duration::from_secs(60 * 60);
/// Fixed delay before answering an unauthenticated request.
/// The longest a supervised worker waits between attempts to reach an unreachable coordinator
/// (P4-M010).
pub const MAX_UNREACHABLE_BACKOFF: Duration = Duration::from_secs(60);
const UNAUTHORIZED_DELAY: Duration = Duration::from_millis(200);

/// Reads `.forge/worker-api.conf`. `Ok(None)` means the worker API is off.
///
/// The only key is `bind=<address:port>`, and the address must be loopback: remote workers reach
/// it through GhostPort, never directly.
pub fn load_worker_api_bind(root: impl AsRef<Path>) -> Result<Option<SocketAddr>, String> {
    let path = root.as_ref().join(WORKER_API_CONFIG_RELATIVE_PATH);
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("{}: {error}", path.display())),
    };
    if text.len() > 1024 {
        return Err(format!("{}: file is too large", path.display()));
    }
    let mut bind = None;
    for line in text.lines().map(str::trim) {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        match line.split_once('=') {
            Some(("bind", value)) if bind.is_none() => {
                let address = value
                    .trim()
                    .parse::<SocketAddr>()
                    .map_err(|_| format!("{}: bind is not an address: {value}", path.display()))?;
                if !address.ip().is_loopback() {
                    return Err(format!(
                        "{}: bind must be a loopback address (expose it through GhostPort): \
                         {address}",
                        path.display()
                    ));
                }
                bind = Some(address);
            }
            _ => return Err(format!("{}: unexpected line: {line}", path.display())),
        }
    }
    bind.map(Some)
        .ok_or_else(|| format!("{}: bind is missing", path.display()))
}

/// A running worker API listener, stopped and joined on drop. `forged serve` starts one from
/// `.forge/worker-api.conf`; tests and embedders can start one directly.
pub struct WorkerApiServer {
    stop: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
    /// The bound address (useful when binding port 0).
    pub address: SocketAddr,
}

impl WorkerApiServer {
    /// Binds `bind` (which must be loopback) and serves the worker API for `root`, with its own
    /// execution slot (standalone use; `forged` shares its slot so imports never run beside an
    /// execution).
    pub fn start(root: &Path, bind: SocketAddr) -> io::Result<Self> {
        Self::start_with_slot(root, bind, Arc::new(Mutex::new(None)))
    }

    pub(crate) fn start_with_slot(
        root: &Path,
        bind: SocketAddr,
        slot: ExecutionSlot,
    ) -> io::Result<Self> {
        if !bind.ip().is_loopback() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "the worker API binds only loopback addresses",
            ));
        }
        let listener = TcpListener::bind(bind)?;
        listener.set_nonblocking(true)?;
        let address = listener.local_addr()?;
        let stop = Arc::new(AtomicBool::new(false));
        let (flag, root) = (Arc::clone(&stop), root.to_path_buf());
        let handle = thread::spawn(move || {
            while !flag.load(Ordering::SeqCst) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        // Each request on its own thread: an import runs gates and must not
                        // stall other workers' renewals.
                        let (root, slot) = (root.clone(), Arc::clone(&slot));
                        thread::spawn(move || handle_connection(&root, &slot, stream));
                    }
                    Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(25));
                    }
                    Err(error) => {
                        crate::log(format_args!("worker API accept failed: {error}"));
                        thread::sleep(Duration::from_millis(100));
                    }
                }
            }
        });
        Ok(Self {
            stop,
            handle: Some(handle),
            address,
        })
    }
}

impl Drop for WorkerApiServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

fn handle_connection(root: &Path, slot: &ExecutionSlot, mut stream: TcpStream) {
    if stream.set_nonblocking(false).is_err()
        || stream.set_read_timeout(Some(IO_TIMEOUT)).is_err()
        || stream.set_write_timeout(Some(IO_TIMEOUT)).is_err()
    {
        return;
    }
    let response = match read_line(&mut stream, MAX_REQUEST_BYTES) {
        Ok(line) => respond(root, slot, &line, &mut stream),
        Err(error) => Response::line(format!("ERR\t{}", clean(&error.to_string()))),
    };
    let _ = stream.write_all(&response.bytes);
    let _ = stream.flush();
}

struct Response {
    bytes: Vec<u8>,
}

impl Response {
    fn line(body: String) -> Self {
        Self {
            bytes: format!("{WORKER_PROTOCOL}\t{body}\n").into_bytes(),
        }
    }
}

fn respond(root: &Path, slot: &ExecutionSlot, line: &str, stream: &mut TcpStream) -> Response {
    let fields = line.split('\t').collect::<Vec<_>>();
    if fields.len() < 4 || fields[0] != WORKER_PROTOCOL {
        return Response::line("ERR\tmalformed request".into());
    }
    let (verb, worker, secret, arguments) = (fields[1], fields[2], fields[3], &fields[4..]);
    if !authenticated(root, worker, secret) {
        crate::log(format_args!(
            "worker API refused an unauthenticated {verb} request"
        ));
        thread::sleep(UNAUTHORIZED_DELAY);
        return Response::line("ERR\tunauthorized".into());
    }
    let remote = [("channel", "remote")];
    let actor = format!("worker:{worker}");
    let result = match (verb, arguments) {
        ("CLAIM", []) => return claim(root, worker),
        // Authenticated and side-effect free: proves the credentials and channel (P4-M009).
        ("PING", []) => return Response::line(format!("OK\tPONG\t{worker}")),
        ("RESULT", arguments) if arguments.len() == 8 => {
            return result(root, slot, worker, arguments, stream);
        }
        ("RENEW", [lease, ttl]) => match ttl.parse::<u64>() {
            Ok(ttl) => renew_lease_as(root, lease, ttl, now_ms(), &actor, Some(worker), &remote)
                .map(|lease| format!("OK\tRENEWED\t{}\t{}", lease.lease_id, lease.expires_at_ms)),
            Err(_) => return Response::line("ERR\tttl is not a number".into()),
        },
        ("RELEASE", [lease]) => {
            release_lease_as(root, lease, now_ms(), &actor, Some(worker), &remote)
                .map(|lease| format!("OK\tRELEASED\t{}", lease.lease_id))
        }
        _ => return Response::line("ERR\tunknown verb or wrong arguments".into()),
    };
    match result {
        Ok(body) => Response::line(body),
        Err(error) => Response::line(format!("ERR\t{}", clean(&error.to_string()))),
    }
}

fn authenticated(root: &Path, worker: &str, secret: &str) -> bool {
    let registered = load_workers(root).is_ok_and(|workers| {
        workers
            .iter()
            .any(|descriptor| descriptor.worker_id().as_str() == worker)
    });
    // Always compare, so timing does not reveal whether the worker is enrolled.
    let expected = match load_worker_secret(root, worker) {
        Ok(Some(expected)) if registered => Some(expected),
        _ => None,
    };
    let matches = secrets_match(expected.as_deref().unwrap_or(&"0".repeat(64)), secret);
    expected.is_some() && matches
}

fn claim(root: &Path, worker: &str) -> Response {
    let outcome = (|| -> Result<Option<(String, Vec<u8>)>, String> {
        let now = now_ms();
        let Some(lease) = next_claimable(root, worker, now).map_err(|error| error.to_string())?
        else {
            return Ok(None);
        };
        let base = agentforge_worktree::WorktreeManager::new(root)
            .and_then(|manager| manager.resolve_base("HEAD"))
            .map_err(|error| format!("cannot resolve the base commit: {error}"))?;
        let task = agentforge_state::TaskStore::load(
            &agentforge_state::FileTaskStore::for_project_root(root),
        )
        .map_err(|error| error.to_string())?
        .and_then(|graph| {
            graph
                .get(lease.task_id())
                .map(|record| record.task().clone())
        })
        .ok_or("leased task not found")?;
        // The coordinator vouches for the task's pre-execution approvals (P4-M008).
        let approved = agentforge_operator::approved_boundaries(root, lease.task_id())
            .map_err(|error| error.to_string())?;
        if let Some(missing) = task
            .required_approvals
            .iter()
            .find(|boundary| !boundary.is_post_execution() && !approved.contains(boundary))
        {
            return Err(format!(
                "task {} needs approval {} before it can run",
                lease.task_id(),
                missing.as_str()
            ));
        }
        let document = agentforge_adapter::render_task_prompt(&task);
        let claim = claim_lease_as(
            root,
            lease.lease_id().as_str(),
            worker,
            now,
            &[("channel", "remote"), ("base_commit", base.as_str())],
        )
        .map_err(|error| error.to_string())?;
        let header = format!(
            "OK\tCLAIMED\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            claim.lease_id.as_str(),
            claim.generation,
            lease.expires_at_ms(),
            lease.task_id(),
            base,
            document.len(),
            lease.expires_at_ms().saturating_sub(lease.issued_at_ms())
        );
        Ok(Some((header, document)))
    })();
    match outcome {
        Ok(None) => Response::line("OK\tNONE".into()),
        Ok(Some((header, document))) => {
            let mut response = Response::line(header);
            response.bytes.extend_from_slice(&document);
            response
        }
        Err(error) => Response::line(format!("ERR\t{}", clean(&error))),
    }
}

/// Maximum bundle size accepted by `RESULT`.
pub const MAX_RESULT_BUNDLE_BYTES: usize = 32 * 1024 * 1024;

/// Handles `RESULT <lease> <base> <head> <exit|none> <termination> <stdout-len> <stderr-len>
/// <bundle-len>` plus bodies: verifies ownership, takes the execution slot, imports at the exact
/// commit, runs gates locally, and releases the lease.
fn result(
    root: &Path,
    slot: &ExecutionSlot,
    worker: &str,
    arguments: &[&str],
    stream: &mut TcpStream,
) -> Response {
    let outcome = (|| -> Result<String, String> {
        let [
            lease_id,
            base,
            head,
            exit,
            termination,
            stdout_len,
            stderr_len,
            bundle_len,
        ] = arguments
        else {
            return Err("RESULT needs 8 arguments".into());
        };
        let limit = |value: &str, maximum: usize| {
            value
                .parse::<usize>()
                .ok()
                .filter(|length| *length <= maximum)
                .ok_or_else(|| format!("body length {value} is invalid or exceeds {maximum}"))
        };
        let stdout_len = limit(stdout_len, agentforge_orchestrator::MAX_REMOTE_LOG_BYTES)?;
        let stderr_len = limit(stderr_len, agentforge_orchestrator::MAX_REMOTE_LOG_BYTES)?;
        let bundle_len = limit(bundle_len, MAX_RESULT_BUNDLE_BYTES)?;
        let exit_code = match *exit {
            "none" => None,
            code => Some(code.parse::<i32>().map_err(|_| "exit is not a number")?),
        };
        let mut read = |length: usize| -> Result<Vec<u8>, String> {
            let mut bytes = vec![0_u8; length];
            stream
                .read_exact(&mut bytes)
                .map_err(|error| format!("body read failed: {error}"))?;
            Ok(bytes)
        };
        let stdout = read(stdout_len)?;
        let stderr = read(stderr_len)?;
        let bundle_bytes = read(bundle_len)?;

        // Only the lease's own worker may report its result.
        let book = agentforge_state::LeaseStore::load(
            &agentforge_state::FileLeaseStore::for_project_root(root),
        )
        .map_err(|error| error.to_string())?
        .unwrap_or_default();
        let parsed_lease = agentforge_core::remote::LeaseId::parse(*lease_id)
            .map_err(|error| error.to_string())?;
        let lease = book
            .get(&parsed_lease)
            .ok_or_else(|| format!("lease not found: {lease_id}"))?
            .clone();
        if lease.worker_id().as_str() != worker {
            return Err(format!(
                "lease {lease_id} belongs to worker {}, not {worker}",
                lease.worker_id().as_str()
            ));
        }
        {
            let mut holder = slot.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
            if let Some(current) = holder.as_ref() {
                return Err(format!("BUSY:{current}"));
            }
            *holder = Some(format!("(remote import {})", lease.task_id()));
        }
        let _guard = SlotGuard(Arc::clone(slot));

        let bundle = if bundle_bytes.is_empty() {
            None
        } else {
            let directory = root.join(".forge/remote-results");
            fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
            let path = directory.join(format!("{lease_id}-{}.bundle", std::process::id()));
            fs::write(&path, &bundle_bytes).map_err(|error| error.to_string())?;
            Some(path)
        };
        let remote = agentforge_orchestrator::RemoteResult {
            claim: agentforge_orchestrator::LeaseClaim {
                lease_id: parsed_lease,
                worker_id: lease.worker_id().clone(),
                generation: lease.generation(),
            },
            base_commit: (*base).to_owned(),
            head_commit: (*head).to_owned(),
            bundle: bundle.clone(),
            exit_code,
            termination: (*termination).to_owned(),
            stdout,
            stderr,
        };
        // The task snapshot exists (the lease names a task), so creating the log here is safe.
        let imported = agentforge_audit::FileAuditStore::open(root.join(".forge/audit.log"))
            .map_err(|error| error.to_string())
            .and_then(|mut audit| {
                agentforge_orchestrator::import_remote_result(
                    root,
                    &agentforge_state::FileTaskStore::for_project_root(root),
                    &mut audit,
                    lease.task_id(),
                    &remote,
                )
                .map_err(|error| error.to_string())
            });
        if let Some(path) = bundle {
            let _ = fs::remove_file(path);
        }
        let imported = imported?;
        release_lease_as(
            root,
            lease_id,
            now_ms(),
            &format!("worker:{worker}"),
            Some(worker),
            &[("channel", "remote")],
        )
        .map_err(|error| format!("imported, but releasing the lease failed: {error}"))?;
        let passed = imported.gates.iter().filter(|gate| gate.passed()).count();
        Ok(format!(
            "OK\tIMPORTED\t{}\t{}/{}",
            imported.state.as_str(),
            passed,
            imported.gates.len()
        ))
    })();
    match outcome {
        Ok(body) => Response::line(body),
        Err(error) if error.starts_with("BUSY:") => Response::line(format!(
            "BUSY\t{}",
            clean(error.trim_start_matches("BUSY:"))
        )),
        Err(error) => Response::line(format!("ERR\t{}", clean(&error))),
    }
}

fn clean(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_control() {
                ' '
            } else {
                character
            }
        })
        .take(512)
        .collect()
}

fn read_line(stream: &mut TcpStream, maximum: usize) -> io::Result<String> {
    let mut line = Vec::new();
    let mut byte = [0_u8; 1];
    loop {
        let count = match stream.read(&mut byte) {
            Ok(count) => count,
            // A signal interrupted the read; nothing was consumed, so retry (P5-M002).
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error),
        };
        if count == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "connection closed",
            ));
        }
        if byte[0] == b'\n' {
            break;
        }
        line.push(byte[0]);
        if line.len() > maximum {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "line exceeds limit",
            ));
        }
    }
    String::from_utf8(line).map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "not UTF-8"))
}

/// A lease claimed over the worker API, with the exact contract to run.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RemoteClaim {
    /// Claimed lease.
    pub lease_id: String,
    /// Lease generation.
    pub generation: u64,
    /// Lease expiry on the coordinator's clock.
    pub expires_at_ms: u64,
    /// Leased task.
    pub task_id: String,
    /// Exact base commit to start from.
    pub base_commit: String,
    /// The `agentforge-task-prompt-v1` contract document.
    pub contract: Vec<u8>,
    /// The lease window on the coordinator's clock; renew for this long (P4-M008).
    pub window_ms: u64,
}

/// Why a worker API request failed (P4-M010).
///
/// Only socket-level failures are [`ClientError::Unreachable`]; a supervised worker retries those.
/// A coordinator refusal is never retried silently.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ClientError {
    /// The worker API could not be reached, or the connection failed mid-request.
    Unreachable(String),
    /// The coordinator answered with an error, for example `unauthorized`.
    Refused(String),
    /// The coordinator's answer was malformed.
    Protocol(String),
}

impl fmt::Display for ClientError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unreachable(message) | Self::Refused(message) | Self::Protocol(message) => {
                formatter.write_str(message)
            }
        }
    }
}

impl std::error::Error for ClientError {}

impl From<ClientError> for String {
    fn from(error: ClientError) -> Self {
        error.to_string()
    }
}

/// Client for the worker API. It only connects to loopback endpoints: the plaintext protocol
/// must travel through a GhostPort tunnel, whose local listener is on loopback.
#[derive(Clone, Debug)]
pub struct WorkerClient {
    endpoint: SocketAddr,
    worker_id: String,
    secret: String,
}

impl WorkerClient {
    /// Creates a client for `endpoint` (`host:port`), which must resolve to a loopback address.
    pub fn new(endpoint: &str, worker_id: &str, secret: &str) -> Result<Self, String> {
        let endpoint = endpoint
            .to_socket_addrs()
            .map_err(|error| format!("invalid endpoint {endpoint}: {error}"))?
            .find(|address| address.ip().is_loopback())
            .ok_or_else(|| {
                format!(
                    "endpoint {endpoint} is not loopback; connect through the local GhostPort \
                     listener"
                )
            })?;
        if worker_id.contains(['\t', '\n']) || secret.contains(['\t', '\n']) {
            return Err("worker ID and secret must not contain tabs or newlines".into());
        }
        Ok(Self {
            endpoint,
            worker_id: worker_id.to_owned(),
            secret: secret.to_owned(),
        })
    }

    /// Claims the worker's next claimable lease. `Ok(None)` means nothing to claim.
    pub fn claim(&self) -> Result<Option<RemoteClaim>, ClientError> {
        let protocol = |message: &str| ClientError::Protocol(message.to_owned());
        let (fields, mut stream) = self.request("CLAIM", &[])?;
        match fields.as_slice() {
            [ok, none] if ok == "OK" && none == "NONE" => Ok(None),
            [
                ok,
                claimed,
                lease,
                generation,
                expires,
                task,
                base,
                length,
                window,
            ] if ok == "OK" && claimed == "CLAIMED" => {
                let length = length
                    .parse::<usize>()
                    .ok()
                    .filter(|length| *length <= MAX_TASK_PROMPT_BYTES)
                    .ok_or_else(|| protocol("contract length is invalid"))?;
                let mut contract = vec![0_u8; length];
                stream.read_exact(&mut contract).map_err(|error| {
                    ClientError::Unreachable(format!("contract read failed: {error}"))
                })?;
                Ok(Some(RemoteClaim {
                    lease_id: lease.clone(),
                    generation: generation.parse().map_err(|_| protocol("bad generation"))?,
                    expires_at_ms: expires.parse().map_err(|_| protocol("bad expiry"))?,
                    task_id: task.clone(),
                    base_commit: base.clone(),
                    contract,
                    window_ms: window.parse().map_err(|_| protocol("bad lease window"))?,
                }))
            }
            _ => Err(classify_response(&fields)),
        }
    }

    /// Proves the channel and credentials without changing anything (`PING`).
    pub fn ping(&self) -> Result<(), String> {
        let (fields, _) = self.request("PING", &[])?;
        match fields.as_slice() {
            [ok, pong, _] if ok == "OK" && pong == "PONG" => Ok(()),
            _ => Err(error_of(&fields)),
        }
    }

    /// Renews one of the worker's leases for `ttl_ms`, returning the new expiry.
    pub fn renew(&self, lease_id: &str, ttl_ms: u64) -> Result<u64, String> {
        let (fields, _) = self.request("RENEW", &[lease_id, &ttl_ms.to_string()])?;
        match fields.as_slice() {
            [ok, renewed, _, expires] if ok == "OK" && renewed == "RENEWED" => {
                expires.parse().map_err(|_| "bad expiry".into())
            }
            _ => Err(error_of(&fields)),
        }
    }

    /// Releases one of the worker's leases.
    pub fn release(&self, lease_id: &str) -> Result<(), String> {
        let (fields, _) = self.request("RELEASE", &[lease_id])?;
        match fields.as_slice() {
            [ok, released, _] if ok == "OK" && released == "RELEASED" => Ok(()),
            _ => Err(error_of(&fields)),
        }
    }

    /// Reports a finished task. `Ok(Err(holder))` means the coordinator is busy; retry later.
    #[allow(clippy::too_many_arguments)]
    pub fn result(
        &self,
        lease_id: &str,
        base: &str,
        head: &str,
        exit_code: Option<i32>,
        termination: &str,
        stdout: &[u8],
        stderr: &[u8],
        bundle: &[u8],
    ) -> Result<Result<ImportedResult, String>, String> {
        let exit = exit_code.map_or_else(|| "none".to_owned(), |code| code.to_string());
        let lengths = [stdout.len(), stderr.len(), bundle.len()].map(|length| length.to_string());
        let mut body = Vec::with_capacity(stdout.len() + stderr.len() + bundle.len());
        body.extend_from_slice(stdout);
        body.extend_from_slice(stderr);
        body.extend_from_slice(bundle);
        let (fields, _) = self.request_with_body(
            "RESULT",
            &[
                lease_id,
                base,
                head,
                &exit,
                termination,
                &lengths[0],
                &lengths[1],
                &lengths[2],
            ],
            &body,
            IMPORT_TIMEOUT,
        )?;
        match fields.as_slice() {
            [ok, imported, state, gates] if ok == "OK" && imported == "IMPORTED" => {
                Ok(Ok(ImportedResult {
                    state: state.clone(),
                    gates: gates.clone(),
                }))
            }
            [busy, holder] if busy == "BUSY" => Ok(Err(holder.clone())),
            _ => Err(error_of(&fields)),
        }
    }

    fn request(
        &self,
        verb: &str,
        arguments: &[&str],
    ) -> Result<(Vec<String>, TcpStream), ClientError> {
        self.request_with_body(verb, arguments, &[], IO_TIMEOUT)
    }

    fn request_with_body(
        &self,
        verb: &str,
        arguments: &[&str],
        body: &[u8],
        read_timeout: Duration,
    ) -> Result<(Vec<String>, TcpStream), ClientError> {
        if arguments
            .iter()
            .any(|argument| argument.contains(['\t', '\n']))
        {
            return Err(ClientError::Protocol(
                "arguments must not contain tabs or newlines".into(),
            ));
        }
        let unreachable = |message: String| ClientError::Unreachable(message);
        let mut stream =
            TcpStream::connect_timeout(&self.endpoint, IO_TIMEOUT).map_err(|error| {
                unreachable(format!(
                    "cannot reach worker API at {}: {error}",
                    self.endpoint
                ))
            })?;
        stream
            .set_read_timeout(Some(read_timeout))
            .and_then(|()| stream.set_write_timeout(Some(IO_TIMEOUT)))
            .map_err(|error| unreachable(error.to_string()))?;
        let mut line = format!(
            "{WORKER_PROTOCOL}\t{verb}\t{}\t{}",
            self.worker_id, self.secret
        );
        for argument in arguments {
            line.push('\t');
            line.push_str(argument);
        }
        line.push('\n');
        stream
            .write_all(line.as_bytes())
            .and_then(|()| stream.write_all(body))
            .map_err(|error| unreachable(format!("request failed: {error}")))?;
        let response = read_line(&mut stream, MAX_RESPONSE_LINE_BYTES).map_err(|error| {
            let message = format!("response failed: {error}");
            if error.kind() == io::ErrorKind::InvalidData {
                ClientError::Protocol(message)
            } else {
                unreachable(message)
            }
        })?;
        let mut fields = response.split('\t').map(str::to_owned);
        if fields.next().as_deref() != Some(WORKER_PROTOCOL) {
            return Err(ClientError::Protocol(
                "unexpected protocol in response".into(),
            ));
        }
        Ok((fields.collect(), stream))
    }
}

fn error_of(fields: &[String]) -> String {
    classify_response(fields).into()
}

fn classify_response(fields: &[String]) -> ClientError {
    match fields {
        [err, message] if err == "ERR" => ClientError::Refused(message.clone()),
        _ => ClientError::Protocol(format!("unexpected response: {}", fields.join(" "))),
    }
}

/// Where the worker API is configured for a project (for messages and docs).
#[must_use]
pub fn worker_api_config_path(root: impl AsRef<Path>) -> PathBuf {
    root.as_ref().join(WORKER_API_CONFIG_RELATIVE_PATH)
}

/// The coordinator's answer to a successful `RESULT`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImportedResult {
    /// The task's state after import (`running` awaiting review, or `failed`).
    pub state: String,
    /// Coordinator gate results as `passed/total`.
    pub gates: String,
}

/// How a remote worker runs.
#[derive(Clone, Debug)]
pub struct RemoteWorkerOptions {
    /// The worker host's clone of the project.
    pub repo: PathBuf,
    /// Run at most one task (or report idle once), then return.
    pub once: bool,
    /// Wait between polls when nothing is claimable, and between `BUSY` retries.
    pub poll: Duration,
    /// Cap for the retry delay while the coordinator is unreachable. The delay starts at `poll`
    /// and doubles per consecutive failure (P4-M010); [`MAX_UNREACHABLE_BACKOFF`] by default.
    pub max_backoff: Duration,
}

/// Progress reported by [`run_remote_worker`].
#[derive(Debug)]
pub enum RemoteReport<'a> {
    /// Nothing to claim right now.
    Idle,
    /// The coordinator could not be reached; the worker retries after `retry_in` (P4-M010).
    Unreachable {
        /// The transport failure.
        error: &'a str,
        /// Delay before the next attempt.
        retry_in: Duration,
    },
    /// The coordinator answered again after being unreachable.
    Reconnected,
    /// A lease was claimed.
    Claimed(&'a RemoteClaim),
    /// The agent finished.
    AgentFinished {
        /// Exit code, when it exited.
        exit_code: Option<i32>,
        /// Termination label.
        termination: &'a str,
    },
    /// The worker's changes were committed (or already were) at this head.
    Committed(&'a str),
    /// Lease renewals made while the agent ran (P4-M009, finding 13).
    Renewals {
        /// Successful renewals.
        renewed: usize,
        /// Renewal failures (the run continued).
        failures: &'a [String],
    },
    /// The coordinator was busy; the worker retries.
    Busy(&'a str),
    /// The coordinator imported the result.
    Imported(&'a ImportedResult),
    /// The result was rejected or could not be produced; the lease was released.
    Abandoned(&'a str),
    /// The finished attempt could not be archived off the task's names, so the next claim of the
    /// same task on this host may fail until it is cleaned up (finding 17).
    ArchiveFailed(&'a str),
}

/// Totals for one [`run_remote_worker`] call.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RemoteSummary {
    /// Claims processed.
    pub claimed: usize,
    /// Results imported.
    pub imported: usize,
    /// Claims abandoned (rejected, boundary violation, or local failure).
    pub abandoned: usize,
}

/// Runs a worker on another machine (P4-M008): claims over the channel, runs the agent in a managed
/// worktree of `options.repo` at the exact base, renews while it runs, commits in-bounds changes,
/// and returns the result as a git bundle for exact-SHA import on the coordinator.
pub fn run_remote_worker<A: agentforge_adapter::AgentAdapter>(
    client: &WorkerClient,
    adapter: &A,
    options: &RemoteWorkerOptions,
    mut report: impl FnMut(RemoteReport<'_>),
) -> Result<RemoteSummary, String> {
    let mut summary = RemoteSummary::default();
    let mut backoff = None::<Duration>;
    loop {
        let claimed = match client.claim() {
            Ok(claimed) => {
                if backoff.take().is_some() {
                    report(RemoteReport::Reconnected);
                }
                claimed
            }
            // A coordinator restart or a dropped tunnel is survivable; a refusal is not (P4-M010).
            Err(ClientError::Unreachable(error)) if !options.once => {
                let retry_in = backoff
                    .map_or(options.poll, |previous| previous.saturating_mul(2))
                    .min(options.max_backoff.max(options.poll));
                backoff = Some(retry_in);
                report(RemoteReport::Unreachable {
                    error: &error,
                    retry_in,
                });
                thread::sleep(retry_in);
                continue;
            }
            Err(error) => return Err(error.into()),
        };
        let Some(claim) = claimed else {
            report(RemoteReport::Idle);
            if options.once {
                return Ok(summary);
            }
            thread::sleep(options.poll);
            continue;
        };
        summary.claimed += 1;
        report(RemoteReport::Claimed(&claim));
        match run_claim(client, adapter, options, &claim, &mut report) {
            Ok(true) => summary.imported += 1,
            Ok(false) => summary.abandoned += 1,
            Err(reason) => {
                summary.abandoned += 1;
                let released = client.release(&claim.lease_id);
                let message = match released {
                    Ok(()) => format!("{reason}; lease released"),
                    Err(error) => format!("{reason}; releasing the lease failed: {error}"),
                };
                report(RemoteReport::Abandoned(&message));
            }
        }
        if options.once {
            return Ok(summary);
        }
    }
}

/// Runs one claim. `Ok(true)`: imported. `Ok(false)`: abandoned and already reported and released.
fn run_claim<A: agentforge_adapter::AgentAdapter>(
    client: &WorkerClient,
    adapter: &A,
    options: &RemoteWorkerOptions,
    claim: &RemoteClaim,
    report: &mut impl FnMut(RemoteReport<'_>),
) -> Result<bool, String> {
    let task = agentforge_adapter::parse_task_prompt(&claim.contract)
        .map_err(|error| format!("contract rejected: {error}"))?;
    if task.task_id != claim.task_id {
        return Err("contract does not match the claimed task".into());
    }
    let repo = &options.repo;
    let base_ref = format!("{}^{{commit}}", claim.base_commit);
    if run_git(repo, &["cat-file", "-e", &base_ref]).is_err() {
        let _ = run_git(repo, &["fetch", "--quiet"]);
        run_git(repo, &["cat-file", "-e", &base_ref]).map_err(|_| {
            format!(
                "base commit {} is not in {} (fetch it from the shared origin)",
                claim.base_commit,
                repo.display()
            )
        })?;
    }
    let task_id = agentforge_core::task::TaskId::parse(task.task_id.clone())
        .map_err(|error| error.to_string())?;
    let manager =
        agentforge_worktree::WorktreeManager::new(repo).map_err(|error| error.to_string())?;
    // Leftovers of an attempt that never finished here (the worker was stopped or killed mid-task)
    // would block this claim; archive them first (finding 17).
    archive_attempt(
        &manager,
        repo,
        &task_id,
        &format!("{}.stale", claim.lease_id),
    )
    .map_err(|error| format!("cannot archive a previous attempt: {error}"))?;
    let worktree = manager
        .create(&agentforge_worktree::WorktreeSpec::new(
            task_id.clone(),
            claim.base_commit.clone(),
        ))
        .map_err(|error| format!("cannot create the worktree: {error}"))?;
    let worktree_path = worktree.path().to_path_buf();

    let renewal = RemoteRenewal::start(client.clone(), &claim.lease_id, claim.window_ms);
    let approvals = task
        .required_approvals
        .iter()
        .copied()
        .filter(|boundary| !boundary.is_post_execution())
        .collect::<Vec<_>>();
    let execution = adapter.execute(agentforge_adapter::AdapterRequest {
        task: &task,
        worktrees: &manager,
        acknowledged_approvals: &approvals,
    });
    let outcome = (|| -> Result<bool, String> {
        let execution = execution.map_err(|error| format!("agent could not run: {error}"))?;
        let termination = match execution.termination() {
            agentforge_adapter::ExecutionTermination::Exited => "exited",
            agentforge_adapter::ExecutionTermination::TimedOut => "timed_out",
            agentforge_adapter::ExecutionTermination::OutputLimitExceeded => {
                "output_limit_exceeded"
            }
        };
        report(RemoteReport::AgentFinished {
            exit_code: execution.exit_code(),
            termination,
        });
        let changed = changed_paths(&worktree_path)?;
        if let Some(violation) = boundary_violation(&task, &changed) {
            return Err(format!("{violation}; nothing was sent"));
        }
        if !changed.is_empty() {
            run_git(&worktree_path, &["add", "-A"])?;
            run_git(
                &worktree_path,
                &[
                    "commit",
                    "-q",
                    "-m",
                    &format!("agentforge: remote result for {}", task.task_id),
                ],
            )?;
        }
        let head = run_git(&worktree_path, &["rev-parse", "HEAD"])?;
        report(RemoteReport::Committed(&head));
        let bundle = if head == claim.base_commit {
            Vec::new()
        } else {
            let directory = repo.join(".forge/remote-results");
            fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
            let path = directory.join(format!("{}.bundle", claim.lease_id));
            let branch = format!("{}..agentforge/task/{}", claim.base_commit, task.task_id);
            run_git(
                repo,
                &["bundle", "create", "-q", &path.to_string_lossy(), &branch],
            )?;
            let bytes = fs::read(&path).map_err(|error| error.to_string())?;
            let _ = fs::remove_file(&path);
            bytes
        };
        let bound = |bytes: &[u8]| {
            bytes[..bytes
                .len()
                .min(agentforge_orchestrator::MAX_REMOTE_LOG_BYTES)]
                .to_vec()
        };
        let (stdout, stderr) = (bound(execution.stdout()), bound(execution.stderr()));
        loop {
            match client.result(
                &claim.lease_id,
                &claim.base_commit,
                &head,
                execution.exit_code(),
                termination,
                &stdout,
                &stderr,
                &bundle,
            )? {
                Ok(imported) => {
                    report(RemoteReport::Imported(&imported));
                    return Ok(true);
                }
                Err(holder) => {
                    report(RemoteReport::Busy(&holder));
                    thread::sleep(options.poll);
                }
            }
        }
    })();
    let (renewed, failures) = renewal.stop();
    report(RemoteReport::Renewals {
        renewed,
        failures: &failures,
    });
    if let Err(error) = archive_attempt(&manager, repo, &task_id, &claim.lease_id) {
        report(RemoteReport::ArchiveFailed(&error));
    }
    outcome
}

/// Moves one attempt off the task's names so the task can be leased to this host again
/// (finding 17). A clean worktree is retired; one that cannot be (dirty) is moved to
/// `.forge/remote-abandoned/<name>`. The task branch becomes `agentforge/remote/<name>`. Names come
/// from unique lease IDs, so every attempt keeps its exact history. Does nothing when the task's
/// names are free.
fn archive_attempt(
    manager: &agentforge_worktree::WorktreeManager,
    repo: &Path,
    task_id: &agentforge_core::task::TaskId,
    name: &str,
) -> Result<(), String> {
    if let Some(status) = manager
        .inspect(task_id)
        .map_err(|error| error.to_string())?
    {
        if manager.retire(task_id).is_err() {
            let kept = repo.join(".forge/remote-abandoned").join(name);
            if let Some(parent) = kept.parent() {
                fs::create_dir_all(parent).map_err(|error| error.to_string())?;
            }
            run_git(
                repo,
                &[
                    "worktree",
                    "move",
                    &status.path().to_string_lossy(),
                    &kept.to_string_lossy(),
                ],
            )?;
        }
    }
    let branch = format!("agentforge/task/{task_id}");
    if run_git(
        repo,
        &[
            "show-ref",
            "--verify",
            "--quiet",
            &format!("refs/heads/{branch}"),
        ],
    )
    .is_ok()
    {
        run_git(
            repo,
            &[
                "branch",
                "-m",
                &branch,
                &format!("agentforge/remote/{name}"),
            ],
        )?;
    }
    Ok(())
}

/// Paths changed in a worktree (tracked and untracked), from `git status`.
fn changed_paths(worktree: &Path) -> Result<Vec<String>, String> {
    let status = run_git(
        worktree,
        &[
            "status",
            "--porcelain=v1",
            "--untracked-files=all",
            "--no-renames",
        ],
    )?;
    Ok(status
        .lines()
        .filter(|line| line.len() > 3)
        .map(|line| line[3..].trim_matches('"').to_owned())
        .collect())
}

/// The first changed path outside the contract's boundary, if any.
fn boundary_violation(
    task: &agentforge_core::agent::AgentTask,
    paths: &[String],
) -> Option<String> {
    let inside = |path: &str, scope: &String| {
        let scope = scope.trim_matches('/');
        path == scope
            || path
                .strip_prefix(scope)
                .is_some_and(|rest| rest.starts_with('/'))
    };
    paths.iter().find_map(|path| {
        if task.forbidden_paths.iter().any(|scope| inside(path, scope)) {
            Some(format!("{path} is a forbidden path"))
        } else if !task.allowed_paths.is_empty()
            && !task.allowed_paths.iter().any(|scope| inside(path, scope))
        {
            Some(format!("{path} is outside the task's allowed paths"))
        } else {
            None
        }
    })
}

fn run_git(directory: &Path, arguments: &[&str]) -> Result<String, String> {
    let output = std::process::Command::new("git")
        .current_dir(directory)
        .args(arguments)
        .output()
        .map_err(|error| format!("git {}: {error}", arguments[0]))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
    } else {
        Err(format!(
            "git {} failed: {}",
            arguments[0],
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}

/// Renews a claimed lease over the channel every third of its window until stopped.
struct RemoteRenewal {
    stop: Arc<AtomicBool>,
    handle: JoinHandle<(usize, Vec<String>)>,
}

impl RemoteRenewal {
    fn start(client: WorkerClient, lease_id: &str, window_ms: u64) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&stop);
        let lease_id = lease_id.to_owned();
        let handle = thread::spawn(move || {
            let interval = Duration::from_millis((window_ms / 3).max(20));
            let mut next = std::time::Instant::now() + interval;
            let mut renewed = 0;
            while !flag.load(Ordering::SeqCst) {
                thread::sleep(Duration::from_millis(10));
                if std::time::Instant::now() < next {
                    continue;
                }
                next = std::time::Instant::now() + interval;
                match client.renew(&lease_id, window_ms) {
                    Ok(_) => renewed += 1,
                    // Keep running: the claim and task already authorize the run.
                    Err(error) => return (renewed, vec![error]),
                }
            }
            (renewed, Vec::new())
        });
        Self { stop, handle }
    }

    fn stop(self) -> (usize, Vec<String>) {
        self.stop.store(true, Ordering::SeqCst);
        self.handle
            .join()
            .unwrap_or_else(|_| (0, vec!["renewal thread panicked".to_owned()]))
    }
}

#[cfg(test)]
mod tests {
    use super::{WorkerApiServer, WorkerClient};
    use std::sync::{Arc, Mutex};

    #[test]
    fn result_answers_busy_while_an_execution_holds_the_slot() {
        let root =
            std::env::temp_dir().join(format!("agentforge-worker-api-busy-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join(".forge/workers")).expect("root");
        let git = |args: &[&str]| {
            let output = std::process::Command::new("git")
                .current_dir(&root)
                .args(args)
                .output()
                .expect("git");
            assert!(output.status.success(), "{output:?}");
            String::from_utf8(output.stdout)
                .expect("utf8")
                .trim()
                .to_owned()
        };
        git(&["init", "-q"]);
        git(&["config", "user.name", "T"]);
        git(&["config", "user.email", "t@example.invalid"]);
        git(&["commit", "-q", "--allow-empty", "-m", "base"]);
        let base = git(&["rev-parse", "HEAD"]);
        let mut task = agentforge_core::agent::AgentTask::new(
            "P4-M008-T0001",
            "P4-M008",
            agentforge_core::agent::AgentRole::Implementer,
            "busy fixture",
        );
        task.allowed_paths = vec!["src".into()];
        agentforge_state::TaskStore::save(
            &agentforge_state::FileTaskStore::for_project_root(&root),
            &agentforge_core::task::TaskGraph::from_tasks([task]).expect("graph"),
        )
        .expect("snapshot");
        std::fs::write(
            root.join(".forge/workers/remote-a.conf"),
            "platform=linux-x86_64\ncapability=rust\nmax_leases=1\n",
        )
        .expect("worker");
        let secret = "d".repeat(64);
        let path = agentforge_operator::secrets::worker_secret_path(&root, "remote-a");
        std::fs::write(&path, format!("{secret}\n")).expect("secret");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).expect("chmod");
        }
        agentforge_operator::leases::grant_lease(
            &root,
            &agentforge_core::task::TaskId::parse("P4-M008-T0001").expect("id"),
            Some("remote-a"),
            60_000,
            agentforge_operator::leases::now_ms(),
            "op",
        )
        .expect("grant");

        let slot = Arc::new(Mutex::new(Some("P4-M008-T0009".to_owned())));
        let server = WorkerApiServer::start_with_slot(
            &root,
            "127.0.0.1:0".parse().expect("addr"),
            Arc::clone(&slot),
        )
        .expect("server");
        let client =
            WorkerClient::new(&server.address.to_string(), "remote-a", &secret).expect("client");
        let busy = client
            .result(
                "P4-M008-T0001.L1",
                &base,
                &base,
                Some(0),
                "exited",
                b"",
                b"",
                b"",
            )
            .expect("answer");
        assert_eq!(
            busy,
            Err("P4-M008-T0009".to_owned()),
            "busy while executing"
        );

        // Once free, the no-change result is imported (the task fails with "no changes").
        *slot.lock().expect("slot") = None;
        let imported = client
            .result(
                "P4-M008-T0001.L1",
                &base,
                &base,
                Some(0),
                "exited",
                b"",
                b"",
                b"",
            )
            .expect("answer")
            .expect("imported");
        assert_eq!(imported.state, "failed");
        assert_eq!(
            *slot.lock().expect("slot"),
            None,
            "the import frees the slot"
        );
        drop(server);
        let _ = std::fs::remove_dir_all(root);
    }
}

/// Severity of one worker-host check.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DoctorLevel {
    /// The requirement is met.
    Ok,
    /// The host works, but something is likely wrong.
    Warn,
    /// The host must not run tasks until fixed.
    Fail,
}

impl DoctorLevel {
    /// Stable label (`ok`, `warn`, `fail`).
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Warn => "warn",
            Self::Fail => "fail",
        }
    }
}

/// One worker-host check with its outcome and, when not ok, how to fix it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DoctorCheck {
    /// Check name (`repo`, `git-identity`, `hooks`, `agent`, `secret`, `endpoint`, `origin`).
    pub name: &'static str,
    /// Outcome.
    pub level: DoctorLevel,
    /// What was found.
    pub detail: String,
    /// How to fix it (empty when ok).
    pub fix: String,
}

impl DoctorCheck {
    fn ok(name: &'static str, detail: impl Into<String>) -> Self {
        Self {
            name,
            level: DoctorLevel::Ok,
            detail: detail.into(),
            fix: String::new(),
        }
    }

    fn not_ok(
        name: &'static str,
        level: DoctorLevel,
        detail: impl Into<String>,
        fix: impl Into<String>,
    ) -> Self {
        Self {
            name,
            level,
            detail: detail.into(),
            fix: fix.into(),
        }
    }
}

/// What the doctor checks on a worker host.
#[derive(Clone, Debug)]
pub struct DoctorOptions {
    /// Coordinator endpoint (the local end of the GhostPort tunnel).
    pub endpoint: String,
    /// Registered worker ID.
    pub worker_id: String,
    /// The worker's secret file.
    pub secret_file: PathBuf,
    /// The worker host's clone.
    pub repo: PathBuf,
    /// The agent executable.
    pub agent_executable: PathBuf,
    /// The agent's arguments (from its profile).
    pub agent_arguments: Vec<String>,
}

/// Checks everything a worker host needs before it runs tasks (P4-M009, finding 14).
#[must_use]
pub fn remote_doctor(options: &DoctorOptions) -> Vec<DoctorCheck> {
    let repo = &options.repo;
    let mut checks = Vec::new();
    let repo_ok = run_git(repo, &["rev-parse", "--verify", "HEAD^{commit}"]).is_ok();
    checks.push(if repo_ok {
        DoctorCheck::ok(
            "repo",
            format!("{} is a Git clone with commits", repo.display()),
        )
    } else {
        DoctorCheck::not_ok(
            "repo",
            DoctorLevel::Fail,
            format!(
                "{} is not a Git repository with at least one commit",
                repo.display()
            ),
            "clone the project here (git clone <origin> <repo>)",
        )
    });
    if repo_ok {
        let name = run_git(repo, &["config", "user.name"]).unwrap_or_default();
        let email = run_git(repo, &["config", "user.email"]).unwrap_or_default();
        checks.push(if name.is_empty() || email.is_empty() {
            DoctorCheck::not_ok(
                "git-identity",
                DoctorLevel::Fail,
                "user.name or user.email is not set; remote results are committed in this clone",
                "git -C <repo> config user.name '<name>' && git -C <repo> config user.email '<email>'",
            )
        } else {
            DoctorCheck::ok("git-identity", format!("{name} <{email}>"))
        });
        checks.push(hooks_check(repo));
        let remotes = run_git(repo, &["remote"]).unwrap_or_default();
        checks.push(if remotes.is_empty() {
            DoctorCheck::not_ok(
                "origin",
                DoctorLevel::Warn,
                "the clone has no remote; a missing base commit cannot be fetched",
                "git -C <repo> remote add origin <url>",
            )
        } else {
            DoctorCheck::ok(
                "origin",
                format!("remotes: {}", remotes.replace('\n', ", ")),
            )
        });
    }
    checks.push(agent_check(options));
    let secret = agentforge_operator::secrets::read_secret_file(&options.secret_file);
    checks.push(match &secret {
        Ok(_) => DoctorCheck::ok(
            "secret",
            format!("{} is valid", options.secret_file.display()),
        ),
        Err(error) => DoctorCheck::not_ok(
            "secret",
            DoctorLevel::Fail,
            error.to_string(),
            "copy the secret from `forge worker enroll` into the file and chmod 600 it",
        ),
    });
    checks.push(match secret {
        Err(_) => DoctorCheck::not_ok(
            "endpoint",
            DoctorLevel::Fail,
            "not checked: the secret is unusable",
            "fix the secret first",
        ),
        Ok(secret) => match WorkerClient::new(&options.endpoint, &options.worker_id, &secret)
            .and_then(|client| client.ping())
        {
            Ok(()) => DoctorCheck::ok(
                "endpoint",
                format!(
                    "{} authenticates as {}",
                    options.endpoint, options.worker_id
                ),
            ),
            Err(error) => DoctorCheck::not_ok(
                "endpoint",
                DoctorLevel::Fail,
                error,
                "check the GhostPort client (ghostport status), the coordinator's forged worker \
                 API, and that this worker is registered and enrolled with this secret",
            ),
        },
    });
    checks
}

fn hooks_check(repo: &Path) -> DoctorCheck {
    let tracked = run_git(
        repo,
        &["ls-files", "--error-unmatch", ".githooks/pre-commit"],
    )
    .is_ok();
    if tracked {
        let hooks_path = run_git(repo, &["config", "core.hooksPath"]).unwrap_or_default();
        return if hooks_path == ".githooks" {
            DoctorCheck::ok(
                "hooks",
                "core.hooksPath=.githooks (the project pre-commit gate runs)",
            )
        } else {
            DoctorCheck::not_ok(
                "hooks",
                DoctorLevel::Fail,
                "the project tracks .githooks/pre-commit but core.hooksPath is not .githooks, so \
                 the agent's pre-commit gate would silently not run",
                "run ./scripts/install-hooks in the clone (or git config core.hooksPath .githooks)",
            )
        };
    }
    let git_dir =
        run_git(repo, &["rev-parse", "--git-path", "hooks/pre-commit"]).unwrap_or_default();
    if !git_dir.is_empty() && repo.join(&git_dir).is_file() {
        DoctorCheck::ok("hooks", format!("{git_dir} is installed"))
    } else {
        DoctorCheck::not_ok(
            "hooks",
            DoctorLevel::Warn,
            "no pre-commit hook; only the coordinator's gates will check results",
            "install the project's pre-commit hook in the clone",
        )
    }
}

fn agent_check(options: &DoctorOptions) -> DoctorCheck {
    let executable = &options.agent_executable;
    if !executable.is_file() {
        return DoctorCheck::not_ok(
            "agent",
            DoctorLevel::Fail,
            format!("agent executable {} does not exist", executable.display()),
            "fix the agent profile's executable= path on this host",
        );
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if fs::metadata(executable).map_or(true, |meta| meta.permissions().mode() & 0o111 == 0) {
            return DoctorCheck::not_ok(
                "agent",
                DoctorLevel::Fail,
                format!(
                    "agent executable {} is not executable",
                    executable.display()
                ),
                "chmod +x it, or point the profile at the right binary",
            );
        }
    }
    let repo = fs::canonicalize(&options.repo).unwrap_or_else(|_| options.repo.clone());
    let mut outside = Vec::new();
    for argument in &options.agent_arguments {
        let path = Path::new(argument);
        if !path.is_absolute() {
            continue;
        }
        if !path.exists() {
            return DoctorCheck::not_ok(
                "agent",
                DoctorLevel::Fail,
                format!("agent argument {argument} does not exist on this host"),
                "profiles hold absolute paths; rewrite them for this host (for example the bridge \
                 script inside this clone)",
            );
        }
        // An argument inside a *different* Git checkout (for example the coordinator's bridge
        // script) means the profile was copied from another host or clone.
        let directory = if path.is_dir() {
            path
        } else {
            path.parent().unwrap_or(path)
        };
        if let Ok(toplevel) = run_git(directory, &["rev-parse", "--show-toplevel"]) {
            let toplevel = fs::canonicalize(&toplevel).unwrap_or_else(|_| PathBuf::from(&toplevel));
            if toplevel != repo {
                outside.push(format!("{argument} (in {})", toplevel.display()));
            }
        }
    }
    if outside.is_empty() {
        DoctorCheck::ok("agent", format!("{} is ready", executable.display()))
    } else {
        DoctorCheck::not_ok(
            "agent",
            DoctorLevel::Warn,
            format!(
                "profile points into another checkout: {}; copied from another host or clone?",
                outside.join(", ")
            ),
            "point project scripts (such as the bridge) at this clone's copy",
        )
    }
}
