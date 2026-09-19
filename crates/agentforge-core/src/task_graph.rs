//! Deterministic task identity, lifecycle, and dependency graph semantics.
//!
//! This module contains no persistence or filesystem behavior. Durable storage is implemented by
//! the separate `agentforge-state` crate.

use crate::agent::{AgentTask, TaskContractError};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::str::FromStr;

/// Current deterministic task-ID algorithm version.
pub const TASK_ID_ALGORITHM_VERSION: u16 = 1;

const TASK_ID_DOMAIN: &[u8] = b"agentforge-task-id/v1";

/// Stable deterministic task identifier.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TaskId(String);

impl TaskId {
    /// Returns the canonical textual task identifier.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for TaskId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl FromStr for TaskId {
    type Err = TaskGraphError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let Some(hex) = value.strip_prefix("task-") else {
            return Err(TaskGraphError::InvalidTaskId {
                value: value.to_owned(),
            });
        };

        let valid = hex.len() == 16
            && hex
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte));

        if !valid {
            return Err(TaskGraphError::InvalidTaskId {
                value: value.to_owned(),
            });
        }

        Ok(Self(value.to_owned()))
    }
}

/// Derives a deterministic task ID from a milestone and stable task key.
///
/// # Errors
///
/// Returns [`TaskGraphError`] when either identity component is empty.
pub fn derive_task_id(milestone_id: &str, stable_key: &str) -> Result<TaskId, TaskGraphError> {
    if milestone_id.trim().is_empty() {
        return Err(TaskGraphError::EmptyMilestoneId);
    }

    if stable_key.trim().is_empty() {
        return Err(TaskGraphError::EmptyStableKey);
    }

    let mut hash = 0xcbf2_9ce4_8422_2325_u64;

    for byte in TASK_ID_DOMAIN
        .iter()
        .copied()
        .chain(std::iter::once(0))
        .chain(milestone_id.as_bytes().iter().copied())
        .chain(std::iter::once(0))
        .chain(stable_key.as_bytes().iter().copied())
    {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }

    Ok(TaskId(format!("task-{hash:016x}")))
}

/// Durable lifecycle state of one AgentForge task.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum TaskState {
    /// Task exists but has not yet been declared ready.
    Pending,
    /// Dependencies are satisfied and the task may be considered for execution.
    Ready,
    /// A worker is currently executing the task.
    Running,
    /// Progress is blocked by a non-success prerequisite or external condition.
    Blocked,
    /// Task completed successfully.
    Completed,
    /// Task execution failed.
    Failed,
    /// Task was explicitly cancelled.
    Cancelled,
}

impl TaskState {
    /// Returns the stable machine-readable identity.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Ready => "ready",
            Self::Running => "running",
            Self::Blocked => "blocked",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }

    /// Returns whether this lifecycle state is terminal.
    #[must_use]
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Cancelled)
    }
}

/// Durable graph record around an [`AgentTask`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TaskRecord {
    stable_key: String,
    task: AgentTask,
    state: TaskState,
}

impl TaskRecord {
    /// Builds a validated task record from an existing agent task.
    ///
    /// # Errors
    ///
    /// Returns [`TaskGraphError`] when the stable key, agent contract, or deterministic task ID is
    /// invalid.
    pub fn from_task(
        stable_key: impl Into<String>,
        task: AgentTask,
        state: TaskState,
    ) -> Result<Self, TaskGraphError> {
        let stable_key = stable_key.into();

        if stable_key.trim().is_empty() {
            return Err(TaskGraphError::EmptyStableKey);
        }

        task.validate().map_err(TaskGraphError::Contract)?;

        let expected = derive_task_id(&task.milestone_id, &stable_key)?;
        let found = TaskId::from_str(&task.task_id)?;

        if found != expected {
            return Err(TaskGraphError::TaskIdMismatch {
                expected,
                found: task.task_id.clone(),
            });
        }

        if task
            .dependency_task_ids
            .iter()
            .any(|dependency| dependency == &task.task_id)
        {
            return Err(TaskGraphError::SelfDependency { task_id: found });
        }

        Ok(Self {
            stable_key,
            task,
            state,
        })
    }

