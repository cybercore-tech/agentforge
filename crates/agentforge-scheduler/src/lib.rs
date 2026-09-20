//! Deterministic task scheduling and serialized integration reservations.

use agentforge_core::agent::AgentRole;
use agentforge_core::task::{TaskGraph, TaskGraphError, TaskId};
use std::collections::BTreeSet;
use std::fmt;

/// One deterministic runnable batch of disjoint tasks.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RunnableBatch {
    /// Task IDs in canonical order.
    pub task_ids: Vec<TaskId>,
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

impl From<TaskGraphError> for ScheduleError {
    fn from(value: TaskGraphError) -> Self {
        Self::InvalidGraph(value.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agentforge_core::agent::{AgentRole, AgentTask};
    fn task(id: &str, path: &str) -> AgentTask {
        let mut t = AgentTask::new(id, "m", AgentRole::Implementer, "x");
        t.allowed_paths = vec![path.into()];
        t
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
}
