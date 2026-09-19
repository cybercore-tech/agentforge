use crate::{StateError, StateSnapshot, StateStore, decode_snapshot, encode_snapshot};
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

const STATE_DIRECTORY: &str = ".forge/state";
const STATE_FILE: &str = "state.afs";
const TEMP_FILE: &str = "state.afs.tmp";

/// Project-local durable state store.
#[derive(Clone, Debug)]
pub struct FileStateStore {
    directory: PathBuf,
}

impl FileStateStore {
    /// Creates a state store rooted at an AgentForge project directory.
    #[must_use]
    pub fn new(project_root: impl AsRef<Path>) -> Self {
        Self {
            directory: project_root.as_ref().join(STATE_DIRECTORY),
        }
    }

    /// Returns the authoritative state path.
    #[must_use]
    pub fn state_path(&self) -> PathBuf {
        self.directory.join(STATE_FILE)
    }

    fn temporary_path(&self) -> PathBuf {
        self.directory.join(TEMP_FILE)
    }
}

impl StateStore for FileStateStore {
    fn load(&self) -> Result<Option<StateSnapshot>, StateError> {
        let path = self.state_path();

        if !path.exists() {
            return Ok(None);
        }

        let bytes = fs::read(path)?;
        decode_snapshot(&bytes).map(Some)
    }

    fn save(&self, snapshot: &StateSnapshot) -> Result<(), StateError> {
        snapshot.validate()?;

        if let Some(current) = self.load()? {
            if snapshot.generation <= current.generation {
                return Err(StateError::GenerationRegression {
                    current: current.generation,
                    attempted: snapshot.generation,
                });
            }
        }

        let bytes = encode_snapshot(snapshot)?;

        fs::create_dir_all(&self.directory)?;

        let temporary_path = self.temporary_path();
        let final_path = self.state_path();

        let mut temporary = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&temporary_path)?;

        temporary.write_all(&bytes)?;
        temporary.flush()?;
        temporary.sync_all()?;
        drop(temporary);

        fs::rename(&temporary_path, &final_path)?;

        File::open(&self.directory)?.sync_all()?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{FileStateStore, StateStore};
    use crate::{StateError, StateSnapshot};
    use agentforge_core::agent::{AgentRole, AgentTask};
    use agentforge_core::task_graph::{TaskGraph, TaskRecord, TaskState, derive_task_id};
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(1);

    fn temporary_directory(test_name: &str) -> PathBuf {
        let sequence = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "agentforge-state-{}-{test_name}-{sequence}",
            std::process::id()
        ));

        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("create test directory");

        path
    }

    fn snapshot(generation: u64, state: TaskState) -> StateSnapshot {
        let task_id = derive_task_id("P0-M004", "store").expect("task ID");

        let task = AgentTask::new(
            task_id.to_string(),
            "P0-M004",
            AgentRole::Implementer,
            "Persist graph state.",
        );

        let record = TaskRecord::from_task("store", task, state).expect("task record");

        let mut graph = TaskGraph::new();
        graph.insert(record).expect("insert task");

        StateSnapshot::new(generation, graph).expect("snapshot")
    }

    #[test]
    fn missing_state_loads_as_none() {
        let directory = temporary_directory("missing");
        let store = FileStateStore::new(&directory);

        assert_eq!(store.load().expect("load"), None);

        fs::remove_dir_all(directory).expect("cleanup");
    }

    #[test]
    fn save_and_load_preserve_graph_state() {
        let directory = temporary_directory("round-trip");
        let store = FileStateStore::new(&directory);
        let expected = snapshot(1, TaskState::Running);

        store.save(&expected).expect("save");

        let loaded = store.load().expect("load").expect("stored state");

        assert_eq!(loaded, expected);

        fs::remove_dir_all(directory).expect("cleanup");
    }

    #[test]
    fn later_generation_replaces_authoritative_snapshot() {
        let directory = temporary_directory("generation");
        let store = FileStateStore::new(&directory);

        store
            .save(&snapshot(1, TaskState::Pending))
            .expect("first save");

        store
            .save(&snapshot(2, TaskState::Completed))
            .expect("second save");

        let loaded = store.load().expect("load").expect("state");

        assert_eq!(loaded.generation, 2);

        let state = loaded.graph.iter().next().expect("task").1.state();

        assert_eq!(state, TaskState::Completed);

        fs::remove_dir_all(directory).expect("cleanup");
    }

    #[test]
    fn generation_regression_is_rejected() {
        let directory = temporary_directory("regression");
        let store = FileStateStore::new(&directory);

        store
            .save(&snapshot(2, TaskState::Running))
            .expect("initial save");

        assert!(matches!(
            store.save(&snapshot(2, TaskState::Completed)),
            Err(StateError::GenerationRegression {
                current: 2,
                attempted: 2
            })
        ));

        fs::remove_dir_all(directory).expect("cleanup");
    }

    #[test]
    fn temporary_file_is_not_authoritative() {
        let directory = temporary_directory("temporary");
        let store = FileStateStore::new(&directory);

        let state_directory = directory.join(".forge/state");
        fs::create_dir_all(&state_directory).expect("state directory");
        fs::write(state_directory.join("state.afs.tmp"), b"incomplete").expect("temporary state");

        assert_eq!(store.load().expect("load"), None);

        fs::remove_dir_all(directory).expect("cleanup");
    }

    #[test]
    fn corrupt_authoritative_state_fails_closed() {
        let directory = temporary_directory("corrupt");
        let store = FileStateStore::new(&directory);

        store.save(&snapshot(1, TaskState::Pending)).expect("save");

        fs::write(store.state_path(), b"corrupt").expect("corrupt state");

        assert!(store.load().is_err());

        fs::remove_dir_all(directory).expect("cleanup");
    }
}
