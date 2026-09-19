//! Deterministic task graph semantics for AgentForge.

use crate::agent::AgentTask;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

/// Validated stable task identifier.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TaskId(String);

impl TaskId {
    /// Parses an explicit task identifier.
    ///
    /// # Errors
    ///
    /// Returns TaskIdError when the identifier is empty, too long, or contains unsupported
    /// characters.
    pub fn parse(value: impl Into<String>) -> Result<Self, TaskIdError> {
        let value = value.into();

        if value.is_empty() {
            return Err(TaskIdError::Empty);
        }

        if value.len() > 128 {
            return Err(TaskIdError::TooLong { length: value.len() });
        }

        if let Some(character) = value
            .chars()
            .find(|character| !(character.is_ascii_alphanumeric() || matches!(character, '-' | '_')))
        {
            return Err(TaskIdError::InvalidCharacter { character });
        }

        Ok(Self(value))
    }

    /// Creates an AgentForge deterministic task ID from a milestone and non-zero sequence.
    ///
    /// # Errors
    ///
    /// Returns TaskIdError when the milestone is invalid or the sequence is zero.
    pub fn for_sequence(milestone: &str, sequence: u32) -> Result<Self, TaskIdError> {
        if sequence == 0 {
            return Err(TaskIdError::ZeroSequence);
        }

        Self::parse(format!("{milestone}-T{sequence:04}"))
    }

    /// Returns the identifier as text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for TaskId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Task-identifier validation failure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TaskIdError {
    /// Identifier is empty.
    Empty,
    /// Identifier exceeds the supported length.
    TooLong {
        /// Number of UTF-8 bytes found.
        length: usize,
    },
    /// Identifier contains an unsupported character.
    InvalidCharacter {
        /// First unsupported character.
        character: char,
    },
    /// Deterministic task sequences begin at one.
    ZeroSequence,
}

impl fmt::Display for TaskIdError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => formatter.write_str("task ID must not be empty"),
            Self::TooLong { length } => {
                write!(formatter, "task ID is too long: {length} bytes")
            }
            Self::InvalidCharacter { character } => {
                write!(formatter, "task ID contains unsupported character: {character:?}")
            }
            Self::ZeroSequence => formatter.write_str("task sequence must be greater than zero"),
        }
    }
}

impl std::error::Error for TaskIdError {}

/// Persisted lifecycle state for one task.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum TaskState {
    /// Task exists but is not currently executing.
    Pending,
    /// Task is currently executing.
    Running,
    /// Task completed successfully.
    Succeeded,
    /// Task execution failed and may be explicitly retried.
    Failed,
    /// Task cannot currently proceed and may be explicitly resumed.
    Blocked,
    /// Task was cancelled and is terminal.
    Cancelled,
}

impl TaskState {
    /// Returns the stable machine-readable state identity.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Running => "running",
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
            Self::Blocked => "blocked",
            Self::Cancelled => "cancelled",
        }
    }

    /// Returns whether the state is terminal in P0-M004.
    #[must_use]
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Succeeded | Self::Cancelled)
    }
}

/// One task plus durable lifecycle metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TaskRecord {
    id: TaskId,
    task: AgentTask,
    state: TaskState,
    revision: u64,
}

impl TaskRecord {
    /// Creates a new pending record at revision zero.
    ///
    /// # Errors
    ///
    /// Returns TaskGraphError when the task contract or task ID is invalid.
    pub fn new(task: AgentTask) -> Result<Self, TaskGraphError> {
        Self::restore(task, TaskState::Pending, 0)
    }

    /// Restores a record from durable state.
    ///
    /// # Errors
    ///
    /// Returns TaskGraphError when the task contract or task ID is invalid.
    pub fn restore(
        task: AgentTask,
        state: TaskState,
        revision: u64,
    ) -> Result<Self, TaskGraphError> {
        task.validate().map_err(|error| TaskGraphError::InvalidTask {
            task_id: task.task_id.clone(),
            reason: error.to_string(),
        })?;

        let id = TaskId::parse(task.task_id.clone()).map_err(|error| {
            TaskGraphError::InvalidTaskId {
                task_id: task.task_id.clone(),
                reason: error.to_string(),
            }
        })?;

        Ok(Self {
            id,
            task,
            state,
            revision,
        })
    }

    /// Returns the stable task ID.
    #[must_use]
    pub fn id(&self) -> &TaskId {
        &self.id
    }

    /// Returns the provider-neutral agent task contract.
    #[must_use]
    pub fn task(&self) -> &AgentTask {
        &self.task
    }

