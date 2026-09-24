//! Operator lease operations for remote workers (P4-M004).
//!
//! Workers are registered as reviewed `.forge/workers/<worker-id>.conf` profiles. Leases are
//! granted, renewed, released, and expired only through these functions, under an exclusive lease
//! lock, and every change is recorded as a `LeaseRecorded` audit event. Until an authenticated
//! transport exists, the operator acts for the worker when renewing or releasing a lease.

use crate::{OperatorError, load_graph, next_sequence, validate_actor};
use agentforge_audit::{AuditEvent, AuditEventKind, AuditStore, FileAuditStore};
use agentforge_core::remote::{
    LeaseBook, LeaseId, LeaseState, MAX_LEASE_DURATION_MS, RemoteWorkerDescriptor, RemoteWorkerId,
    TaskLease, WorkerCapability,
};
use agentforge_core::task::{TaskGraph, TaskId, TaskState};
use agentforge_orchestrator::{first_path_overlap, remote_lease_conflict};
use agentforge_scheduler::{RemoteDispatchRequest, plan_remote_dispatch};
use agentforge_state::{FileLeaseStore, LeaseStore};
use std::fs::{self, OpenOptions};
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};

/// Project-relative directory holding worker profiles.
pub const WORKER_PROFILE_RELATIVE_PATH: &str = ".forge/workers";
/// Project-relative lease lock path.
pub const LEASE_LOCK_RELATIVE_PATH: &str = ".forge/state/remote-leases.lock";
/// Default lease time-to-live when none is given (15 minutes).
pub const DEFAULT_LEASE_TTL_MS: u64 = 15 * 60 * 1_000;
/// Maximum size of one worker profile.
const MAX_WORKER_PROFILE_BYTES: u64 = 4 * 1024;
/// How long a CLI lease operation waits for the lease lock.
const LEASE_LOCK_WAIT: Duration = Duration::from_secs(2);

/// Current wall-clock time in milliseconds since the Unix epoch.
pub use agentforge_orchestrator::wall_clock_ms as now_ms;

/// One lease as seen at an observation time.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LeaseView {
    /// Lease identity.
    pub lease_id: String,
    /// Leased task.
    pub task_id: String,
    /// Owning worker.
    pub worker_id: String,
    /// Task lease generation.
    pub generation: u64,
    /// Issue time.
    pub issued_at_ms: u64,
    /// Expiry time.
    pub expires_at_ms: u64,
    /// Effective state at the observation time (`active`, `expired`, or `released`); an active
    /// lease past its expiry reads `expired` even before a sweep records it.
    pub state: String,
}

/// One registered worker and its active lease count.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkerView {
    /// Worker descriptor built from its profile.
    pub descriptor: RemoteWorkerDescriptor,
    /// Active leases at the observation time.
    pub active_leases: usize,
}

/// Loads every worker profile in worker-ID order. A missing directory means no workers.
pub fn load_workers(root: impl AsRef<Path>) -> Result<Vec<RemoteWorkerDescriptor>, OperatorError> {
    let directory = root.as_ref().join(WORKER_PROFILE_RELATIVE_PATH);
    let entries = match fs::read_dir(&directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(OperatorError::new(error.to_string())),
    };
    let mut paths = Vec::new();
    for entry in entries {
        let path = entry
            .map_err(|error| OperatorError::new(error.to_string()))?
            .path();
        if path.extension().and_then(|value| value.to_str()) == Some("conf") {
            paths.push(path);
        }
    }
    paths.sort();
    paths.iter().map(|path| load_worker(path)).collect()
}

