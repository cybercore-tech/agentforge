//! Durable local state for AgentForge.
//!
//! Domain graph behavior remains in `agentforge-core`. This crate owns only the durable snapshot
//! envelope, deterministic codec, and local filesystem storage.

mod codec;
mod file_store;

pub use codec::{decode_snapshot, encode_snapshot};
pub use file_store::FileStateStore;

use agentforge_core::task_graph::{TaskGraph, TaskGraphError};
use std::fmt;

/// Current AgentForge durable-state format version.
pub const STATE_FORMAT_VERSION: u16 = 1;

/// Complete durable AgentForge orchestration snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StateSnapshot {
    /// Durable format version.
    pub format_version: u16,
    /// Monotonically increasing state generation.
    pub generation: u64,
    /// Complete task graph.
    pub graph: TaskGraph,
}

impl StateSnapshot {
    /// Constructs a validated state snapshot.
    ///
    /// # Errors
    ///
    /// Returns [`StateError`] for generation zero or an invalid task graph.
    pub fn new(generation: u64, graph: TaskGraph) -> Result<Self, StateError> {
        if generation == 0 {
            return Err(StateError::InvalidGeneration);
        }

        graph.validate().map_err(StateError::Graph)?;

        Ok(Self {
            format_version: STATE_FORMAT_VERSION,
            generation,
            graph,
        })
    }

    /// Validates the snapshot envelope and task graph.
    ///
    /// # Errors
    ///
    /// Returns [`StateError`] when the version, generation, or graph is invalid.
    pub fn validate(&self) -> Result<(), StateError> {
        if self.format_version != STATE_FORMAT_VERSION {
            return Err(StateError::UnsupportedVersion {
                found: self.format_version,
            });
        }

        if self.generation == 0 {
            return Err(StateError::InvalidGeneration);
        }

        self.graph.validate().map_err(StateError::Graph)
    }
}

/// Storage abstraction for complete AgentForge state snapshots.
pub trait StateStore {
    /// Loads authoritative state, returning `None` when no state has ever been committed.
    ///
    /// # Errors
    ///
    /// Returns [`StateError`] when authoritative state exists but cannot be loaded safely.
    fn load(&self) -> Result<Option<StateSnapshot>, StateError>;

    /// Atomically publishes a new state generation.
    ///
    /// # Errors
    ///
    /// Returns [`StateError`] when validation, encoding, filesystem I/O, or generation checks fail.
    fn save(&self, snapshot: &StateSnapshot) -> Result<(), StateError>;
}

/// Durable-state failure.
#[derive(Debug)]
pub enum StateError {
    /// Filesystem operation failed.
    Io(std::io::Error),
    /// State magic header was invalid.
    InvalidMagic,
    /// State format version is unsupported.
    UnsupportedVersion {
        /// Version found in the snapshot.
        found: u16,
    },
    /// Generation zero is invalid.
    InvalidGeneration,
    /// Attempted generation did not advance beyond persisted state.
    GenerationRegression {
        /// Current persisted generation.
        current: u64,
        /// Attempted new generation.
        attempted: u64,
    },
    /// Snapshot ended before a required field could be decoded.
    Truncated,
    /// Encoded collection or string length exceeded implementation bounds.
    InvalidLength {
        /// Kind of value whose length was invalid.
        kind: &'static str,
        /// Encoded length.
        value: u64,
    },
    /// Encoded string was not valid UTF-8.
    InvalidUtf8,
    /// Encoded enum identity was unknown.
    InvalidEnum {
        /// Enum category.
        kind: &'static str,
        /// Unknown value.
        value: String,
    },
    /// Snapshot body checksum did not match.
    ChecksumMismatch {
        /// Checksum stored in the snapshot.
        expected: u64,
        /// Checksum calculated from the body.
        found: u64,
    },
    /// Bytes remained after the complete snapshot body was decoded.
    TrailingData,
    /// Reconstructed task graph was invalid.
    Graph(TaskGraphError),
}

impl fmt::Display for StateError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "state I/O error: {error}"),
            Self::InvalidMagic => formatter.write_str("invalid AgentForge state magic"),
            Self::UnsupportedVersion { found } => {
                write!(formatter, "unsupported AgentForge state version {found}")
            }
            Self::InvalidGeneration => {
                formatter.write_str("state generation must be greater than zero")
            }
            Self::GenerationRegression { current, attempted } => write!(
                formatter,
                "state generation must advance beyond {current}; attempted {attempted}"
            ),
            Self::Truncated => formatter.write_str("truncated AgentForge state"),
            Self::InvalidLength { kind, value } => {
                write!(formatter, "invalid {kind} length: {value}")
            }
            Self::InvalidUtf8 => formatter.write_str("invalid UTF-8 in AgentForge state"),
            Self::InvalidEnum { kind, value } => {
                write!(
                    formatter,
                    "unknown {kind} value in AgentForge state: {value}"
                )
            }
            Self::ChecksumMismatch { expected, found } => write!(
                formatter,
                "AgentForge state checksum mismatch: stored {expected:016x}, calculated {found:016x}"
            ),
            Self::TrailingData => formatter.write_str("unexpected trailing AgentForge state data"),
            Self::Graph(error) => write!(formatter, "invalid persisted task graph: {error}"),
        }
    }
}

impl std::error::Error for StateError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Graph(error) => Some(error),
            _ => None,
        }
    }
}

impl From<std::io::Error> for StateError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}
