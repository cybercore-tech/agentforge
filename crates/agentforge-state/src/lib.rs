//! Durable project-local task state for AgentForge.
//!
//! This crate owns persistence mechanics. Task identity, lifecycle, dependencies, and readiness
//! remain owned by agentforge-core.

use agentforge_core::agent::{AgentRole, AgentTask, ApprovalBoundary, Capability};
use agentforge_core::task::{TaskGraph, TaskGraphError, TaskRecord, TaskState};
use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

const MAGIC: &[u8; 8] = b"AFGST01\0";
const HEADER_BYTES: usize = 18;
const CHECKSUM_BYTES: usize = 8;
const MAX_SNAPSHOT_BYTES: usize = 64 * 1024 * 1024;
const MAX_STRING_BYTES: usize = 1024 * 1024;
const MAX_TASKS: usize = 100_000;
const MAX_LIST_ITEMS: usize = 100_000;
static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// Current durable task-state snapshot schema version.
pub const SNAPSHOT_VERSION: u16 = 1;

/// Default project-relative task-state snapshot path.
pub const DEFAULT_RELATIVE_PATH: &str = ".forge/state/tasks.snapshot";

/// Persistence boundary for task graphs.
pub trait TaskStore {
    /// Loads the current graph, returning None when no durable state exists.
    ///
    /// # Errors
    ///
    /// Returns StateError for I/O, snapshot-format, or restored-domain failures.
    fn load(&self) -> Result<Option<TaskGraph>, StateError>;

    /// Persists one validated graph.
    ///
    /// # Errors
    ///
    /// Returns StateError for invalid graph state, encoding limits, or I/O failures.
    fn save(&self, graph: &TaskGraph) -> Result<(), StateError>;
}

/// Project-local file implementation of TaskStore.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FileTaskStore {
    path: PathBuf,
}

impl FileTaskStore {
    /// Creates a store using the default .forge state path beneath a project root.
    #[must_use]
    pub fn for_project_root(root: impl AsRef<Path>) -> Self {
        Self {
            path: root.as_ref().join(DEFAULT_RELATIVE_PATH),
        }
    }

    /// Creates a store for an explicit snapshot path.
    #[must_use]
    pub fn from_path(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    /// Returns the durable snapshot path.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    fn temporary_path(&self) -> PathBuf {
        let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let file_name = self
            .path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("tasks.snapshot");

        self.path.with_file_name(format!(
            ".{file_name}.tmp-{}-{sequence}",
            std::process::id()
        ))
    }
}

impl TaskStore for FileTaskStore {
    fn load(&self) -> Result<Option<TaskGraph>, StateError> {
        let metadata = match fs::metadata(&self.path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(StateError::Io(error)),
        };

        let maximum_file_bytes = MAX_SNAPSHOT_BYTES + HEADER_BYTES + CHECKSUM_BYTES;
        if metadata.len() > maximum_file_bytes as u64 {
            return Err(StateError::Format(StateFormatError::SnapshotTooLarge {
                declared: metadata.len(),
                maximum: maximum_file_bytes as u64,
            }));
        }

        let bytes = fs::read(&self.path)?;
        decode_snapshot(&bytes).map(Some)
    }

    fn save(&self, graph: &TaskGraph) -> Result<(), StateError> {
        graph.validate().map_err(StateError::Graph)?;
        let bytes = encode_snapshot(graph)?;

        if let Some(parent) = self.path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent)?;
            }
        }

        let temporary = self.temporary_path();

        let save_result = (|| -> Result<(), StateError> {
            let mut file = OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&temporary)?;
            file.write_all(&bytes)?;
            file.sync_all()?;
            drop(file);

            fs::rename(&temporary, &self.path)?;
            sync_parent_directory(&self.path)?;
            Ok(())
        })();

        if save_result.is_err() {
            let _ = fs::remove_file(&temporary);
        }

        save_result
    }
}

#[cfg(unix)]
fn sync_parent_directory(path: &Path) -> Result<(), StateError> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            File::open(parent)?.sync_all()?;
        }
    }

    Ok(())
}

#[cfg(not(unix))]
fn sync_parent_directory(_path: &Path) -> Result<(), StateError> {
    Ok(())
}