fn load_worker(path: &Path) -> Result<RemoteWorkerDescriptor, OperatorError> {
    let invalid =
        |reason: String| OperatorError::new(format!("worker profile {}: {reason}", path.display()));
    let id = path
        .file_stem()
        .and_then(|value| value.to_str())
        .ok_or_else(|| invalid("file name is not UTF-8".into()))?;
    let metadata = fs::symlink_metadata(path).map_err(|error| invalid(error.to_string()))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(invalid("is not a regular file".into()));
    }
    if metadata.len() > MAX_WORKER_PROFILE_BYTES {
        return Err(invalid(format!("exceeds {MAX_WORKER_PROFILE_BYTES} bytes")));
    }
    let text = fs::read_to_string(path).map_err(|error| invalid(error.to_string()))?;
    let mut platform = None;
    let mut capabilities = Vec::new();
    let mut max_leases = None;
    for (index, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (key, value) = line
            .split_once('=')
            .ok_or_else(|| invalid(format!("line {} is not key=value", index + 1)))?;
        let value = value.trim();
        match key.trim() {
            "platform" if platform.is_none() => platform = Some(value.to_owned()),
            "capability" => capabilities
                .push(WorkerCapability::parse(value).map_err(|error| invalid(error.to_string()))?),
            "max_leases" if max_leases.is_none() => {
                max_leases = Some(
                    value
                        .parse::<u16>()
                        .map_err(|_| invalid(format!("max_leases is not a number: {value}")))?,
                );
            }
            other => return Err(invalid(format!("unknown or repeated key: {other}"))),
        }
    }
    let worker_id = RemoteWorkerId::parse(id).map_err(|error| invalid(error.to_string()))?;
    RemoteWorkerDescriptor::new(
        worker_id,
        platform.ok_or_else(|| invalid("platform is missing".into()))?,
        capabilities,
        max_leases.ok_or_else(|| invalid("max_leases is missing".into()))?,
    )
    .map_err(|error| invalid(error.to_string()))
}

/// Lists registered workers with their active lease counts.
pub fn list_workers(root: impl AsRef<Path>, now: u64) -> Result<Vec<WorkerView>, OperatorError> {
    let root = root.as_ref();
    let book = load_book(root)?;
    Ok(load_workers(root)?
        .into_iter()
        .map(|descriptor| {
            let active_leases = book
                .active_leases_at(now)
                .into_iter()
                .filter(|lease| lease.worker_id() == descriptor.worker_id())
                .count();
            WorkerView {
                descriptor,
                active_leases,
            }
        })
        .collect())
}

/// Lists every lease in lease-ID order with its effective state at `now`. Read-only.
pub fn list_leases(root: impl AsRef<Path>, now: u64) -> Result<Vec<LeaseView>, OperatorError> {
    Ok(load_book(root.as_ref())?
        .leases()
        .map(|lease| view(lease, now))
        .collect())
}

/// Grants one lease for a ready, pending task to the first registered worker with capacity, or to
/// `worker` when given.
pub fn grant_lease(
    root: impl AsRef<Path>,
    task_id: &TaskId,
    worker: Option<&str>,
    ttl_ms: u64,
    now: u64,
    actor: &str,
) -> Result<LeaseView, OperatorError> {
    validate_actor(actor)?;
    validate_ttl(ttl_ms)?;
    let root = root.as_ref();
    let _lock = LeaseLock::acquire(root, LEASE_LOCK_WAIT)?;
    let graph = load_graph(root)?;
    let record = graph
        .get(task_id)
        .ok_or_else(|| OperatorError::new("task not found"))?;
    if record.state() != TaskState::Pending {
        return Err(OperatorError::new(format!(
            "only pending tasks can be leased; {task_id} is {}",
            record.state().as_str()
        )));
    }
    let mut workers = load_workers(root)?;
    if let Some(wanted) = worker {
        workers.retain(|descriptor| descriptor.worker_id().as_str() == wanted);
        if workers.is_empty() {
            return Err(OperatorError::new(format!(
                "worker is not registered: {wanted} (add .forge/workers/{wanted}.conf)"
            )));
        }
    }
    if workers.is_empty() {
        return Err(OperatorError::new(
            "no workers are registered; add a profile under .forge/workers/",
        ));
    }
    let mut book = load_book(root)?;
    if let Some(conflict) = ownership_conflict(&graph, &book, task_id, now) {
        return Err(OperatorError::new(conflict));
    }
    let ordinal = book
        .leases()
        .filter(|lease| lease.task_id() == task_id)
        .count()
        + 1;
    let lease_id = LeaseId::parse(format!("{task_id}.L{ordinal}"))
        .map_err(|error| OperatorError::new(error.to_string()))?;
    let expires_at_ms = now
        .checked_add(ttl_ms)
        .ok_or_else(|| OperatorError::new("lease expiry overflows"))?;
    let batch = plan_remote_dispatch(
        &graph,
        &workers,
        &mut book,
        &[RemoteDispatchRequest {
            task_id: task_id.clone(),
            lease_id: lease_id.clone(),
            issued_at_ms: now,
            expires_at_ms,
        }],
        now,
    )
    .map_err(|error| OperatorError::new(error.to_string()))?;
    debug_assert_eq!(batch.assignments.len(), 1);
    let lease = book
        .get(&lease_id)
        .cloned()
        .ok_or_else(|| OperatorError::new("granted lease is missing"))?;
    commit(root, &book, &[(&lease, "granted")], actor)?;
    Ok(view(&lease, now))
}

