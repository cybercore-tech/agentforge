//! Deterministic task scheduling and serialized integration reservations.

use agentforge_core::agent::AgentRole;
use agentforge_core::remote::{
    LeaseBook, LeaseId, RemoteWorkerDescriptor, RemoteWorkerError, RemoteWorkerId,
};
use agentforge_core::task::{TaskGraph, TaskGraphError, TaskId};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

/// One deterministic runnable batch of disjoint tasks.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RunnableBatch {
    /// Task IDs in canonical order.
    pub task_ids: Vec<TaskId>,
}

/// One caller-supplied request to lease a ready task to a remote worker.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RemoteDispatchRequest {
    /// Task that must be ready in the supplied graph.
    pub task_id: TaskId,
    /// Unique lease identity selected by the local authority.
    pub lease_id: LeaseId,
    /// Timestamp at which the lease is issued.
    pub issued_at_ms: u64,
    /// Timestamp at which the lease expires.
    pub expires_at_ms: u64,
}

/// Evidence for one successful remote dispatch assignment.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RemoteAssignment {
    /// Dispatched task identity.
    pub task_id: TaskId,
    /// Selected worker identity.
    pub worker_id: RemoteWorkerId,
    /// Persistable lease identity.
    pub lease_id: LeaseId,
    /// Generation accepted by the lease book.
    pub generation: u64,
}

/// One deterministic, all-or-nothing remote dispatch decision.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RemoteDispatchBatch {
    /// Assignment evidence in canonical task-ID order.
    pub assignments: Vec<RemoteAssignment>,
}

/// A scheduler decision failure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ScheduleError {
    /// Underlying graph was invalid.
    InvalidGraph(String),
    /// Two tasks claim overlapping write ownership.
    OwnershipConflict {
        /// Earlier task/path owner.
        first: String,
        /// Later conflicting task.
        second: String,
        /// Conflicting path.
        path: String,
    },
    /// More than one integrator reservation.
    MultipleIntegrators,
    /// A remote-worker domain invariant rejected a dispatch attempt.
    RemoteWorker(RemoteWorkerError),
    /// A request names a task that is not currently ready.
    TaskNotReady {
        /// Task that cannot be dispatched.
        task_id: TaskId,
    },
    /// Two requests name the same task.
    DuplicateRequestTask {
        /// Duplicated task identity.
        task_id: TaskId,
    },
    /// Two requests name the same lease.
    DuplicateRequestLease {
        /// Duplicated lease identity.
        lease_id: LeaseId,
    },
    /// Two worker descriptors use the same worker identity.
    DuplicateWorker {
        /// Duplicated worker identity.
        worker_id: RemoteWorkerId,
    },
    /// No supplied worker had capacity for this request.
    WorkerUnavailable {
        /// Task that could not be assigned.
        task_id: TaskId,
    },
}
impl fmt::Display for ScheduleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for ScheduleError {}

/// Produces one deterministic batch from currently ready tasks.
pub fn plan_batch(graph: &TaskGraph) -> Result<RunnableBatch, ScheduleError> {
    let ready = graph
        .ready_task_ids()
        .map_err(|e| ScheduleError::InvalidGraph(e.to_string()))?;
    let mut chosen = Vec::new();
    let mut owned = BTreeSet::new();
    for id in ready {
        let record = graph
            .records()
            .find(|r| r.id() == &id)
            .expect("ready task exists");
        let mut conflict = None;
        for path in &record.task().allowed_paths {
            if let Some(existing) = owned.iter().find(|p: &&String| overlaps(p, path)) {
                conflict = Some(((*existing).clone(), path.clone()));
                break;
            }
        }
        if let Some((first, path)) = conflict {
            return Err(ScheduleError::OwnershipConflict {
                first,
                second: id.to_string(),
                path,
            });
        }
        owned.extend(record.task().allowed_paths.iter().cloned());
        chosen.push(id);
    }
    Ok(RunnableBatch { task_ids: chosen })
}