    /// Returns the stable caller-supplied task key.
    #[must_use]
    pub fn stable_key(&self) -> &str {
        &self.stable_key
    }

    /// Returns the underlying provider-neutral task contract.
    #[must_use]
    pub const fn task(&self) -> &AgentTask {
        &self.task
    }

    /// Returns mutable access to the provider-neutral task contract.
    #[must_use]
    pub const fn task_mut(&mut self) -> &mut AgentTask {
        &mut self.task
    }

    /// Returns the current lifecycle state.
    #[must_use]
    pub const fn state(&self) -> TaskState {
        self.state
    }

    /// Changes the lifecycle state.
    pub fn set_state(&mut self, state: TaskState) {
        self.state = state;
    }

    /// Returns this record's deterministic task ID.
    ///
    /// # Errors
    ///
    /// Returns [`TaskGraphError`] if the stored contract contains an invalid task ID.
    pub fn task_id(&self) -> Result<TaskId, TaskGraphError> {
        TaskId::from_str(&self.task.task_id)
    }

    fn validate(&self) -> Result<(), TaskGraphError> {
        self.task.validate().map_err(TaskGraphError::Contract)?;

        let expected = derive_task_id(&self.task.milestone_id, &self.stable_key)?;
        let actual = TaskId::from_str(&self.task.task_id)?;

        if actual != expected {
            return Err(TaskGraphError::TaskIdMismatch {
                expected,
                found: self.task.task_id.clone(),
            });
        }

        if self
            .task
            .dependency_task_ids
            .iter()
            .any(|dependency| dependency == &self.task.task_id)
        {
            return Err(TaskGraphError::SelfDependency { task_id: actual });
        }

        Ok(())
    }
}

/// Deterministic directed task graph.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TaskGraph {
    tasks: BTreeMap<TaskId, TaskRecord>,
}