/// Extends an active lease to `now + ttl_ms` on behalf of its recorded owner.
pub fn renew_lease(
    root: impl AsRef<Path>,
    lease_id: &str,
    ttl_ms: u64,
    now: u64,
    actor: &str,
) -> Result<LeaseView, OperatorError> {
    validate_actor(actor)?;
    validate_ttl(ttl_ms)?;
    let root = root.as_ref();
    let _lock = LeaseLock::acquire(root, LEASE_LOCK_WAIT)?;
    let mut book = load_book(root)?;
    let (lease_id, owner, generation) = owner_of(&book, lease_id)?;
    sweep_due(root, &mut book, now, actor)?;
    let expires_at_ms = now
        .checked_add(ttl_ms)
        .ok_or_else(|| OperatorError::new("lease expiry overflows"))?;
    let result = book
        .renew(&lease_id, &owner, generation, now, expires_at_ms)
        .cloned();
    finish_owner_change(root, &book, result, "renewed", now, actor)
}

/// Releases an active lease on behalf of its recorded owner, freeing its task.
pub fn release_lease(
    root: impl AsRef<Path>,
    lease_id: &str,
    now: u64,
    actor: &str,
) -> Result<LeaseView, OperatorError> {
    validate_actor(actor)?;
    let root = root.as_ref();
    let _lock = LeaseLock::acquire(root, LEASE_LOCK_WAIT)?;
    let mut book = load_book(root)?;
    let (lease_id, owner, generation) = owner_of(&book, lease_id)?;
    sweep_due(root, &mut book, now, actor)?;
    let result = book.release(&lease_id, &owner, generation, now).cloned();
    finish_owner_change(root, &book, result, "released", now, actor)
}

/// Marks every lease due at `now` as expired and audits each one.
pub fn expire_leases(
    root: impl AsRef<Path>,
    now: u64,
    actor: &str,
) -> Result<Vec<LeaseView>, OperatorError> {
    expire_leases_waiting(root.as_ref(), now, actor, LEASE_LOCK_WAIT)
}

/// Like [`expire_leases`], but gives up immediately when the lease lock is held.
///
/// Returns `Ok(None)` when the lock was busy. The daemon sweeper uses this so it never waits.
pub fn try_expire_leases(
    root: impl AsRef<Path>,
    now: u64,
    actor: &str,
) -> Result<Option<Vec<LeaseView>>, OperatorError> {
    match expire_leases_waiting(root.as_ref(), now, actor, Duration::ZERO) {
        Ok(expired) => Ok(Some(expired)),
        Err(error) if error.to_string().starts_with(LOCK_HELD_PREFIX) => Ok(None),
        Err(error) => Err(error),
    }
}