/// Durable-state persistence failure.
#[derive(Debug)]
pub enum StateError {
    /// Filesystem or file I/O failure.
    Io(std::io::Error),
    /// Snapshot encoding/decoding failure.
    Format(StateFormatError),
    /// Restored or saved graph violates domain invariants.
    Graph(TaskGraphError),
}

impl fmt::Display for StateError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "state I/O error: {error}"),
            Self::Format(error) => write!(formatter, "state format error: {error}"),
            Self::Graph(error) => write!(formatter, "state graph error: {error}"),
        }
    }
}

impl std::error::Error for StateError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Format(error) => Some(error),
            Self::Graph(error) => Some(error),
        }
    }
}

impl From<std::io::Error> for StateError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

/// Versioned snapshot format failure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StateFormatError {
    /// Snapshot magic bytes are invalid.
    BadMagic,
    /// Snapshot schema version is not supported.
    UnsupportedVersion {
        /// Version declared by the file.
        found: u16,
    },
    /// Snapshot declares or contains too much data.
    SnapshotTooLarge {
        /// Declared or observed byte count.
        declared: u64,
        /// Maximum accepted byte count.
        maximum: u64,
    },
    /// Snapshot payload checksum does not match.
    ChecksumMismatch,
    /// Snapshot ended before a complete value could be read.
    UnexpectedEof,
    /// A length cannot be represented safely.
    LengthOverflow,
    /// A string exceeds the configured decode bound.
    StringTooLarge {
        /// Declared string size.
        declared: u64,
        /// Maximum supported string size.
        maximum: u64,
    },
    /// A collection exceeds the configured decode bound.
    ListTooLarge {
        /// Declared item count.
        declared: u64,
        /// Maximum supported item count.
        maximum: u64,
    },
    /// A string is not valid UTF-8.
    InvalidUtf8,
    /// A persisted enum tag is unknown.
    InvalidTag {
        /// Logical enum name.
        kind: &'static str,
        /// Invalid numeric tag.
        tag: u8,
    },
    /// Bytes remained after the expected snapshot ended.
    TrailingBytes,
}

impl fmt::Display for StateFormatError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BadMagic => formatter.write_str("bad snapshot magic"),
            Self::UnsupportedVersion { found } => {
                write!(formatter, "unsupported snapshot version: {found}")
            }
            Self::SnapshotTooLarge { declared, maximum } => write!(
                formatter,
                "snapshot is too large: {declared} bytes exceeds {maximum}"
            ),
            Self::ChecksumMismatch => formatter.write_str("snapshot checksum mismatch"),
            Self::UnexpectedEof => formatter.write_str("unexpected end of snapshot"),
            Self::LengthOverflow => formatter.write_str("snapshot length overflow"),
            Self::StringTooLarge { declared, maximum } => write!(
                formatter,
                "snapshot string is too large: {declared} bytes exceeds {maximum}"
            ),
            Self::ListTooLarge { declared, maximum } => write!(
                formatter,
                "snapshot list is too large: {declared} items exceeds {maximum}"
            ),
            Self::InvalidUtf8 => formatter.write_str("snapshot contains invalid UTF-8"),
            Self::InvalidTag { kind, tag } => {
                write!(formatter, "invalid {kind} tag: {tag}")
            }
            Self::TrailingBytes => formatter.write_str("snapshot contains trailing bytes"),
        }
    }
}

impl std::error::Error for StateFormatError {}

fn encode_snapshot(graph: &TaskGraph) -> Result<Vec<u8>, StateError> {
    graph.validate().map_err(StateError::Graph)?;

    let mut payload = Vec::new();
    write_count(&mut payload, graph.len(), MAX_TASKS)?;

    for record in graph.records() {
        encode_record(&mut payload, record)?;
    }

    wrap_payload(payload)
}

fn wrap_payload(payload: Vec<u8>) -> Result<Vec<u8>, StateError> {
    if payload.len() > MAX_SNAPSHOT_BYTES {
        return Err(StateError::Format(StateFormatError::SnapshotTooLarge {
            declared: payload.len() as u64,
            maximum: MAX_SNAPSHOT_BYTES as u64,
        }));
    }

    let checksum = checksum(&payload);
    let mut bytes =
        Vec::with_capacity(HEADER_BYTES + payload.len().saturating_add(CHECKSUM_BYTES));
    bytes.extend_from_slice(MAGIC);
    write_u16(&mut bytes, SNAPSHOT_VERSION);
    write_u64(&mut bytes, payload.len() as u64);
    bytes.extend_from_slice(&payload);
    write_u64(&mut bytes, checksum);
    Ok(bytes)
}