impl TaskGraph {
    /// Creates an empty graph.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            tasks: BTreeMap::new(),
        }
    }

    /// Inserts a task record.
    ///
    /// Missing dependencies are allowed temporarily so a graph can be reconstructed in arbitrary
    /// record order. [`Self::validate`] rejects them before the graph is usable.
    ///
    /// # Errors
    ///
    /// Returns [`TaskGraphError`] when the record itself is invalid or duplicates an existing
    /// identity.
    pub fn insert(&mut self, record: TaskRecord) -> Result<TaskId, TaskGraphError> {
        record.validate()?;
        let id = record.task_id()?;

        if self.tasks.contains_key(&id) {
            return Err(TaskGraphError::DuplicateTask { task_id: id });
        }

        if self.tasks.values().any(|existing| {
            existing.task.milestone_id == record.task.milestone_id
                && existing.stable_key == record.stable_key
        }) {
            return Err(TaskGraphError::DuplicateStableIdentity {
                milestone_id: record.task.milestone_id.clone(),
                stable_key: record.stable_key.clone(),
            });
        }

        self.tasks.insert(id.clone(), record);
        Ok(id)
    }

    /// Returns a task by ID.
    #[must_use]
    pub fn get(&self, task_id: &TaskId) -> Option<&TaskRecord> {
        self.tasks.get(task_id)
    }

    /// Returns mutable access to a task by ID.
    #[must_use]
    pub fn get_mut(&mut self, task_id: &TaskId) -> Option<&mut TaskRecord> {
        self.tasks.get_mut(task_id)
    }

    /// Changes a task lifecycle state.
    ///
    /// # Errors
    ///
    /// Returns [`TaskGraphError::UnknownTask`] when the requested task does not exist.
    pub fn transition(&mut self, task_id: &TaskId, state: TaskState) -> Result<(), TaskGraphError> {
        let record = self
            .tasks
            .get_mut(task_id)
            .ok_or_else(|| TaskGraphError::UnknownTask {
                task_id: task_id.clone(),
            })?;

        record.set_state(state);
        Ok(())
    }

    /// Returns the direct dependencies of one task in deterministic order.
    ///
    /// # Errors
    ///
    /// Returns [`TaskGraphError`] when the task or one of its dependencies does not exist.
    pub fn dependencies(&self, task_id: &TaskId) -> Result<Vec<&TaskRecord>, TaskGraphError> {
        let record = self
            .tasks
            .get(task_id)
            .ok_or_else(|| TaskGraphError::UnknownTask {
                task_id: task_id.clone(),
            })?;

        let mut dependencies = Vec::with_capacity(record.task.dependency_task_ids.len());

        for dependency in &record.task.dependency_task_ids {
            let dependency_id = TaskId::from_str(dependency)?;
            let dependency_record = self.tasks.get(&dependency_id).ok_or_else(|| {
                TaskGraphError::MissingDependency {
                    task_id: task_id.clone(),
                    dependency_id: dependency_id.clone(),
                }
            })?;

            dependencies.push(dependency_record);
        }

        dependencies.sort_by_key(|record| record.task.task_id.as_str());
        Ok(dependencies)
    }

    /// Returns tasks that directly depend on the provided task.
    #[must_use]
    pub fn dependents(&self, task_id: &TaskId) -> Vec<&TaskRecord> {
        self.tasks
            .values()
            .filter(|record| {
                record
                    .task
                    .dependency_task_ids
                    .iter()
                    .any(|dependency| dependency == task_id.as_str())
            })
            .collect()
    }

    /// Validates all graph-wide invariants.
    ///
    /// # Errors
    ///
    /// Returns [`TaskGraphError`] for malformed records, missing dependencies, or cycles.
    pub fn validate(&self) -> Result<(), TaskGraphError> {
        for (task_id, record) in &self.tasks {
            record.validate()?;

            for dependency in &record.task.dependency_task_ids {
                let dependency_id = TaskId::from_str(dependency)?;

                if !self.tasks.contains_key(&dependency_id) {
                    return Err(TaskGraphError::MissingDependency {
                        task_id: task_id.clone(),
                        dependency_id,
                    });
                }
            }
        }

        let mut visiting = BTreeSet::new();
        let mut visited = BTreeSet::new();

        for task_id in self.tasks.keys() {
            self.visit(task_id, &mut visiting, &mut visited)?;
        }

        Ok(())
    }

    /// Returns graph-ready tasks in deterministic task-ID order.
    ///
    /// # Errors
    ///
    /// Returns [`TaskGraphError`] when the graph is invalid.
    pub fn ready_tasks(&self) -> Result<Vec<&TaskRecord>, TaskGraphError> {
        self.validate()?;

        let mut ready = Vec::new();

        for record in self.tasks.values() {
            if !matches!(record.state, TaskState::Pending | TaskState::Ready) {
                continue;
            }

            let record_id = record.task_id()?;
            let mut dependencies_completed = true;

            for dependency in &record.task.dependency_task_ids {
                let dependency_id = TaskId::from_str(dependency)?;
                let dependency_record = self.tasks.get(&dependency_id).ok_or_else(|| {
                    TaskGraphError::MissingDependency {
                        task_id: record_id.clone(),
                        dependency_id,
                    }
                })?;

                if dependency_record.state != TaskState::Completed {
                    dependencies_completed = false;
                    break;
                }
            }

            if dependencies_completed {
                ready.push(record);
            }
        }

        Ok(ready)
    }

    /// Returns deterministic iteration over graph records.
    pub fn iter(&self) -> impl Iterator<Item = (&TaskId, &TaskRecord)> {
        self.tasks.iter()
    }

    /// Returns the number of tasks.
    #[must_use]
    pub fn len(&self) -> usize {
        self.tasks.len()
    }

    /// Returns whether the graph contains no tasks.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.tasks.is_empty()
    }

    fn visit(
        &self,
        task_id: &TaskId,
        visiting: &mut BTreeSet<TaskId>,
        visited: &mut BTreeSet<TaskId>,
    ) -> Result<(), TaskGraphError> {
        if visited.contains(task_id) {
            return Ok(());
        }

        if !visiting.insert(task_id.clone()) {
            return Err(TaskGraphError::CycleDetected {
                task_id: task_id.clone(),
            });
        }

        let record = self
            .tasks
            .get(task_id)
            .ok_or_else(|| TaskGraphError::UnknownTask {
                task_id: task_id.clone(),
            })?;

        for dependency in &record.task.dependency_task_ids {
            let dependency_id = TaskId::from_str(dependency)?;

            if visiting.contains(&dependency_id) {
                return Err(TaskGraphError::CycleDetected {
                    task_id: dependency_id,
                });
            }

            self.visit(&dependency_id, visiting, visited)?;
        }

        visiting.remove(task_id);
        visited.insert(task_id.clone());

        Ok(())
    }
}