    /// Returns the persisted lifecycle state.
    #[must_use]
    pub const fn state(&self) -> TaskState {
        self.state
    }

    /// Returns the per-task lifecycle revision.
    #[must_use]
    pub const fn revision(&self) -> u64 {
        self.revision
    }
}

/// Deterministic validated dependency graph.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TaskGraph {
    tasks: BTreeMap<TaskId, TaskRecord>,
}

impl TaskGraph {
    /// Creates an empty task graph.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            tasks: BTreeMap::new(),
        }
    }

    /// Builds and validates a graph from task contracts.
    ///
    /// # Errors
    ///
    /// Returns TaskGraphError for invalid contracts, duplicate IDs, missing dependencies,
    /// self dependencies, or cycles.
    pub fn from_tasks<I>(tasks: I) -> Result<Self, TaskGraphError>
    where
        I: IntoIterator<Item = AgentTask>,
    {
        let mut records = Vec::new();

        for task in tasks {
            records.push(TaskRecord::new(task)?);
        }

        Self::from_records(records)
    }

    /// Builds and validates a graph from restored records.
    ///
    /// # Errors
    ///
    /// Returns TaskGraphError for invalid records, duplicate IDs, missing dependencies,
    /// self dependencies, or cycles.
    pub fn from_records<I>(records: I) -> Result<Self, TaskGraphError>
    where
        I: IntoIterator<Item = TaskRecord>,
    {
        let mut graph = Self::new();

        for record in records {
            let id = record.id.clone();

            if graph.tasks.insert(id.clone(), record).is_some() {
                return Err(TaskGraphError::DuplicateTask { task_id: id });
            }
        }

        graph.validate()?;
        Ok(graph)
    }

    /// Returns the number of tasks.
    #[must_use]
    pub fn len(&self) -> usize {
        self.tasks.len()
    }

    /// Returns whether the graph is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.tasks.is_empty()
    }

    /// Returns one task record by ID.
    #[must_use]
    pub fn get(&self, task_id: &TaskId) -> Option<&TaskRecord> {
        self.tasks.get(task_id)
    }

    /// Iterates task records in stable task-ID order.
    pub fn records(&self) -> impl Iterator<Item = &TaskRecord> {
        self.tasks.values()
    }

    /// Validates every graph invariant.
    ///
    /// # Errors
    ///
    /// Returns TaskGraphError when a task is invalid, a dependency is invalid/missing, or a
    /// dependency cycle exists.
    pub fn validate(&self) -> Result<(), TaskGraphError> {
        let mut dependency_counts = BTreeMap::<TaskId, usize>::new();
        let mut dependents = BTreeMap::<TaskId, Vec<TaskId>>::new();

        for (id, record) in &self.tasks {
            record
                .task
                .validate()
                .map_err(|error| TaskGraphError::InvalidTask {
                    task_id: id.to_string(),
                    reason: error.to_string(),
                })?;

            if record.task.task_id != id.as_str() {
                return Err(TaskGraphError::RecordIdentityMismatch {
                    key: id.clone(),
                    embedded: record.task.task_id.clone(),
                });
            }

            let mut seen_dependencies = BTreeSet::new();

            for raw_dependency in &record.task.dependency_task_ids {
                let dependency = TaskId::parse(raw_dependency.clone()).map_err(|error| {
                    TaskGraphError::InvalidDependencyId {
                        task_id: id.clone(),
                        dependency: raw_dependency.clone(),
                        reason: error.to_string(),
                    }
                })?;

                if dependency == *id {
                    return Err(TaskGraphError::SelfDependency {
                        task_id: id.clone(),
                    });
                }

                if !self.tasks.contains_key(&dependency) {
                    return Err(TaskGraphError::MissingDependency {
                        task_id: id.clone(),
                        dependency,
                    });
                }

                if !seen_dependencies.insert(dependency.clone()) {
                    return Err(TaskGraphError::DuplicateDependency {
                        task_id: id.clone(),
                        dependency,
                    });
                }

                dependents
                    .entry(dependency)
                    .or_default()
                    .push(id.clone());
            }

            dependency_counts.insert(id.clone(), seen_dependencies.len());
        }

        let mut ready = BTreeSet::new();

        for (id, count) in &dependency_counts {
            if *count == 0 {
                ready.insert(id.clone());
            }
        }

        let mut visited = 0usize;

        while let Some(id) = ready.pop_first() {
            visited += 1;

            if let Some(children) = dependents.get(&id) {
                for child in children {
                    let count = dependency_counts
                        .get_mut(child)
                        .expect("validated dependent must have a dependency count");
                    *count -= 1;

                    if *count == 0 {
                        ready.insert(child.clone());
                    }
                }
            }
        }

        if visited != self.tasks.len() {
            return Err(TaskGraphError::CycleDetected);
        }

        Ok(())
    }

    /// Returns pending tasks whose dependencies have all succeeded, in task-ID order.
    ///
    /// # Errors
    ///
    /// Returns TaskGraphError if the graph is structurally invalid.
    pub fn ready_task_ids(&self) -> Result<Vec<TaskId>, TaskGraphError> {
        self.validate()?;

        let mut ready = Vec::new();

        for (id, record) in &self.tasks {
            if record.state != TaskState::Pending {
                continue;
            }

            let all_succeeded = record.task.dependency_task_ids.iter().all(|raw_dependency| {
                let dependency =
                    TaskId::parse(raw_dependency.clone()).expect("validated dependency ID");
                self.tasks
                    .get(&dependency)
                    .is_some_and(|dependency_record| {
                        dependency_record.state == TaskState::Succeeded
                    })
            });

            if all_succeeded {
                ready.push(id.clone());
            }
        }

        Ok(ready)
    }

    /// Applies one validated lifecycle transition.
    ///
    /// # Errors
    ///
    /// Returns TaskTransitionError for a missing task, an invalid graph, an invalid transition,
    /// an attempt to run a non-ready task, or revision overflow.
    pub fn transition(
        &mut self,
        task_id: &TaskId,
        next: TaskState,
    ) -> Result<(), TaskTransitionError> {
        self.validate().map_err(TaskTransitionError::InvalidGraph)?;

        let current = self
            .tasks
            .get(task_id)
            .ok_or_else(|| TaskTransitionError::MissingTask {
                task_id: task_id.clone(),
            })?
            .state;

        if current == TaskState::Pending && next == TaskState::Running {
            let ready = self
                .ready_task_ids()
                .map_err(TaskTransitionError::InvalidGraph)?;

            if !ready.contains(task_id) {
                return Err(TaskTransitionError::NotReady {
                    task_id: task_id.clone(),
                });
            }
        }

        if !transition_allowed(current, next) {
            return Err(TaskTransitionError::InvalidTransition {
                task_id: task_id.clone(),
                from: current,
                to: next,
            });
        }

        let record = self
            .tasks
            .get_mut(task_id)
            .expect("task existence checked before mutation");

        record.revision = record
            .revision
            .checked_add(1)
            .ok_or_else(|| TaskTransitionError::RevisionOverflow {
                task_id: task_id.clone(),
            })?;
        record.state = next;

        Ok(())
    }
}