fn expire_leases_waiting(
    root: &Path,
    now: u64,
    actor: &str,
    wait: Duration,
) -> Result<Vec<LeaseView>, OperatorError> {
    validate_actor(actor)?;
    let store = FileLeaseStore::for_project_root(root);
    // Nothing to do (and nothing to lock) for a project that never leased a task.
    if !store.path().is_file() {
        return Ok(Vec::new());
    }
    let _lock = LeaseLock::acquire(root, wait)?;
    let mut book = load_book(root)?;
    let before = book.clone();
    sweep_due(root, &mut book, now, actor)?;
    let leases = book
        .leases()
        .filter(|lease| {
            before.get(lease.lease_id()).map(TaskLease::state) == Some(LeaseState::Active)
                && lease.state() == LeaseState::Expired
        })
        .cloned()
        .collect::<Vec<_>>();
    Ok(leases.iter().map(|lease| view(lease, now)).collect())
}

/// Returns why `task_id` may not run locally at `now`: it holds an active lease, or its paths
/// overlap a task that does. Read-only; `None` when the project has no lease state.
pub fn local_run_conflict(
    root: impl AsRef<Path>,
    graph: &TaskGraph,
    task_id: &TaskId,
    now: u64,
) -> Result<Option<String>, OperatorError> {
    let book = load_book(root.as_ref())?;
    Ok(remote_lease_conflict(graph, &book, task_id, now))
}

fn ownership_conflict(
    graph: &TaskGraph,
    book: &LeaseBook,
    task_id: &TaskId,
    now: u64,
) -> Option<String> {
    let task = graph.get(task_id)?.task();
    for record in graph.records() {
        if record.id() == task_id || record.state() != TaskState::Running {
            continue;
        }
        if let Some(path) = first_path_overlap(&task.allowed_paths, &record.task().allowed_paths) {
            return Some(format!(
                "task {task_id} path {path} overlaps running task {}",
                record.id()
            ));
        }
    }
    // The task's own active lease is left to the dispatch planner, which reports it precisely.
    if book.active_lease_for_task_at(task_id, now).is_some() {
        return None;
    }
    remote_lease_conflict(graph, book, task_id, now)
}

fn finish_owner_change(
    root: &Path,
    book: &LeaseBook,
    result: Result<TaskLease, agentforge_core::remote::RemoteWorkerError>,
    action: &str,
    now: u64,
    actor: &str,
) -> Result<LeaseView, OperatorError> {
    let lease = result.map_err(|error| OperatorError::new(error.to_string()))?;
    commit(root, book, &[(&lease, action)], actor)?;
    Ok(view(&lease, now))
}

/// Records every lease already due at `now` as expired before an owner change, so a renew or
/// release of a past-due lease is refused with the snapshot and audit log in agreement.
fn sweep_due(
    root: &Path,
    book: &mut LeaseBook,
    now: u64,
    actor: &str,
) -> Result<(), OperatorError> {
    let expired = book
        .expire_due(now)
        .iter()
        .filter_map(|lease_id| book.get(lease_id).cloned())
        .collect::<Vec<_>>();
    if expired.is_empty() {
        return Ok(());
    }
    let changes = expired
        .iter()
        .map(|lease| (lease, "expired"))
        .collect::<Vec<_>>();
    commit(root, book, &changes, actor)
}

fn owner_of(
    book: &LeaseBook,
    lease_id: &str,
) -> Result<(LeaseId, RemoteWorkerId, u64), OperatorError> {
    let lease_id =
        LeaseId::parse(lease_id).map_err(|error| OperatorError::new(error.to_string()))?;
    let lease = book
        .get(&lease_id)
        .ok_or_else(|| OperatorError::new(format!("lease not found: {}", lease_id.as_str())))?;
    Ok((
        lease_id.clone(),
        lease.worker_id().clone(),
        lease.generation(),
    ))
}