/// Task identity or graph validation failure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TaskGraphError {
    /// Milestone identity was empty.
    EmptyMilestoneId,
    /// Stable task key was empty.
    EmptyStableKey,
    /// A task ID was not in canonical AgentForge form.
    InvalidTaskId {
        /// Invalid textual value.
        value: String,
    },
    /// An existing task contract was structurally invalid.
    Contract(TaskContractError),
    /// Stored task ID differs from deterministic derivation.
    TaskIdMismatch {
        /// Deterministically expected identity.
        expected: TaskId,
        /// Identity stored in the task contract.
        found: String,
    },
    /// A task ID already exists.
    DuplicateTask {
        /// Duplicate task identity.
        task_id: TaskId,
    },
    /// A milestone/stable-key pair already exists.
    DuplicateStableIdentity {
        /// Owning milestone.
        milestone_id: String,
        /// Stable key.
        stable_key: String,
    },
    /// A task directly depends on itself.
    SelfDependency {
        /// Self-dependent task.
        task_id: TaskId,
    },
    /// A task references a dependency absent from the graph.
    MissingDependency {
        /// Task containing the dependency reference.
        task_id: TaskId,
        /// Missing dependency identity.
        dependency_id: TaskId,
    },
    /// A requested task is absent.
    UnknownTask {
        /// Missing task identity.
        task_id: TaskId,
    },
    /// Dependency graph contains a cycle.
    CycleDetected {
        /// Task encountered while already being visited.
        task_id: TaskId,
    },
}

impl fmt::Display for TaskGraphError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyMilestoneId => formatter.write_str("milestone ID must not be empty"),
            Self::EmptyStableKey => formatter.write_str("stable task key must not be empty"),
            Self::InvalidTaskId { value } => {
                write!(formatter, "invalid deterministic task ID: {value}")
            }
            Self::Contract(error) => write!(formatter, "invalid agent task contract: {error}"),
            Self::TaskIdMismatch { expected, found } => write!(
                formatter,
                "stored task ID {found} does not match deterministic identity {expected}"
            ),
            Self::DuplicateTask { task_id } => {
                write!(formatter, "duplicate task ID: {task_id}")
            }
            Self::DuplicateStableIdentity {
                milestone_id,
                stable_key,
            } => write!(
                formatter,
                "duplicate stable identity in {milestone_id}: {stable_key}"
            ),
            Self::SelfDependency { task_id } => {
                write!(formatter, "task depends on itself: {task_id}")
            }
            Self::MissingDependency {
                task_id,
                dependency_id,
            } => write!(
                formatter,
                "task {task_id} references missing dependency {dependency_id}"
            ),
            Self::UnknownTask { task_id } => write!(formatter, "unknown task: {task_id}"),
            Self::CycleDetected { task_id } => {
                write!(formatter, "dependency cycle detected at task {task_id}")
            }
        }
    }
}