impl Default for TaskGraph {
    fn default() -> Self {
        Self::new()
    }
}

fn transition_allowed(from: TaskState, to: TaskState) -> bool {
    matches!(
        (from, to),
        (TaskState::Pending, TaskState::Running | TaskState::Blocked | TaskState::Cancelled)
            | (
                TaskState::Running,
                TaskState::Succeeded
                    | TaskState::Failed
                    | TaskState::Blocked
                    | TaskState::Cancelled
            )
            | (TaskState::Failed, TaskState::Pending | TaskState::Cancelled)
            | (TaskState::Blocked, TaskState::Pending | TaskState::Cancelled)
    )
}

/// Task-graph structural validation failure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TaskGraphError {
    /// One task contract is invalid.
    InvalidTask {
        /// Task ID as supplied by the contract.
        task_id: String,
        /// Validation reason.
        reason: String,
    },
    /// One task identifier is invalid.
    InvalidTaskId {
        /// Invalid task identifier.
        task_id: String,
        /// Validation reason.
        reason: String,
    },
    /// A restored record key does not match its embedded contract ID.
    RecordIdentityMismatch {
        /// Record map key.
        key: TaskId,
        /// Embedded task ID.
        embedded: String,
    },
    /// The same task ID was added more than once.
    DuplicateTask {
        /// Duplicate task ID.
        task_id: TaskId,
    },
    /// A dependency identifier is invalid.
    InvalidDependencyId {
        /// Task containing the dependency.
        task_id: TaskId,
        /// Invalid dependency text.
        dependency: String,
        /// Validation reason.
        reason: String,
    },
    /// A task depends on itself.
    SelfDependency {
        /// Self-dependent task ID.
        task_id: TaskId,
    },
    /// A task lists the same dependency more than once.
    DuplicateDependency {
        /// Task containing the duplicate dependency.
        task_id: TaskId,
        /// Duplicate dependency ID.
        dependency: TaskId,
    },
    /// A dependency target does not exist.
    MissingDependency {
        /// Task containing the missing dependency.
        task_id: TaskId,
        /// Missing dependency ID.
        dependency: TaskId,
    },
    /// At least one dependency cycle exists.
    CycleDetected,
}