fn decode_snapshot(bytes: &[u8]) -> Result<TaskGraph, StateError> {
    let mut reader = Reader::new(bytes);

    if reader.take(MAGIC.len())? != MAGIC {
        return Err(StateError::Format(StateFormatError::BadMagic));
    }

    let version = reader.read_u16()?;
    if version != SNAPSHOT_VERSION {
        return Err(StateError::Format(StateFormatError::UnsupportedVersion {
            found: version,
        }));
    }

    let payload_length = reader.read_u64()?;
    if payload_length > MAX_SNAPSHOT_BYTES as u64 {
        return Err(StateError::Format(StateFormatError::SnapshotTooLarge {
            declared: payload_length,
            maximum: MAX_SNAPSHOT_BYTES as u64,
        }));
    }

    let payload_length =
        usize::try_from(payload_length).map_err(|_| StateFormatError::LengthOverflow)?;
    let payload = reader.take(payload_length)?;
    let expected_checksum = reader.read_u64()?;

    if !reader.is_empty() {
        return Err(StateError::Format(StateFormatError::TrailingBytes));
    }

    if checksum(payload) != expected_checksum {
        return Err(StateError::Format(StateFormatError::ChecksumMismatch));
    }

    decode_payload(payload)
}

fn decode_payload(payload: &[u8]) -> Result<TaskGraph, StateError> {
    let mut reader = Reader::new(payload);
    let task_count = reader.read_count(MAX_TASKS)?;
    let mut records = Vec::with_capacity(task_count);

    for _ in 0..task_count {
        records.push(decode_record(&mut reader)?);
    }

    if !reader.is_empty() {
        return Err(StateError::Format(StateFormatError::TrailingBytes));
    }

    TaskGraph::from_records(records).map_err(StateError::Graph)
}

fn encode_record(bytes: &mut Vec<u8>, record: &TaskRecord) -> Result<(), StateError> {
    encode_agent_task(bytes, record.task())?;
    write_u8(bytes, encode_task_state(record.state()));
    write_u64(bytes, record.revision());
    Ok(())
}

fn decode_record(reader: &mut Reader<'_>) -> Result<TaskRecord, StateError> {
    let task = decode_agent_task(reader)?;
    let state = decode_task_state(reader.read_u8()?)?;
    let revision = reader.read_u64()?;
    TaskRecord::restore(task, state, revision).map_err(StateError::Graph)
}

fn encode_agent_task(bytes: &mut Vec<u8>, task: &AgentTask) -> Result<(), StateError> {
    write_u16(bytes, task.contract_version);
    write_string(bytes, &task.task_id)?;
    write_string(bytes, &task.milestone_id)?;
    write_u8(bytes, encode_agent_role(task.primary_role));
    write_string(bytes, &task.goal)?;
    write_strings(bytes, &task.non_goals)?;
    write_strings(bytes, &task.dependency_task_ids)?;
    write_strings(bytes, &task.allowed_paths)?;
    write_strings(bytes, &task.forbidden_paths)?;

    write_count(bytes, task.capabilities.len(), MAX_LIST_ITEMS)?;
    for capability in &task.capabilities {
        write_u8(bytes, encode_capability(*capability));
    }

    write_count(bytes, task.required_approvals.len(), MAX_LIST_ITEMS)?;
    for approval in &task.required_approvals {
        write_u8(bytes, encode_approval(*approval));
    }

    write_strings(bytes, &task.required_gates)?;
    write_strings(bytes, &task.expected_outputs)?;
    write_strings(bytes, &task.evidence_requirements)?;
    Ok(())
}