/// Plans deterministic remote assignments without partially mutating the lease book.
pub fn plan_remote_dispatch(
    graph: &TaskGraph,
    workers: &[RemoteWorkerDescriptor],
    leases: &mut LeaseBook,
    requests: &[RemoteDispatchRequest],
    observed_at_ms: u64,
) -> Result<RemoteDispatchBatch, ScheduleError> {
    let ready = graph
        .ready_task_ids()
        .map_err(|error| ScheduleError::InvalidGraph(error.to_string()))?;
    let ready = ready.into_iter().collect::<BTreeSet<_>>();

    let mut ordered_requests = requests.to_vec();
    ordered_requests.sort_by(|left, right| left.task_id.cmp(&right.task_id));

    let mut request_tasks = BTreeSet::new();
    let mut request_leases = BTreeSet::new();
    for request in &ordered_requests {
        if !request_tasks.insert(request.task_id.clone()) {
            return Err(ScheduleError::DuplicateRequestTask {
                task_id: request.task_id.clone(),
            });
        }
        if !request_leases.insert(request.lease_id.clone()) {
            return Err(ScheduleError::DuplicateRequestLease {
                lease_id: request.lease_id.clone(),
            });
        }
        if !ready.contains(&request.task_id) {
            return Err(ScheduleError::TaskNotReady {
                task_id: request.task_id.clone(),
            });
        }
    }

    let mut ordered_workers = BTreeMap::new();
    for worker in workers {
        let worker_id = worker.worker_id().clone();
        if ordered_workers.insert(worker_id.clone(), worker).is_some() {
            return Err(ScheduleError::DuplicateWorker { worker_id });
        }
    }

    validate_requested_ownership(graph, &request_tasks)?;

    let mut planned_leases = leases.clone();
    planned_leases.expire_due(observed_at_ms);
    let mut assignments = Vec::with_capacity(ordered_requests.len());

    for request in ordered_requests {
        let mut assigned = None;
        for worker in ordered_workers.values() {
            match planned_leases.grant(
                worker,
                request.lease_id.clone(),
                request.task_id.clone(),
                request.issued_at_ms,
                request.expires_at_ms,
            ) {
                Ok(lease) => {
                    assigned = Some(RemoteAssignment {
                        task_id: lease.task_id().clone(),
                        worker_id: lease.worker_id().clone(),
                        lease_id: lease.lease_id().clone(),
                        generation: lease.generation(),
                    });
                    break;
                }
                Err(RemoteWorkerError::WorkerConcurrencyLimit { .. }) => continue,
                Err(error) => return Err(ScheduleError::RemoteWorker(error)),
            }
        }

        assignments.push(assigned.ok_or(ScheduleError::WorkerUnavailable {
            task_id: request.task_id,
        })?);
    }

    *leases = planned_leases;
    Ok(RemoteDispatchBatch { assignments })
}

/// Validates that at most one Integrator is reserved for serialized integration.
pub fn reserve_integrator(graph: &TaskGraph) -> Result<Option<TaskId>, ScheduleError> {
    let mut found = None;
    for record in graph.records() {
        if record.task().primary_role == AgentRole::Integrator {
            if found.is_some() {
                return Err(ScheduleError::MultipleIntegrators);
            }
            found = Some(record.id().clone());
        }
    }
    Ok(found)
}

fn overlaps(a: &str, b: &str) -> bool {
    let a = a.trim_matches('/');
    let b = b.trim_matches('/');
    a == b
        || a.strip_prefix(b).is_some_and(|r| r.starts_with('/'))
        || b.strip_prefix(a).is_some_and(|r| r.starts_with('/'))
}

fn validate_requested_ownership(
    graph: &TaskGraph,
    task_ids: &BTreeSet<TaskId>,
) -> Result<(), ScheduleError> {
    let mut owned = BTreeSet::new();
    for task_id in task_ids {
        let record = graph
            .records()
            .find(|record| record.id() == task_id)
            .expect("ready task exists in graph");
        for requested_path in &record.task().allowed_paths {
            if let Some(existing) = owned
                .iter()
                .find(|existing: &&String| overlaps(existing, requested_path))
            {
                return Err(ScheduleError::OwnershipConflict {
                    first: (*existing).clone(),
                    second: task_id.to_string(),
                    path: requested_path.clone(),
                });
            }
        }
        owned.extend(record.task().allowed_paths.iter().cloned());
    }
    Ok(())
}

impl From<TaskGraphError> for ScheduleError {
    fn from(value: TaskGraphError) -> Self {
        Self::InvalidGraph(value.to_string())
    }
}