impl fmt::Display for TaskGraphError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidTask { task_id, reason } => {
                write!(formatter, "invalid task {task_id}: {reason}")
            }
            Self::InvalidTaskId { task_id, reason } => {
                write!(formatter, "invalid task ID {task_id:?}: {reason}")
            }
            Self::RecordIdentityMismatch { key, embedded } => {
                write!(
                    formatter,
                    "task record identity mismatch: key {key}, embedded {embedded}"
                )
            }
            Self::DuplicateTask { task_id } => write!(formatter, "duplicate task: {task_id}"),
            Self::InvalidDependencyId {
                task_id,
                dependency,
                reason,
            } => write!(
                formatter,
                "task {task_id} has invalid dependency {dependency:?}: {reason}"
            ),
            Self::SelfDependency { task_id } => {
                write!(formatter, "task depends on itself: {task_id}")
            }
            Self::DuplicateDependency {
                task_id,
                dependency,
            } => write!(
                formatter,
                "task {task_id} lists dependency {dependency} more than once"
            ),
            Self::MissingDependency {
                task_id,
                dependency,
            } => write!(
                formatter,
                "task {task_id} depends on missing task {dependency}"
            ),
            Self::CycleDetected => formatter.write_str("task dependency cycle detected"),
        }
    }
}

impl std::error::Error for TaskGraphError {}

/// Lifecycle transition failure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TaskTransitionError {
    /// The graph is structurally invalid.
    InvalidGraph(TaskGraphError),
    /// The task does not exist.
    MissingTask {
        /// Missing task ID.
        task_id: TaskId,
    },
    /// The task has incomplete dependencies.
    NotReady {
        /// Task that cannot run yet.
        task_id: TaskId,
    },
    /// The requested lifecycle edge is not allowed.
    InvalidTransition {
        /// Task being changed.
        task_id: TaskId,
        /// Current state.
        from: TaskState,
        /// Requested next state.
        to: TaskState,
    },
    /// The per-task revision counter cannot be incremented.
    RevisionOverflow {
        /// Task whose revision overflowed.
        task_id: TaskId,
    },
}

impl fmt::Display for TaskTransitionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidGraph(error) => write!(formatter, "invalid task graph: {error}"),
            Self::MissingTask { task_id } => write!(formatter, "missing task: {task_id}"),
            Self::NotReady { task_id } => write!(formatter, "task is not ready: {task_id}"),
            Self::InvalidTransition { task_id, from, to } => write!(
                formatter,
                "invalid transition for {task_id}: {} -> {}",
                from.as_str(),
                to.as_str()
            ),
            Self::RevisionOverflow { task_id } => {
                write!(formatter, "task revision overflow: {task_id}")
            }
        }
    }
}

impl std::error::Error for TaskTransitionError {}

#[cfg(test)]
mod tests {
    use super::{
        TaskGraph, TaskGraphError, TaskId, TaskIdError, TaskRecord, TaskState, TaskTransitionError,
    };
    use crate::agent::{AgentRole, AgentTask};

    fn task(id: &str, dependencies: &[&str]) -> AgentTask {
        let mut task = AgentTask::new(id, "P0-M004", AgentRole::Implementer, format!("Goal for {id}"));
        task.dependency_task_ids = dependencies.iter().map(|value| (*value).to_owned()).collect();
        task
    }

    #[test]
    fn deterministic_sequence_id_is_stable() {
        assert_eq!(
            TaskId::for_sequence("P0-M004", 1).expect("valid task ID").as_str(),
            "P0-M004-T0001"
        );
        assert_eq!(
            TaskId::for_sequence("P0-M004", 42)
                .expect("valid task ID")
                .as_str(),
            "P0-M004-T0042"
        );
    }

    #[test]
    fn sequence_zero_is_rejected() {
        assert_eq!(
            TaskId::for_sequence("P0-M004", 0),
            Err(TaskIdError::ZeroSequence)
        );
    }

    #[test]
    fn duplicate_task_is_rejected() {
        let first = TaskRecord::new(task("P0-M004-T0001", &[])).expect("valid record");
        let second = TaskRecord::new(task("P0-M004-T0001", &[])).expect("valid record");

        assert_eq!(
            TaskGraph::from_records([first, second]),
            Err(TaskGraphError::DuplicateTask {
                task_id: TaskId::parse("P0-M004-T0001").expect("valid ID"),
            })
        );
    }