fn decode_agent_task(reader: &mut Reader<'_>) -> Result<AgentTask, StateError> {
    let contract_version = reader.read_u16()?;
    let task_id = reader.read_string()?;
    let milestone_id = reader.read_string()?;
    let primary_role = decode_agent_role(reader.read_u8()?)?;
    let goal = reader.read_string()?;
    let non_goals = reader.read_strings()?;
    let dependency_task_ids = reader.read_strings()?;
    let allowed_paths = reader.read_strings()?;
    let forbidden_paths = reader.read_strings()?;

    let capability_count = reader.read_count(MAX_LIST_ITEMS)?;
    let mut capabilities = Vec::with_capacity(capability_count);
    for _ in 0..capability_count {
        capabilities.push(decode_capability(reader.read_u8()?)?);
    }

    let approval_count = reader.read_count(MAX_LIST_ITEMS)?;
    let mut required_approvals = Vec::with_capacity(approval_count);
    for _ in 0..approval_count {
        required_approvals.push(decode_approval(reader.read_u8()?)?);
    }

    let required_gates = reader.read_strings()?;
    let expected_outputs = reader.read_strings()?;
    let evidence_requirements = reader.read_strings()?;

    Ok(AgentTask {
        contract_version,
        task_id,
        milestone_id,
        primary_role,
        goal,
        non_goals,
        dependency_task_ids,
        allowed_paths,
        forbidden_paths,
        capabilities,
        required_approvals,
        required_gates,
        expected_outputs,
        evidence_requirements,
    })
}

fn write_strings(bytes: &mut Vec<u8>, values: &[String]) -> Result<(), StateError> {
    write_count(bytes, values.len(), MAX_LIST_ITEMS)?;

    for value in values {
        write_string(bytes, value)?;
    }

    Ok(())
}

fn write_string(bytes: &mut Vec<u8>, value: &str) -> Result<(), StateError> {
    if value.len() > MAX_STRING_BYTES {
        return Err(StateError::Format(StateFormatError::StringTooLarge {
            declared: value.len() as u64,
            maximum: MAX_STRING_BYTES as u64,
        }));
    }

    let length = u32::try_from(value.len()).map_err(|_| StateFormatError::LengthOverflow)?;
    write_u32(bytes, length);
    bytes.extend_from_slice(value.as_bytes());
    Ok(())
}

fn write_count(bytes: &mut Vec<u8>, value: usize, maximum: usize) -> Result<(), StateError> {
    if value > maximum {
        return Err(StateError::Format(StateFormatError::ListTooLarge {
            declared: value as u64,
            maximum: maximum as u64,
        }));
    }

    let value = u32::try_from(value).map_err(|_| StateFormatError::LengthOverflow)?;
    write_u32(bytes, value);
    Ok(())
}

fn write_u8(bytes: &mut Vec<u8>, value: u8) {
    bytes.push(value);
}

fn write_u16(bytes: &mut Vec<u8>, value: u16) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn write_u32(bytes: &mut Vec<u8>, value: u32) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn write_u64(bytes: &mut Vec<u8>, value: u64) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn checksum(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;

    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }

    hash
}

fn encode_agent_role(role: AgentRole) -> u8 {
    match role {
        AgentRole::Planner => 0,
        AgentRole::Architect => 1,
        AgentRole::Researcher => 2,
        AgentRole::Implementer => 3,
        AgentRole::Tester => 4,
        AgentRole::Reviewer => 5,
        AgentRole::SecurityReviewer => 6,
        AgentRole::Integrator => 7,
        AgentRole::ReleaseManager => 8,
    }
}

fn decode_agent_role(tag: u8) -> Result<AgentRole, StateError> {
    match tag {
        0 => Ok(AgentRole::Planner),
        1 => Ok(AgentRole::Architect),
        2 => Ok(AgentRole::Researcher),
        3 => Ok(AgentRole::Implementer),
        4 => Ok(AgentRole::Tester),
        5 => Ok(AgentRole::Reviewer),
        6 => Ok(AgentRole::SecurityReviewer),
        7 => Ok(AgentRole::Integrator),
        8 => Ok(AgentRole::ReleaseManager),
        _ => Err(invalid_tag("agent role", tag)),
    }
}

fn encode_capability(capability: Capability) -> u8 {
    match capability {
        Capability::ReadRepository => 0,
        Capability::WriteOwnedPaths => 1,
        Capability::RunLocalCommands => 2,
        Capability::UseNetwork => 3,
        Capability::ReadGitHub => 4,
        Capability::WriteGitHub => 5,
        Capability::ManageWorktrees => 6,
        Capability::ReadSecrets => 7,
        Capability::UseMcpTools => 8,
        Capability::CreatePullRequest => 9,
        Capability::MergeProtectedBranch => 10,
        Capability::DeployStaging => 11,
        Capability::DeployProduction => 12,
    }
}