impl std::error::Error for TaskGraphError {}

#[cfg(test)]
mod tests {
    use super::{TaskGraph, TaskGraphError, TaskRecord, TaskState, derive_task_id};
    use crate::agent::{AgentRole, AgentTask};

    fn record(stable_key: &str, dependencies: &[String], state: TaskState) -> TaskRecord {
        let task_id = derive_task_id("P0-M004", stable_key).expect("identity should derive");

        let mut task = AgentTask::new(
            task_id.to_string(),
            "P0-M004",
            AgentRole::Implementer,
            format!("Implement {stable_key}"),
        );

        task.dependency_task_ids = dependencies.to_vec();

        TaskRecord::from_task(stable_key, task, state).expect("record should be valid")
    }

    #[test]
    fn deterministic_identity_has_fixed_vector() {
        let task_id = derive_task_id("P0-M004", "root").expect("identity should derive");
        assert_eq!(task_id.as_str(), "task-e1f5ec3871e1621f");
    }

    #[test]
    fn deterministic_identity_changes_with_namespace() {
        let first = derive_task_id("P0-M004", "root").expect("identity should derive");
        let second = derive_task_id("P0-M005", "root").expect("identity should derive");
        let third = derive_task_id("P0-M004", "worker").expect("identity should derive");

        assert_ne!(first, second);
        assert_ne!(first, third);
    }

    #[test]
    fn mutable_goal_does_not_define_identity() {
        let task_id = derive_task_id("P0-M004", "root").expect("identity should derive");

        let first = AgentTask::new(
            task_id.to_string(),
            "P0-M004",
            AgentRole::Implementer,
            "First goal",
        );

        let second = AgentTask::new(
            task_id.to_string(),
            "P0-M004",
            AgentRole::Implementer,
            "Completely different goal",
        );

        assert_eq!(first.task_id, second.task_id);
    }

    #[test]
    fn valid_dag_passes_validation() {
        let root = record("root", &[], TaskState::Pending);
        let root_id = root.task_id().expect("root ID");

        let child = record("child", &[root_id.to_string()], TaskState::Pending);

        let mut graph = TaskGraph::new();
        graph.insert(child).expect("insert child");
        graph.insert(root).expect("insert root");

        assert!(graph.validate().is_ok());
    }

    #[test]
    fn missing_dependency_is_rejected() {
        let missing = derive_task_id("P0-M004", "missing").expect("missing ID");

        let child = record("child", &[missing.to_string()], TaskState::Pending);

        let child_id = child.task_id().expect("child ID");

        let mut graph = TaskGraph::new();
        graph.insert(child).expect("insert child");

        assert_eq!(
            graph.validate(),
            Err(TaskGraphError::MissingDependency {
                task_id: child_id,
                dependency_id: missing,
            })
        );
    }

    #[test]
    fn self_dependency_is_rejected() {
        let task_id = derive_task_id("P0-M004", "self").expect("task ID");

        let mut task = AgentTask::new(
            task_id.to_string(),
            "P0-M004",
            AgentRole::Implementer,
            "Self dependency test",
        );

        task.dependency_task_ids.push(task_id.to_string());

        assert_eq!(
            TaskRecord::from_task("self", task, TaskState::Pending),
            Err(TaskGraphError::SelfDependency { task_id })
        );
    }

    #[test]
    fn two_node_cycle_is_rejected() {
        let a_id = derive_task_id("P0-M004", "a").expect("a ID");
        let b_id = derive_task_id("P0-M004", "b").expect("b ID");

        let a = record("a", &[b_id.to_string()], TaskState::Pending);
        let b = record("b", &[a_id.to_string()], TaskState::Pending);

        let mut graph = TaskGraph::new();
        graph.insert(a).expect("insert a");
        graph.insert(b).expect("insert b");

        assert!(matches!(
            graph.validate(),
            Err(TaskGraphError::CycleDetected { .. })
        ));
    }

