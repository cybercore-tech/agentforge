//! Same-host worker process (P4-M005).
//!
//! A registered worker claims the tasks leased to it, runs each through the standard launch path
//! (managed worktree, agent, gates, evidence), renews the lease while the agent runs, and releases
//! it afterwards. The worker shares the project directory; there is no transport yet.

use crate::leases::{LeaseView, claim_lease, load_workers, now_ms, release_lease, renew_lease};
use crate::{OperatorError, approved_boundaries, load_graph, open_project_audit};
use agentforge_adapter::AgentAdapter;
use agentforge_core::remote::{LeaseState, TaskLease};
use agentforge_core::task::{TaskId, TaskState};
use agentforge_orchestrator::{
    ForegroundLaunch, LeaseClaim, SliceError, launch_leased_process_persisted,
};
use agentforge_state::{FileLeaseStore, FileTaskStore, LeaseStore};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

/// Shortest interval between lease renewals.
const MIN_RENEW_INTERVAL: Duration = Duration::from_millis(20);

/// How a worker runs.
#[derive(Clone, Debug)]
pub struct WorkerOptions {
    /// Base ref for new task worktrees.
    pub base_ref: String,
    /// Run at most one task (or report idle once), then return.
    pub once: bool,
    /// Wait between polls when no lease is claimable.
    pub poll: Duration,
}

/// Progress reported by [`run_worker`].
#[derive(Debug)]
pub enum WorkerReport<'a> {
    /// No claimable lease right now.
    Idle,
    /// A lease was claimed and its task is about to run.
    Claimed {
        /// Claimed lease.
        claim: &'a LeaseClaim,
        /// Its task.
        task_id: &'a TaskId,
    },
    /// The task ran (successfully or not) through the launch path.
    Launched(&'a ForegroundLaunch),
    /// The launch was refused or failed before a result.
    LaunchFailed {
        /// The task.
        task_id: &'a TaskId,
        /// Why.
        error: &'a SliceError,
    },
    /// Lease renewals made while the agent ran.
    Renewals {
        /// Successful renewals.
        renewed: usize,
        /// Renewal failures (the worker kept running).
        failures: &'a [String],
    },
    /// The lease was released after the run.
    Released(&'a LeaseView),
    /// Releasing the lease failed (for example, it already expired).
    ReleaseFailed(&'a str),
}

/// Totals for one [`run_worker`] call.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct WorkerSummary {
    /// Tasks claimed and run.
    pub tasks_run: usize,
    /// Tasks whose launch failed or whose agent or gates failed.
    pub failures: usize,
}

/// Runs a registered worker: claims its leases in lease-ID order and runs each task.
///
/// With `options.once` it runs at most one task (or reports idle once) and returns. Otherwise it
/// polls until the process stops.
pub fn run_worker<A: AgentAdapter>(
    root: impl AsRef<Path>,
    worker_id: &str,
    adapter: &A,
    options: &WorkerOptions,
    mut report: impl FnMut(WorkerReport<'_>),
) -> Result<WorkerSummary, OperatorError> {
    let root = root.as_ref();
    if !load_workers(root)?
        .iter()
        .any(|worker| worker.worker_id().as_str() == worker_id)
    {
        return Err(OperatorError::new(format!(
            "worker is not registered: {worker_id} (add .forge/workers/{worker_id}.conf)"
        )));
    }
    let actor = format!("worker:{worker_id}");
    let mut summary = WorkerSummary::default();
    loop {
        let Some(lease) = next_claimable(root, worker_id, now_ms())? else {
            report(WorkerReport::Idle);
            if options.once {
                return Ok(summary);
            }
            thread::sleep(options.poll);
            continue;
        };
        let claim = claim_lease(root, lease.lease_id().as_str(), worker_id, now_ms())?;
        let task_id = lease.task_id().clone();
        report(WorkerReport::Claimed {
            claim: &claim,
            task_id: &task_id,
        });

        let window = lease.expires_at_ms().saturating_sub(lease.issued_at_ms());
        let renewal = Renewal::start(root, lease.lease_id().as_str(), window, &actor);
        let result = approved_boundaries(root, &task_id).and_then(|approvals| {
            let mut audit = open_project_audit(root)?;
            Ok(launch_leased_process_persisted(
                root,
                &FileTaskStore::for_project_root(root),
                &mut audit,
                &task_id,
                adapter,
                &approvals,
                &options.base_ref,
                &claim,
            ))
        });
        let (renewed, failures) = renewal.stop();
        summary.tasks_run += 1;
        match result? {
            Ok(launch) => {
                if !launch.execution.succeeded() {
                    summary.failures += 1;
                }
                report(WorkerReport::Launched(&launch));
            }
            Err(error) => {
                summary.failures += 1;
                report(WorkerReport::LaunchFailed {
                    task_id: &task_id,
                    error: &error,
                });
            }
        }
        report(WorkerReport::Renewals {
            renewed,
            failures: &failures,
        });
        match release_lease(root, lease.lease_id().as_str(), now_ms(), &actor) {
            Ok(released) => report(WorkerReport::Released(&released)),
            Err(error) => report(WorkerReport::ReleaseFailed(&error.to_string())),
        }
        if options.once {
            return Ok(summary);
        }
    }
}

/// The next lease this worker can claim: its own, active at `now`, for a pending, ready task.
fn next_claimable(
    root: &Path,
    worker_id: &str,
    now: u64,
) -> Result<Option<TaskLease>, OperatorError> {
    let Some(book) = FileLeaseStore::for_project_root(root)
        .load()
        .map_err(|error| OperatorError::new(error.to_string()))?
    else {
        return Ok(None);
    };
    let graph = load_graph(root)?;
    let ready = graph
        .ready_task_ids()
        .map_err(|error| OperatorError::new(error.to_string()))?;
    Ok(book
        .leases()
        .find(|lease| {
            lease.state() == LeaseState::Active
                && lease.is_active_at(now)
                && lease.worker_id().as_str() == worker_id
                && ready.contains(lease.task_id())
                && graph
                    .get(lease.task_id())
                    .is_some_and(|record| record.state() == TaskState::Pending)
        })
        .cloned())
}

/// Background renewal of one lease while its task runs.
struct Renewal {
    stop: Arc<AtomicBool>,
    handle: thread::JoinHandle<(usize, Vec<String>)>,
}

impl Renewal {
    fn start(root: &Path, lease_id: &str, window_ms: u64, actor: &str) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let (root, lease_id, actor) = (root.to_path_buf(), lease_id.to_owned(), actor.to_owned());
        let flag = Arc::clone(&stop);
        let failures = Arc::new(Mutex::new(Vec::new()));
        let handle = thread::spawn(move || {
            let interval = Duration::from_millis(window_ms / 3).max(MIN_RENEW_INTERVAL);
            let mut renewed = 0;
            let mut next = Instant::now() + interval;
            while !flag.load(Ordering::SeqCst) {
                thread::sleep(Duration::from_millis(5));
                if Instant::now() < next {
                    continue;
                }
                next = Instant::now() + interval;
                match renew_lease(&root, &lease_id, window_ms, now_ms(), &actor) {
                    Ok(_) => renewed += 1,
                    Err(error) => {
                        failures
                            .lock()
                            .unwrap_or_else(|poisoned| poisoned.into_inner())
                            .push(error.to_string());
                        // Keep running: the claim and task state already authorize the run.
                        break;
                    }
                }
            }
            let failures = failures
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .clone();
            (renewed, failures)
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