fn decode_capability(tag: u8) -> Result<Capability, StateError> {
    match tag {
        0 => Ok(Capability::ReadRepository),
        1 => Ok(Capability::WriteOwnedPaths),
        2 => Ok(Capability::RunLocalCommands),
        3 => Ok(Capability::UseNetwork),
        4 => Ok(Capability::ReadGitHub),
        5 => Ok(Capability::WriteGitHub),
        6 => Ok(Capability::ManageWorktrees),
        7 => Ok(Capability::ReadSecrets),
        8 => Ok(Capability::UseMcpTools),
        9 => Ok(Capability::CreatePullRequest),
        10 => Ok(Capability::MergeProtectedBranch),
        11 => Ok(Capability::DeployStaging),
        12 => Ok(Capability::DeployProduction),
        _ => Err(invalid_tag("capability", tag)),
    }
}

fn encode_approval(approval: ApprovalBoundary) -> u8 {
    match approval {
        ApprovalBoundary::ActivateImplementationPlan => 0,
        ApprovalBoundary::ExpandTaskScope => 1,
        ApprovalBoundary::ChangeDependencies => 2,
        ApprovalBoundary::ElevateCapability => 3,
        ApprovalBoundary::AccessSecrets => 4,
        ApprovalBoundary::DestructiveDataMigration => 5,
        ApprovalBoundary::IrreversibleExternalChange => 6,
        ApprovalBoundary::MergeProtectedBranch => 7,
        ApprovalBoundary::PublishRelease => 8,
        ApprovalBoundary::DeployProduction => 9,
        ApprovalBoundary::ChangeGovernanceRules => 10,
    }
}

fn decode_approval(tag: u8) -> Result<ApprovalBoundary, StateError> {
    match tag {
        0 => Ok(ApprovalBoundary::ActivateImplementationPlan),
        1 => Ok(ApprovalBoundary::ExpandTaskScope),
        2 => Ok(ApprovalBoundary::ChangeDependencies),
        3 => Ok(ApprovalBoundary::ElevateCapability),
        4 => Ok(ApprovalBoundary::AccessSecrets),
        5 => Ok(ApprovalBoundary::DestructiveDataMigration),
        6 => Ok(ApprovalBoundary::IrreversibleExternalChange),
        7 => Ok(ApprovalBoundary::MergeProtectedBranch),
        8 => Ok(ApprovalBoundary::PublishRelease),
        9 => Ok(ApprovalBoundary::DeployProduction),
        10 => Ok(ApprovalBoundary::ChangeGovernanceRules),
        _ => Err(invalid_tag("approval boundary", tag)),
    }
}

fn encode_task_state(state: TaskState) -> u8 {
    match state {
        TaskState::Pending => 0,
        TaskState::Running => 1,
        TaskState::Succeeded => 2,
        TaskState::Failed => 3,
        TaskState::Blocked => 4,
        TaskState::Cancelled => 5,
    }
}

fn decode_task_state(tag: u8) -> Result<TaskState, StateError> {
    match tag {
        0 => Ok(TaskState::Pending),
        1 => Ok(TaskState::Running),
        2 => Ok(TaskState::Succeeded),
        3 => Ok(TaskState::Failed),
        4 => Ok(TaskState::Blocked),
        5 => Ok(TaskState::Cancelled),
        _ => Err(invalid_tag("task state", tag)),
    }
}

fn invalid_tag(kind: &'static str, tag: u8) -> StateError {
    StateError::Format(StateFormatError::InvalidTag { kind, tag })
}

struct Reader<'a> {
    bytes: &'a [u8],
    cursor: usize,
}