    #[test]
    fn missing_dependency_is_rejected() {
        assert!(matches!(
            TaskGraph::from_tasks([task("P0-M004-T0002", &["P0-M004-T0001"])]),
            Err(TaskGraphError::MissingDependency { .. })
        ));
    }

    #[test]
    fn self_dependency_is_rejected() {
        assert!(matches!(
            TaskGraph::from_tasks([task("P0-M004-T0001", &["P0-M004-T0001"])]),
            Err(TaskGraphError::SelfDependency { .. })
        ));
    }

    #[test]
    fn cycle_is_rejected() {
        assert_eq!(
            TaskGraph::from_tasks([
                task("P0-M004-T0001", &["P0-M004-T0002"]),
                task("P0-M004-T0002", &["P0-M004-T0001"]),
            ]),
            Err(TaskGraphError::CycleDetected)
        );
    }

    #[test]
    fn ready_tasks_are_derived_from_dependencies() {
        let mut graph = TaskGraph::from_tasks([
            task("P0-M004-T0001", &[]),
            task("P0-M004-T0002", &["P0-M004-T0001"]),
        ])
        .expect("valid graph");

        assert_eq!(
            graph
                .ready_task_ids()
                .expect("valid graph")
                .iter()
                .map(TaskId::as_str)
                .collect::<Vec<_>>(),
            vec!["P0-M004-T0001"]
        );

        let first = TaskId::parse("P0-M004-T0001").expect("valid ID");
        graph
            .transition(&first, TaskState::Running)
            .expect("ready task starts");
        graph
            .transition(&first, TaskState::Succeeded)
            .expect("running task succeeds");

        assert_eq!(
            graph
                .ready_task_ids()
                .expect("valid graph")
                .iter()
                .map(TaskId::as_str)
                .collect::<Vec<_>>(),
            vec!["P0-M004-T0002"]
        );
    }

    #[test]
    fn task_cannot_run_before_dependencies_succeed() {
        let mut graph = TaskGraph::from_tasks([
            task("P0-M004-T0001", &[]),
            task("P0-M004-T0002", &["P0-M004-T0001"]),
        ])
        .expect("valid graph");

        let second = TaskId::parse("P0-M004-T0002").expect("valid ID");

        assert_eq!(
            graph.transition(&second, TaskState::Running),
            Err(TaskTransitionError::NotReady { task_id: second })
        );
    }

    #[test]
    fn accepted_transition_increments_revision_once() {
        let mut graph =
            TaskGraph::from_tasks([task("P0-M004-T0001", &[])]).expect("valid graph");
        let id = TaskId::parse("P0-M004-T0001").expect("valid ID");

        assert_eq!(graph.get(&id).expect("record").revision(), 0);

        graph
            .transition(&id, TaskState::Running)
            .expect("ready task starts");

        assert_eq!(graph.get(&id).expect("record").revision(), 1);
    }

    #[test]
    fn failed_and_blocked_tasks_can_explicitly_retry() {
        let id = TaskId::parse("P0-M004-T0001").expect("valid ID");

        for state in [TaskState::Failed, TaskState::Blocked] {
            let record = TaskRecord::restore(task(id.as_str(), &[]), state, 8)
                .expect("valid restored record");
            let mut graph = TaskGraph::from_records([record]).expect("valid graph");

            graph
                .transition(&id, TaskState::Pending)
                .expect("retry transition");

            assert_eq!(graph.get(&id).expect("record").state(), TaskState::Pending);
            assert_eq!(graph.get(&id).expect("record").revision(), 9);
        }
    }

    #[test]
    fn terminal_states_do_not_restart() {
        let id = TaskId::parse("P0-M004-T0001").expect("valid ID");

        for state in [TaskState::Succeeded, TaskState::Cancelled] {
            let record = TaskRecord::restore(task(id.as_str(), &[]), state, 2)
                .expect("valid restored record");
            let mut graph = TaskGraph::from_records([record]).expect("valid graph");

            assert!(matches!(
                graph.transition(&id, TaskState::Running),
                Err(TaskTransitionError::InvalidTransition { .. })
            ));
        }
    }

    #[test]
    fn records_iterate_in_task_id_order() {
        let graph = TaskGraph::from_tasks([
            task("P0-M004-T0003", &[]),
            task("P0-M004-T0001", &[]),
            task("P0-M004-T0002", &[]),
        ])
        .expect("valid graph");

        assert_eq!(
            graph
                .records()
                .map(|record| record.id().as_str())
                .collect::<Vec<_>>(),
            vec![
                "P0-M004-T0001",
                "P0-M004-T0002",
                "P0-M004-T0003"
            ]
        );
    }
}