impl From<RemoteWorkerError> for ScheduleError {
    fn from(value: RemoteWorkerError) -> Self {
        Self::RemoteWorker(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agentforge_core::agent::{AgentRole, AgentTask};
    use agentforge_core::remote::{
        LeaseId, LeaseState, RemoteWorkerDescriptor, RemoteWorkerId, WorkerCapability,
    };

    fn task(id: &str, path: &str) -> AgentTask {
        let mut t = AgentTask::new(id, "m", AgentRole::Implementer, "x");
        t.allowed_paths = vec![path.into()];
        t
    }

    fn worker(id: &str, maximum: u16) -> RemoteWorkerDescriptor {
        RemoteWorkerDescriptor::new(
            RemoteWorkerId::parse(id).expect("valid worker ID"),
            "linux-x86_64",
            vec![WorkerCapability::parse("rust").expect("valid capability")],
            maximum,
        )
        .expect("valid worker")
    }

    fn request(task_id: &str, lease_id: &str) -> RemoteDispatchRequest {
        RemoteDispatchRequest {
            task_id: TaskId::parse(task_id).expect("valid task ID"),
            lease_id: LeaseId::parse(lease_id).expect("valid lease ID"),
            issued_at_ms: 1_000,
            expires_at_ms: 5_000,
        }
    }
    #[test]
    fn disjoint_tasks_share_batch() {
        let g = TaskGraph::from_tasks([task("a", "a"), task("b", "b")]).unwrap();
        assert_eq!(plan_batch(&g).unwrap().task_ids.len(), 2);
    }
    #[test]
    fn overlap_fails_closed() {
        let g = TaskGraph::from_tasks([task("a", "src"), task("b", "src/lib")]).unwrap();
        assert!(matches!(
            plan_batch(&g),
            Err(ScheduleError::OwnershipConflict { .. })
        ));
    }

    #[test]
    fn remote_dispatch_is_deterministic_across_input_order() {
        let graph = TaskGraph::from_tasks([task("a", "a"), task("b", "b")]).unwrap();
        let workers = [worker("worker-b", 1), worker("worker-a", 1)];
        let requests = [request("b", "lease-b"), request("a", "lease-a")];
        let mut first_leases = LeaseBook::new();
        let first = plan_remote_dispatch(&graph, &workers, &mut first_leases, &requests, 1_000)
            .expect("dispatch succeeds");

        let reversed_workers = [worker("worker-a", 1), worker("worker-b", 1)];
        let reversed_requests = [request("a", "lease-a"), request("b", "lease-b")];
        let mut second_leases = LeaseBook::new();
        let second = plan_remote_dispatch(
            &graph,
            &reversed_workers,
            &mut second_leases,
            &reversed_requests,
            1_000,
        )
        .expect("dispatch succeeds");

        assert_eq!(first, second);
        assert_eq!(
            first
                .assignments
                .iter()
                .map(|assignment| assignment.worker_id.as_str())
                .collect::<Vec<_>>(),
            vec!["worker-a", "worker-b"]
        );
    }

    #[test]
    fn remote_dispatch_requires_ready_tasks() {
        let mut blocked = task("b", "b");
        blocked.dependency_task_ids = vec!["a".into()];
        let graph = TaskGraph::from_tasks([task("a", "a"), blocked]).unwrap();
        let mut leases = LeaseBook::new();

        assert!(matches!(
            plan_remote_dispatch(
                &graph,
                &[worker("worker-a", 1)],
                &mut leases,
                &[request("b", "lease-b")],
                1_000,
            ),
            Err(ScheduleError::TaskNotReady { .. })
        ));
        assert!(leases.leases().next().is_none());
    }

    #[test]
    fn remote_dispatch_rejects_path_conflicts_before_mutation() {
        let graph = TaskGraph::from_tasks([task("a", "src"), task("b", "src/lib")]).unwrap();
        let mut leases = LeaseBook::new();

        assert!(matches!(
            plan_remote_dispatch(
                &graph,
                &[worker("worker-a", 2)],
                &mut leases,
                &[request("a", "lease-a"), request("b", "lease-b")],
                1_000,
            ),
            Err(ScheduleError::OwnershipConflict { .. })
        ));
        assert!(leases.leases().next().is_none());
    }

    #[test]
    fn remote_dispatch_is_all_or_nothing_when_workers_run_out_of_capacity() {
        let graph = TaskGraph::from_tasks([task("a", "a"), task("b", "b")]).unwrap();
        let mut leases = LeaseBook::new();

        assert!(matches!(
            plan_remote_dispatch(
                &graph,
                &[worker("worker-a", 1)],
                &mut leases,
                &[request("a", "lease-a"), request("b", "lease-b")],
                1_000,
            ),
            Err(ScheduleError::WorkerUnavailable { task_id }) if task_id.as_str() == "b"
        ));
        assert!(leases.leases().next().is_none());
    }

    #[test]
    fn remote_dispatch_reclaims_expired_lease_and_advances_generation() {
        let graph = TaskGraph::from_tasks([task("a", "a")]).unwrap();
        let worker = worker("worker-a", 1);
        let mut leases = LeaseBook::new();
        let old_id = LeaseId::parse("lease-old").expect("valid lease ID");
        leases
            .grant(
                &worker,
                old_id.clone(),
                TaskId::parse("a").expect("valid task ID"),
                0,
                1_000,
            )
            .expect("old lease");

        let batch = plan_remote_dispatch(
            &graph,
            &[worker],
            &mut leases,
            &[request("a", "lease-new")],
            1_000,
        )
        .expect("expired lease can be reclaimed");

        assert_eq!(batch.assignments[0].generation, 2);
        assert_eq!(
            leases.get(&old_id).expect("old lease").state(),
            LeaseState::Expired
        );
    }

    #[test]
    fn remote_dispatch_rejects_duplicate_inputs() {
        let graph = TaskGraph::from_tasks([task("a", "a"), task("b", "b")]).unwrap();
        let mut leases = LeaseBook::new();
        let duplicate_task_requests = [request("a", "lease-a"), request("a", "lease-b")];
        assert!(matches!(
            plan_remote_dispatch(
                &graph,
                &[worker("worker-a", 2)],
                &mut leases,
                &duplicate_task_requests,
                1_000,
            ),
            Err(ScheduleError::DuplicateRequestTask { .. })
        ));

        let duplicate_workers = [worker("worker-a", 1), worker("worker-a", 1)];
        assert!(matches!(
            plan_remote_dispatch(
                &graph,
                &duplicate_workers,
                &mut leases,
                &[request("a", "lease-a")],
                1_000,
            ),
            Err(ScheduleError::DuplicateWorker { .. })
        ));
    }
}
