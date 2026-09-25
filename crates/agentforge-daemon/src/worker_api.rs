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

use agentforge_adapter::MAX_TASK_PROMPT_BYTES;
use agentforge_operator::leases::{
    claim_lease_as, load_workers, now_ms, release_lease_as, renew_lease_as,
};
use agentforge_operator::secrets::{load_worker_secret, secrets_match};
use agentforge_operator::worker::next_claimable;
use std::fs;
use std::io::{self, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::Duration;

/// Protocol identifier.
pub const WORKER_PROTOCOL: &str = "AFW1";
/// Project-relative worker API configuration.
pub const WORKER_API_CONFIG_RELATIVE_PATH: &str = ".forge/worker-api.conf";
const MAX_REQUEST_BYTES: usize = 1024;
const MAX_RESPONSE_LINE_BYTES: usize = 4 * 1024;
const IO_TIMEOUT: Duration = Duration::from_secs(5);
/// Fixed delay before answering an unauthenticated request.
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
    /// Binds `bind` (which must be loopback) and serves the worker API for `root`.
    pub fn start(root: &Path, bind: SocketAddr) -> io::Result<Self> {
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
                    Ok((stream, _)) => handle_connection(&root, stream),
                    Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(25));
                    }
                    Err(error) => {
                        eprintln!("forged: worker API accept failed: {error}");
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

fn handle_connection(root: &Path, mut stream: TcpStream) {
    if stream.set_nonblocking(false).is_err()
        || stream.set_read_timeout(Some(IO_TIMEOUT)).is_err()
        || stream.set_write_timeout(Some(IO_TIMEOUT)).is_err()
    {
        return;
    }
    let response = match read_line(&mut stream, MAX_REQUEST_BYTES) {
        Ok(line) => respond(root, &line),
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

fn respond(root: &Path, line: &str) -> Response {
    let fields = line.split('\t').collect::<Vec<_>>();
    if fields.len() < 4 || fields[0] != WORKER_PROTOCOL {
        return Response::line("ERR\tmalformed request".into());
    }
    let (verb, worker, secret, arguments) = (fields[1], fields[2], fields[3], &fields[4..]);
    if !authenticated(root, worker, secret) {
        eprintln!("forged: worker API refused an unauthenticated {verb} request");
        thread::sleep(UNAUTHORIZED_DELAY);
        return Response::line("ERR\tunauthorized".into());
    }
    let remote = [("channel", "remote")];
    let actor = format!("worker:{worker}");
    let result = match (verb, arguments) {
        ("CLAIM", []) => return claim(root, worker),
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
            "OK\tCLAIMED\t{}\t{}\t{}\t{}\t{}\t{}",
            claim.lease_id.as_str(),
            claim.generation,
            lease.expires_at_ms(),
            lease.task_id(),
            base,
            document.len()
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
        if stream.read(&mut byte)? == 0 {
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
    pub fn claim(&self) -> Result<Option<RemoteClaim>, String> {
        let (fields, mut stream) = self.request("CLAIM", &[])?;
        match fields.as_slice() {
            [ok, none] if ok == "OK" && none == "NONE" => Ok(None),
            [ok, claimed, lease, generation, expires, task, base, length]
                if ok == "OK" && claimed == "CLAIMED" =>
            {
                let length = length
                    .parse::<usize>()
                    .ok()
                    .filter(|length| *length <= MAX_TASK_PROMPT_BYTES)
                    .ok_or("contract length is invalid")?;
                let mut contract = vec![0_u8; length];
                stream
                    .read_exact(&mut contract)
                    .map_err(|error| format!("contract read failed: {error}"))?;
                Ok(Some(RemoteClaim {
                    lease_id: lease.clone(),
                    generation: generation.parse().map_err(|_| "bad generation")?,
                    expires_at_ms: expires.parse().map_err(|_| "bad expiry")?,
                    task_id: task.clone(),
                    base_commit: base.clone(),
                    contract,
                }))
            }
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

    fn request(&self, verb: &str, arguments: &[&str]) -> Result<(Vec<String>, TcpStream), String> {
        if arguments
            .iter()
            .any(|argument| argument.contains(['\t', '\n']))
        {
            return Err("arguments must not contain tabs or newlines".into());
        }
        let mut stream = TcpStream::connect_timeout(&self.endpoint, IO_TIMEOUT)
            .map_err(|error| format!("cannot reach worker API at {}: {error}", self.endpoint))?;
        stream
            .set_read_timeout(Some(IO_TIMEOUT))
            .and_then(|()| stream.set_write_timeout(Some(IO_TIMEOUT)))
            .map_err(|error| error.to_string())?;
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
            .map_err(|error| format!("request failed: {error}"))?;
        let response = read_line(&mut stream, MAX_RESPONSE_LINE_BYTES)
            .map_err(|error| format!("response failed: {error}"))?;
        let mut fields = response.split('\t').map(str::to_owned);
        if fields.next().as_deref() != Some(WORKER_PROTOCOL) {
            return Err("unexpected protocol in response".into());
        }
        Ok((fields.collect(), stream))
    }
}

fn error_of(fields: &[String]) -> String {
    match fields {
        [err, message] if err == "ERR" => message.clone(),
        _ => format!("unexpected response: {}", fields.join(" ")),
    }
}

/// Where the worker API is configured for a project (for messages and docs).
#[must_use]
pub fn worker_api_config_path(root: impl AsRef<Path>) -> PathBuf {
    root.as_ref().join(WORKER_API_CONFIG_RELATIVE_PATH)
}