impl<'a> Reader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, cursor: 0 }
    }

    fn is_empty(&self) -> bool {
        self.cursor == self.bytes.len()
    }

    fn take(&mut self, length: usize) -> Result<&'a [u8], StateError> {
        let end = self
            .cursor
            .checked_add(length)
            .ok_or(StateFormatError::LengthOverflow)?;

        if end > self.bytes.len() {
            return Err(StateError::Format(StateFormatError::UnexpectedEof));
        }

        let slice = &self.bytes[self.cursor..end];
        self.cursor = end;
        Ok(slice)
    }

    fn read_u8(&mut self) -> Result<u8, StateError> {
        Ok(self.take(1)?[0])
    }

    fn read_u16(&mut self) -> Result<u16, StateError> {
        let bytes: [u8; 2] = self
            .take(2)?
            .try_into()
            .expect("reader returned exact u16 width");
        Ok(u16::from_le_bytes(bytes))
    }

    fn read_u32(&mut self) -> Result<u32, StateError> {
        let bytes: [u8; 4] = self
            .take(4)?
            .try_into()
            .expect("reader returned exact u32 width");
        Ok(u32::from_le_bytes(bytes))
    }

    fn read_u64(&mut self) -> Result<u64, StateError> {
        let bytes: [u8; 8] = self
            .take(8)?
            .try_into()
            .expect("reader returned exact u64 width");
        Ok(u64::from_le_bytes(bytes))
    }

    fn read_count(&mut self, maximum: usize) -> Result<usize, StateError> {
        let count = u64::from(self.read_u32()?);

        if count > maximum as u64 {
            return Err(StateError::Format(StateFormatError::ListTooLarge {
                declared: count,
                maximum: maximum as u64,
            }));
        }

        usize::try_from(count)
            .map_err(|_| StateError::Format(StateFormatError::LengthOverflow))
    }

    fn read_string(&mut self) -> Result<String, StateError> {
        let length = u64::from(self.read_u32()?);

        if length > MAX_STRING_BYTES as u64 {
            return Err(StateError::Format(StateFormatError::StringTooLarge {
                declared: length,
                maximum: MAX_STRING_BYTES as u64,
            }));
        }

        let length = usize::try_from(length)
            .map_err(|_| StateError::Format(StateFormatError::LengthOverflow))?;
        let bytes = self.take(length)?;
        let value = std::str::from_utf8(bytes)
            .map_err(|_| StateError::Format(StateFormatError::InvalidUtf8))?;
        Ok(value.to_owned())
    }

    fn read_strings(&mut self) -> Result<Vec<String>, StateError> {
        let count = self.read_count(MAX_LIST_ITEMS)?;
        let mut values = Vec::with_capacity(count);

        for _ in 0..count {
            values.push(self.read_string()?);
        }

        Ok(values)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CHECKSUM_BYTES, FileTaskStore, HEADER_BYTES, MAGIC, MAX_SNAPSHOT_BYTES, SNAPSHOT_VERSION,
        StateError, StateFormatError, TaskStore, checksum, decode_snapshot, encode_record,
        encode_snapshot, wrap_payload, write_count,
    };
    use agentforge_core::agent::{
        AgentRole, AgentTask, ApprovalBoundary, Capability,
    };
    use agentforge_core::task::{TaskGraph, TaskRecord, TaskState};
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEST_SEQUENCE: AtomicU64 = AtomicU64::new(0);

    fn task(id: &str, dependencies: &[&str]) -> AgentTask {
        let mut task = AgentTask::new(id, "P0-M004", AgentRole::Implementer, format!("Goal for {id}"));
        task.non_goals = vec!["Do not change unrelated files.".to_owned()];
        task.dependency_task_ids = dependencies.iter().map(|value| (*value).to_owned()).collect();
        task.allowed_paths = vec!["crates/**".to_owned()];
        task.forbidden_paths = vec!["secrets/**".to_owned()];
        task.capabilities = vec![
            Capability::ReadRepository,
            Capability::WriteOwnedPaths,
            Capability::RunLocalCommands,
        ];
        task.required_approvals = vec![ApprovalBoundary::ActivateImplementationPlan];
        task.required_gates = vec!["Stable code gate".to_owned()];
        task.expected_outputs = vec!["commit".to_owned()];
        task.evidence_requirements = vec!["exact-head CI".to_owned()];
        task
    }

    fn sample_graph() -> TaskGraph {
        let first = TaskRecord::restore(
            task("P0-M004-T0001", &[]),
            TaskState::Succeeded,
            2,
        )
        .expect("valid first record");
        let second = TaskRecord::restore(
            task("P0-M004-T0002", &["P0-M004-T0001"]),
            TaskState::Pending,
            0,
        )
        .expect("valid second record");

        TaskGraph::from_records([second, first]).expect("valid graph")
    }

    fn temp_root() -> PathBuf {
        let sequence = TEST_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "agentforge-state-test-{}-{sequence}",
            std::process::id()
        ))
    }

    #[test]
    fn snapshot_round_trip_preserves_semantics() {
        let graph = sample_graph();
        let bytes = encode_snapshot(&graph).expect("encode snapshot");
        let restored = decode_snapshot(&bytes).expect("decode snapshot");

        assert_eq!(restored, graph);
    }

    #[test]
    fn snapshot_bytes_are_deterministic() {
        let first = sample_graph();
        let mut reversed = first.records().cloned().collect::<Vec<_>>();
        reversed.reverse();
        let second =
            TaskGraph::from_records(reversed).expect("same valid graph in reverse insertion order");

        assert_eq!(
            encode_snapshot(&first).expect("first encoding"),
            encode_snapshot(&second).expect("second encoding")
        );
    }

    #[test]
    fn checksum_detects_payload_corruption() {
        let graph = sample_graph();
        let mut bytes = encode_snapshot(&graph).expect("encode snapshot");
        bytes[HEADER_BYTES] ^= 0x01;

        assert!(matches!(
            decode_snapshot(&bytes),
            Err(StateError::Format(StateFormatError::ChecksumMismatch))
        ));
    }

    #[test]
    fn unknown_version_is_rejected() {
        let graph = sample_graph();
        let mut bytes = encode_snapshot(&graph).expect("encode snapshot");
        bytes[MAGIC.len()..MAGIC.len() + 2].copy_from_slice(&(SNAPSHOT_VERSION + 1).to_le_bytes());

        assert!(matches!(
            decode_snapshot(&bytes),
            Err(StateError::Format(
                StateFormatError::UnsupportedVersion { .. }
            ))
        ));
    }

    #[test]
    fn truncated_snapshot_is_rejected() {
        let graph = sample_graph();
        let mut bytes = encode_snapshot(&graph).expect("encode snapshot");
        bytes.truncate(bytes.len() - CHECKSUM_BYTES + 1);

        assert!(matches!(
            decode_snapshot(&bytes),
            Err(StateError::Format(StateFormatError::UnexpectedEof))
                | Err(StateError::Format(StateFormatError::ChecksumMismatch))
        ));
    }

    #[test]
    fn oversized_declared_payload_is_rejected_before_allocation() {
        let graph = sample_graph();
        let mut bytes = encode_snapshot(&graph).expect("encode snapshot");
        let oversized = (MAX_SNAPSHOT_BYTES as u64) + 1;
        let start = MAGIC.len() + 2;
        bytes[start..start + 8].copy_from_slice(&oversized.to_le_bytes());

        assert_eq!(
            decode_snapshot(&bytes)
                .expect_err("oversized snapshot")
                .to_string(),
            StateError::Format(StateFormatError::SnapshotTooLarge {
                declared: oversized,
                maximum: MAX_SNAPSHOT_BYTES as u64,
            })
            .to_string()
        );
    }

    #[test]
    fn restored_domain_invalid_graph_is_rejected() {
        let record = TaskRecord::new(task(
            "P0-M004-T0002",
            &["P0-M004-T0001"],
        ))
        .expect("record contract itself is valid");

        let mut payload = Vec::new();
        write_count(&mut payload, 1, 100_000).expect("write count");
        encode_record(&mut payload, &record).expect("encode record");
        let bytes = wrap_payload(payload).expect("wrap payload");

        assert!(matches!(
            decode_snapshot(&bytes),
            Err(StateError::Graph(_))
        ));
    }

    #[test]
    fn file_store_creates_parent_and_replaces_snapshot() {
        let root = temp_root();
        let store = FileTaskStore::for_project_root(&root);
        let first = sample_graph();

        store.save(&first).expect("save first graph");
        assert!(store.path().is_file());
        assert_eq!(store.load().expect("load first graph"), Some(first.clone()));

        let empty = TaskGraph::new();
        store.save(&empty).expect("replace with empty graph");
        assert_eq!(store.load().expect("load empty graph"), Some(empty));

        fs::remove_dir_all(root).expect("cleanup temp state");
    }

    #[test]
    fn missing_state_returns_none() {
        let root = temp_root();
        let store = FileTaskStore::for_project_root(&root);

        assert_eq!(store.load().expect("missing state is valid"), None);
    }
}