fn commit(
    root: &Path,
    book: &LeaseBook,
    changes: &[(&TaskLease, &str)],
    actor: &str,
) -> Result<(), OperatorError> {
    // A lease grant can be a project's first audited action, so the audit log is created here
    // when missing (task creation does not create it).
    let audit_path = root.join(crate::AUDIT_RELATIVE_PATH);
    if let Some(parent) = audit_path.parent() {
        fs::create_dir_all(parent).map_err(|error| OperatorError::new(error.to_string()))?;
    }
    let mut audit =
        FileAuditStore::open(audit_path).map_err(|error| OperatorError::new(error.to_string()))?;
    FileLeaseStore::for_project_root(root)
        .save(book)
        .map_err(|error| OperatorError::new(error.to_string()))?;
    for (lease, action) in changes {
        let sequence = next_sequence(&audit);
        let event = AuditEvent::new(
            sequence,
            format!("lease-{action}-{sequence}"),
            AuditEventKind::LeaseRecorded,
            actor,
            1,
        )
        .with_task_id(lease.task_id().as_str())
        .with_field("action", *action)
        .with_field("lease_id", lease.lease_id().as_str())
        .with_field("worker_id", lease.worker_id().as_str())
        .with_field("generation", lease.generation().to_string())
        .with_field("expires_at_ms", lease.expires_at_ms().to_string());
        audit
            .append(event)
            .map_err(|error| OperatorError::new(error.to_string()))?;
    }
    Ok(())
}

fn load_book(root: &Path) -> Result<LeaseBook, OperatorError> {
    Ok(FileLeaseStore::for_project_root(root)
        .load()
        .map_err(|error| OperatorError::new(error.to_string()))?
        .unwrap_or_default())
}

fn view(lease: &TaskLease, now: u64) -> LeaseView {
    let state = match lease.state() {
        LeaseState::Active if !lease.is_active_at(now) => "expired",
        other => other.as_str(),
    };
    LeaseView {
        lease_id: lease.lease_id().as_str().to_owned(),
        task_id: lease.task_id().to_string(),
        worker_id: lease.worker_id().as_str().to_owned(),
        generation: lease.generation(),
        issued_at_ms: lease.issued_at_ms(),
        expires_at_ms: lease.expires_at_ms(),
        state: state.to_owned(),
    }
}

fn validate_ttl(ttl_ms: u64) -> Result<(), OperatorError> {
    if ttl_ms == 0 || ttl_ms > MAX_LEASE_DURATION_MS {
        return Err(OperatorError::new(format!(
            "lease TTL must be between 1 and {MAX_LEASE_DURATION_MS} ms"
        )));
    }
    Ok(())
}

const LOCK_HELD_PREFIX: &str = "lease state is locked";

/// Exclusive, never-stolen lock over lease state; removed on drop.
struct LeaseLock {
    path: PathBuf,
}

impl LeaseLock {
    fn acquire(root: &Path, wait: Duration) -> Result<Self, OperatorError> {
        let path = root.join(LEASE_LOCK_RELATIVE_PATH);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| OperatorError::new(error.to_string()))?;
        }
        let deadline = Instant::now() + wait;
        loop {
            match OpenOptions::new().write(true).create_new(true).open(&path) {
                Ok(mut file) => {
                    let _ = writeln!(file, "pid={}", std::process::id());
                    return Ok(Self { path });
                }
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                    if Instant::now() >= deadline {
                        return Err(OperatorError::new(format!(
                            "{LOCK_HELD_PREFIX} by another operation ({}); if no forge or forged \
                             process is running for this project, remove that file",
                            path.display()
                        )));
                    }
                    thread::sleep(Duration::from_millis(20));
                }
                Err(error) => return Err(OperatorError::new(error.to_string())),
            }
        }
    }
}

impl Drop for LeaseLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}