    #[test]
    fn longer_cycle_is_rejected() {
        let a_id = derive_task_id("P0-M004", "a").expect("a ID");
        let b_id = derive_task_id("P0-M004", "b").expect("b ID");
        let c_id = derive_task_id("P0-M004", "c").expect("c ID");

        let a = record("a", &[b_id.to_string()], TaskState::Pending);
        let b = record("b", &[c_id.to_string()], TaskState::Pending);
        let c = record("c", &[a_id.to_string()], TaskState::Pending);

        let mut graph = TaskGraph::new();
        graph.insert(c).expect("insert c");
        graph.insert(a).expect("insert a");
        graph.insert(b).expect("insert b");

        assert!(matches!(
            graph.validate(),
            Err(TaskGraphError::CycleDetected { .. })
        ));
    }

    #[test]
    fn readiness_requires_completed_dependencies() {
        let root = record("root", &[], TaskState::Pending);
        let root_id = root.task_id().expect("root ID");

        let child = record("child", &[root_id.to_string()], TaskState::Pending);
        let child_id = child.task_id().expect("child ID");

        let mut graph = TaskGraph::new();
        graph.insert(child).expect("insert child");
        graph.insert(root).expect("insert root");

        let ready = graph.ready_tasks().expect("valid graph");

        assert_eq!(ready.len(), 1);
        assert_eq!(ready[0].task().task_id, root_id.to_string());

        graph
            .transition(&root_id, TaskState::Completed)
            .expect("transition root");

        let ready = graph.ready_tasks().expect("valid graph");

        assert_eq!(ready.len(), 1);
        assert_eq!(ready[0].task().task_id, child_id.to_string());
    }

    #[test]
    fn failed_dependency_does_not_satisfy_readiness() {
        let root = record("root", &[], TaskState::Failed);
        let root_id = root.task_id().expect("root ID");

        let child = record("child", &[root_id.to_string()], TaskState::Pending);

        let mut graph = TaskGraph::new();
        graph.insert(root).expect("insert root");
        graph.insert(child).expect("insert child");

        assert!(graph.ready_tasks().expect("valid graph").is_empty());
    }

    #[test]
    fn cancelled_dependency_does_not_satisfy_readiness() {
        let root = record("root", &[], TaskState::Cancelled);
        let root_id = root.task_id().expect("root ID");

        let child = record("child", &[root_id.to_string()], TaskState::Pending);

        let mut graph = TaskGraph::new();
        graph.insert(root).expect("insert root");
        graph.insert(child).expect("insert child");

        assert!(graph.ready_tasks().expect("valid graph").is_empty());
    }

    #[test]
    fn running_and_terminal_tasks_are_not_newly_ready() {
        for state in [
            TaskState::Running,
            TaskState::Completed,
            TaskState::Failed,
            TaskState::Cancelled,
            TaskState::Blocked,
        ] {
            let mut graph = TaskGraph::new();
            graph
                .insert(record("root", &[], state))
                .expect("insert root");

            assert!(
                graph.ready_tasks().expect("valid graph").is_empty(),
                "state {state:?} unexpectedly appeared ready"
            );
        }
    }

    #[test]
    fn iteration_order_is_deterministic() {
        let mut first = TaskGraph::new();
        first
            .insert(record("zeta", &[], TaskState::Pending))
            .expect("zeta");
        first
            .insert(record("alpha", &[], TaskState::Pending))
            .expect("alpha");
        first
            .insert(record("middle", &[], TaskState::Pending))
            .expect("middle");

        let ids: Vec<_> = first.iter().map(|(id, _)| id.to_string()).collect();

        let mut sorted = ids.clone();
        sorted.sort();

        assert_eq!(ids, sorted);
    }
}
