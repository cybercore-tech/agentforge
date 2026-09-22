//! Transport-neutral remote-worker and task-lease semantics.
//!
//! This module defines the domain boundary for future distributed execution. It deliberately does
//! not open sockets, authenticate peers, persist state, or execute tasks. A future transport must
//! enforce these validated values while local AgentForge policy remains authoritative.

use crate::task::TaskId;
use std::collections::BTreeMap;
use std::fmt;

/// Current semantic version for the remote-worker contract.
pub const REMOTE_WORKER_PROTOCOL_VERSION: u16 = 1;
/// Maximum byte length for a worker identifier.
pub const MAX_WORKER_ID_BYTES: usize = 128;
/// Maximum byte length for a worker platform label.
pub const MAX_PLATFORM_BYTES: usize = 64;
/// Maximum byte length for one capability identity.
pub const MAX_CAPABILITY_BYTES: usize = 64;
/// Maximum number of capabilities a worker may advertise.
pub const MAX_CAPABILITIES: usize = 32;
/// Maximum number of concurrently active leases one worker may hold.
pub const MAX_CONCURRENT_LEASES: u16 = 256;
/// Maximum byte length for a lease identifier.
pub const MAX_LEASE_ID_BYTES: usize = 128;
/// Maximum duration of one lease or renewal.
pub const MAX_LEASE_DURATION_MS: u64 = 24 * 60 * 60 * 1_000;

/// Validated stable identity for a remote worker.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RemoteWorkerId(String);

impl RemoteWorkerId {
    /// Parses a worker identifier using the transport-neutral identifier grammar.
    pub fn parse(value: impl Into<String>) -> Result<Self, RemoteWorkerError> {
        let value = value.into();
        validate_identifier("worker ID", &value, MAX_WORKER_ID_BYTES)?;
        Ok(Self(value))
    }

    /// Returns the identifier as text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for RemoteWorkerId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Validated capability advertised by a remote worker.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct WorkerCapability(String);

impl WorkerCapability {
    /// Parses a lowercase, machine-readable capability identity.
    pub fn parse(value: impl Into<String>) -> Result<Self, RemoteWorkerError> {
        let value = value.into();
        validate_capability(&value)?;
        Ok(Self(value))
    }

    /// Returns the capability identity as text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for WorkerCapability {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Validated descriptor for one remote worker.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RemoteWorkerDescriptor {
    protocol_version: u16,
    worker_id: RemoteWorkerId,
    platform: String,
    capabilities: Vec<WorkerCapability>,
    max_concurrent_leases: u16,
}

impl RemoteWorkerDescriptor {
    /// Creates a descriptor using the current remote-worker protocol version.
    pub fn new(
        worker_id: RemoteWorkerId,
        platform: impl Into<String>,
        capabilities: Vec<WorkerCapability>,
        max_concurrent_leases: u16,
    ) -> Result<Self, RemoteWorkerError> {
        Self::with_protocol_version(
            REMOTE_WORKER_PROTOCOL_VERSION,
            worker_id,
            platform,
            capabilities,
            max_concurrent_leases,
        )
    }

    /// Creates a descriptor with an explicit protocol version for compatibility checks.
    pub fn with_protocol_version(
        protocol_version: u16,
        worker_id: RemoteWorkerId,
        platform: impl Into<String>,
        mut capabilities: Vec<WorkerCapability>,
        max_concurrent_leases: u16,
    ) -> Result<Self, RemoteWorkerError> {
        if protocol_version != REMOTE_WORKER_PROTOCOL_VERSION {
            return Err(RemoteWorkerError::UnsupportedProtocolVersion {
                found: protocol_version,
            });
        }

        let platform = platform.into();
        validate_identifier("platform", &platform, MAX_PLATFORM_BYTES)?;

        if capabilities.is_empty() {
            return Err(RemoteWorkerError::EmptyCapabilities);
        }
        if capabilities.len() > MAX_CAPABILITIES {
            return Err(RemoteWorkerError::TooManyCapabilities {
                count: capabilities.len(),
                maximum: MAX_CAPABILITIES,
            });
        }
        capabilities.sort();
        for pair in capabilities.windows(2) {
            if pair[0] == pair[1] {
                return Err(RemoteWorkerError::DuplicateCapability {
                    capability: pair[0].clone(),
                });
            }
        }

        if max_concurrent_leases == 0 || max_concurrent_leases > MAX_CONCURRENT_LEASES {
            return Err(RemoteWorkerError::InvalidConcurrency {
                found: max_concurrent_leases,
                maximum: MAX_CONCURRENT_LEASES,
            });
        }

        Ok(Self {
            protocol_version,
            worker_id,
            platform,
            capabilities,
            max_concurrent_leases,
        })
    }

    /// Returns the negotiated protocol version.
    #[must_use]
    pub const fn protocol_version(&self) -> u16 {
        self.protocol_version
    }

    /// Returns the worker identity.
    #[must_use]
    pub fn worker_id(&self) -> &RemoteWorkerId {
        &self.worker_id
    }

    /// Returns the validated platform label.
    #[must_use]
    pub fn platform(&self) -> &str {
        &self.platform
    }

    /// Returns capabilities in deterministic sorted order.
    #[must_use]
    pub fn capabilities(&self) -> &[WorkerCapability] {
        &self.capabilities
    }

    /// Returns the maximum number of active leases for this worker.
    #[must_use]
    pub const fn max_concurrent_leases(&self) -> u16 {
        self.max_concurrent_leases
    }
}

/// Validated identity for a task lease.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct LeaseId(String);

impl LeaseId {
    /// Parses a lease identifier.
    pub fn parse(value: impl Into<String>) -> Result<Self, RemoteWorkerError> {
        let value = value.into();
        validate_identifier("lease ID", &value, MAX_LEASE_ID_BYTES)?;
        Ok(Self(value))
    }

    /// Returns the identifier as text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for LeaseId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Terminal or active state of a task lease.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum LeaseState {
    /// The worker may continue holding the task until the expiry time.
    Active,
    /// The lease was explicitly released by its owner.
    Released,
    /// The lease was observed after its expiry time.
    Expired,
}

impl LeaseState {
    /// Returns the stable machine-readable state identity.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Released => "released",
            Self::Expired => "expired",
        }
    }
}

impl fmt::Display for LeaseState {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Immutable view of one task lease.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TaskLease {
    lease_id: LeaseId,
    task_id: TaskId,
    worker_id: RemoteWorkerId,
    generation: u64,
    issued_at_ms: u64,
    expires_at_ms: u64,
    state: LeaseState,
}

impl TaskLease {
    /// Returns the lease identity.
    #[must_use]
    pub fn lease_id(&self) -> &LeaseId {
        &self.lease_id
    }

    /// Returns the leased task identity.
    #[must_use]
    pub fn task_id(&self) -> &TaskId {
        &self.task_id
    }

    /// Returns the owning worker identity.
    #[must_use]
    pub fn worker_id(&self) -> &RemoteWorkerId {
        &self.worker_id
    }

    /// Returns the monotonically increasing task lease generation.
    #[must_use]
    pub const fn generation(&self) -> u64 {
        self.generation
    }

    /// Returns the issue timestamp supplied by the local authority.
    #[must_use]
    pub const fn issued_at_ms(&self) -> u64 {
        self.issued_at_ms
    }

    /// Returns the current expiry timestamp.
    #[must_use]
    pub const fn expires_at_ms(&self) -> u64 {
        self.expires_at_ms
    }

    /// Returns the lifecycle state.
    #[must_use]
    pub const fn state(&self) -> LeaseState {
        self.state
    }

    /// Returns whether the lease is active at the supplied observation time.
    #[must_use]
    pub fn is_active_at(&self, observed_at_ms: u64) -> bool {
        self.state == LeaseState::Active && observed_at_ms < self.expires_at_ms
    }
}

/// Deterministic in-memory state machine for task leases.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct LeaseBook {
    leases: BTreeMap<LeaseId, TaskLease>,
}

impl LeaseBook {
    /// Creates an empty lease book.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            leases: BTreeMap::new(),
        }
    }

    /// Grants one bounded lease to an eligible worker.
    pub fn grant(
        &mut self,
        worker: &RemoteWorkerDescriptor,
        lease_id: LeaseId,
        task_id: TaskId,
        issued_at_ms: u64,
        expires_at_ms: u64,
    ) -> Result<&TaskLease, RemoteWorkerError> {
        validate_lease_window(issued_at_ms, expires_at_ms)?;
        self.expire_due(issued_at_ms);

        if self.leases.contains_key(&lease_id) {
            return Err(RemoteWorkerError::DuplicateLease { lease_id });
        }

        if let Some(existing) = self.active_lease_for_task_at(&task_id, issued_at_ms) {
            return Err(RemoteWorkerError::TaskAlreadyLeased {
                task_id,
                lease_id: existing.lease_id.clone(),
            });
        }

        let active_for_worker = self
            .leases
            .values()
            .filter(|lease| {
                lease.worker_id == *worker.worker_id() && lease.is_active_at(issued_at_ms)
            })
            .count();
        if active_for_worker >= usize::from(worker.max_concurrent_leases()) {
            return Err(RemoteWorkerError::WorkerConcurrencyLimit {
                worker_id: worker.worker_id().clone(),
                maximum: worker.max_concurrent_leases(),
            });
        }

        let generation = self
            .leases
            .values()
            .filter(|lease| lease.task_id == task_id)
            .map(|lease| lease.generation)
            .max()
            .unwrap_or(0)
            .checked_add(1)
            .ok_or(RemoteWorkerError::GenerationExhausted)?;

        let lease = TaskLease {
            lease_id: lease_id.clone(),
            task_id,
            worker_id: worker.worker_id().clone(),
            generation,
            issued_at_ms,
            expires_at_ms,
            state: LeaseState::Active,
        };
        self.leases.insert(lease_id.clone(), lease);
        Ok(self
            .leases
            .get(&lease_id)
            .expect("inserted lease must be present"))
    }

    /// Renews an active lease for its owner and generation.
    pub fn renew(
        &mut self,
        lease_id: &LeaseId,
        worker_id: &RemoteWorkerId,
        generation: u64,
        observed_at_ms: u64,
        new_expires_at_ms: u64,
    ) -> Result<&TaskLease, RemoteWorkerError> {
        let lease =
            self.leases
                .get_mut(lease_id)
                .ok_or_else(|| RemoteWorkerError::LeaseNotFound {
                    lease_id: lease_id.clone(),
                })?;

        ensure_lease_owner(lease, worker_id, generation)?;
        if !lease.is_active_at(observed_at_ms) {
            if lease.state == LeaseState::Active {
                lease.state = LeaseState::Expired;
            }
            return Err(RemoteWorkerError::LeaseExpired {
                lease_id: lease_id.clone(),
            });
        }
        if new_expires_at_ms <= lease.expires_at_ms {
            return Err(RemoteWorkerError::RenewalNotExtended {
                current_expiry_ms: lease.expires_at_ms,
                requested_expiry_ms: new_expires_at_ms,
            });
        }
        validate_lease_window(observed_at_ms, new_expires_at_ms)?;
        lease.expires_at_ms = new_expires_at_ms;
        Ok(lease)
    }

    /// Releases an active lease for its owner and generation.
    pub fn release(
        &mut self,
        lease_id: &LeaseId,
        worker_id: &RemoteWorkerId,
        generation: u64,
        observed_at_ms: u64,
    ) -> Result<&TaskLease, RemoteWorkerError> {
        let lease =
            self.leases
                .get_mut(lease_id)
                .ok_or_else(|| RemoteWorkerError::LeaseNotFound {
                    lease_id: lease_id.clone(),
                })?;

        ensure_lease_owner(lease, worker_id, generation)?;
        if !lease.is_active_at(observed_at_ms) {
            if lease.state == LeaseState::Active {
                lease.state = LeaseState::Expired;
            }
            return Err(RemoteWorkerError::LeaseExpired {
                lease_id: lease_id.clone(),
            });
        }
        lease.state = LeaseState::Released;
        Ok(lease)
    }

    /// Marks every active lease due at the observation time as expired.
    pub fn expire_due(&mut self, observed_at_ms: u64) -> Vec<LeaseId> {
        let mut expired = Vec::new();
        for lease in self.leases.values_mut() {
            if lease.is_active_at(observed_at_ms) {
                continue;
            }
            if lease.state == LeaseState::Active {
                lease.state = LeaseState::Expired;
                expired.push(lease.lease_id.clone());
            }
        }
        expired
    }

    /// Returns one lease by identity without changing state.
    #[must_use]
    pub fn get(&self, lease_id: &LeaseId) -> Option<&TaskLease> {
        self.leases.get(lease_id)
    }

    /// Returns the active lease for a task at an observation time.
    #[must_use]
    pub fn active_lease_for_task_at(
        &self,
        task_id: &TaskId,
        observed_at_ms: u64,
    ) -> Option<&TaskLease> {
        self.leases
            .values()
            .find(|lease| lease.task_id == *task_id && lease.is_active_at(observed_at_ms))
    }

    /// Returns active leases in deterministic lease-identity order.
    #[must_use]
    pub fn active_leases_at(&self, observed_at_ms: u64) -> Vec<&TaskLease> {
        self.leases
            .values()
            .filter(|lease| lease.is_active_at(observed_at_ms))
            .collect()
    }
}

fn ensure_lease_owner(
    lease: &TaskLease,
    worker_id: &RemoteWorkerId,
    generation: u64,
) -> Result<(), RemoteWorkerError> {
    if lease.worker_id != *worker_id {
        return Err(RemoteWorkerError::LeaseOwnerMismatch {
            expected: lease.worker_id.clone(),
            found: worker_id.clone(),
        });
    }
    if lease.generation != generation {
        return Err(RemoteWorkerError::LeaseGenerationMismatch {
            expected: lease.generation,
            found: generation,
        });
    }
    Ok(())
}

fn validate_identifier(
    field: &'static str,
    value: &str,
    maximum: usize,
) -> Result<(), RemoteWorkerError> {
    if value.is_empty() {
        return Err(RemoteWorkerError::EmptyField { field });
    }
    if value.len() > maximum {
        return Err(RemoteWorkerError::FieldTooLong {
            field,
            length: value.len(),
            maximum,
        });
    }
    if let Some(character) = value.chars().find(|character| {
        !(character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.'))
    }) {
        return Err(RemoteWorkerError::InvalidCharacter { field, character });
    }
    Ok(())
}

fn validate_capability(value: &str) -> Result<(), RemoteWorkerError> {
    if value.is_empty() {
        return Err(RemoteWorkerError::EmptyField {
            field: "capability",
        });
    }
    if value.len() > MAX_CAPABILITY_BYTES {
        return Err(RemoteWorkerError::FieldTooLong {
            field: "capability",
            length: value.len(),
            maximum: MAX_CAPABILITY_BYTES,
        });
    }
    if let Some(character) = value.chars().find(|character| {
        !(character.is_ascii_lowercase()
            || character.is_ascii_digit()
            || matches!(character, '-' | '_' | '.'))
    }) {
        return Err(RemoteWorkerError::InvalidCharacter {
            field: "capability",
            character,
        });
    }
    Ok(())
}

fn validate_lease_window(issued_at_ms: u64, expires_at_ms: u64) -> Result<(), RemoteWorkerError> {
    let Some(duration_ms) = expires_at_ms.checked_sub(issued_at_ms) else {
        return Err(RemoteWorkerError::InvalidLeaseWindow {
            issued_at_ms,
            expires_at_ms,
        });
    };
    if duration_ms == 0 {
        return Err(RemoteWorkerError::InvalidLeaseWindow {
            issued_at_ms,
            expires_at_ms,
        });
    }
    if duration_ms > MAX_LEASE_DURATION_MS {
        return Err(RemoteWorkerError::LeaseDurationTooLong {
            duration_ms,
            maximum: MAX_LEASE_DURATION_MS,
        });
    }
    Ok(())
}

/// Validation and lease-transition failure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RemoteWorkerError {
    /// A bounded identifier field is empty.
    EmptyField {
        /// Field name.
        field: &'static str,
    },
    /// A bounded identifier field exceeds its maximum byte length.
    FieldTooLong {
        /// Field name.
        field: &'static str,
        /// Observed byte length.
        length: usize,
        /// Maximum permitted byte length.
        maximum: usize,
    },
    /// A field contains a character outside its identifier grammar.
    InvalidCharacter {
        /// Field name.
        field: &'static str,
        /// First invalid character.
        character: char,
    },
    /// The descriptor uses a protocol version this build does not understand.
    UnsupportedProtocolVersion {
        /// Version found in the descriptor.
        found: u16,
    },
    /// A worker must advertise at least one capability.
    EmptyCapabilities,
    /// The capability list exceeds its bound.
    TooManyCapabilities {
        /// Observed capability count.
        count: usize,
        /// Maximum permitted count.
        maximum: usize,
    },
    /// A capability appears more than once.
    DuplicateCapability {
        /// Duplicated capability.
        capability: WorkerCapability,
    },
    /// The worker concurrency limit is outside its supported range.
    InvalidConcurrency {
        /// Observed limit.
        found: u16,
        /// Maximum permitted limit.
        maximum: u16,
    },
    /// A lease window is empty or runs backward.
    InvalidLeaseWindow {
        /// Issue timestamp.
        issued_at_ms: u64,
        /// Expiry timestamp.
        expires_at_ms: u64,
    },
    /// A lease window exceeds the protocol maximum.
    LeaseDurationTooLong {
        /// Observed duration.
        duration_ms: u64,
        /// Maximum permitted duration.
        maximum: u64,
    },
    /// A lease identity is already present, including terminal leases.
    DuplicateLease {
        /// Existing lease identity.
        lease_id: LeaseId,
    },
    /// A task already has an active lease.
    TaskAlreadyLeased {
        /// Conflicting task.
        task_id: TaskId,
        /// Existing lease identity.
        lease_id: LeaseId,
    },
    /// A worker has reached its advertised active lease limit.
    WorkerConcurrencyLimit {
        /// Worker at capacity.
        worker_id: RemoteWorkerId,
        /// Advertised maximum.
        maximum: u16,
    },
    /// A lease identity is not present.
    LeaseNotFound {
        /// Missing lease identity.
        lease_id: LeaseId,
    },
    /// A lease operation was attempted by another worker.
    LeaseOwnerMismatch {
        /// Worker that owns the lease.
        expected: RemoteWorkerId,
        /// Worker presented by the caller.
        found: RemoteWorkerId,
    },
    /// A lease operation used a stale generation.
    LeaseGenerationMismatch {
        /// Current generation.
        expected: u64,
        /// Generation presented by the caller.
        found: u64,
    },
    /// A lease can no longer be renewed or released.
    LeaseExpired {
        /// Expired lease identity.
        lease_id: LeaseId,
    },
    /// A renewal would shorten or preserve the current expiry.
    RenewalNotExtended {
        /// Current expiry timestamp.
        current_expiry_ms: u64,
        /// Requested expiry timestamp.
        requested_expiry_ms: u64,
    },
    /// The task's generation counter cannot advance.
    GenerationExhausted,
}

impl fmt::Display for RemoteWorkerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyField { field } => write!(formatter, "{field} must not be empty"),
            Self::FieldTooLong {
                field,
                length,
                maximum,
            } => write!(formatter, "{field} is {length} bytes; maximum is {maximum}"),
            Self::InvalidCharacter { field, character } => {
                write!(
                    formatter,
                    "{field} contains unsupported character {character:?}"
                )
            }
            Self::UnsupportedProtocolVersion { found } => write!(
                formatter,
                "unsupported remote-worker protocol version {found}; expected {REMOTE_WORKER_PROTOCOL_VERSION}"
            ),
            Self::EmptyCapabilities => formatter.write_str("worker must advertise a capability"),
            Self::TooManyCapabilities { count, maximum } => write!(
                formatter,
                "worker advertises {count} capabilities; maximum is {maximum}"
            ),
            Self::DuplicateCapability { capability } => {
                write!(
                    formatter,
                    "capability is advertised more than once: {capability}"
                )
            }
            Self::InvalidConcurrency { found, maximum } => write!(
                formatter,
                "worker concurrency {found} is outside 1..={maximum}"
            ),
            Self::InvalidLeaseWindow {
                issued_at_ms,
                expires_at_ms,
            } => write!(
                formatter,
                "lease window is invalid: issued at {issued_at_ms}, expires at {expires_at_ms}"
            ),
            Self::LeaseDurationTooLong {
                duration_ms,
                maximum,
            } => write!(
                formatter,
                "lease duration {duration_ms}ms exceeds maximum {maximum}ms"
            ),
            Self::DuplicateLease { lease_id } => {
                write!(formatter, "lease already exists: {lease_id}")
            }
            Self::TaskAlreadyLeased { task_id, lease_id } => write!(
                formatter,
                "task {task_id} already has active lease {lease_id}"
            ),
            Self::WorkerConcurrencyLimit { worker_id, maximum } => write!(
                formatter,
                "worker {worker_id} reached concurrency limit {maximum}"
            ),
            Self::LeaseNotFound { lease_id } => write!(formatter, "lease not found: {lease_id}"),
            Self::LeaseOwnerMismatch { expected, found } => {
                write!(formatter, "lease belongs to worker {expected}, not {found}")
            }
            Self::LeaseGenerationMismatch { expected, found } => {
                write!(formatter, "lease generation is {expected}, not {found}")
            }
            Self::LeaseExpired { lease_id } => write!(formatter, "lease is expired: {lease_id}"),
            Self::RenewalNotExtended {
                current_expiry_ms,
                requested_expiry_ms,
            } => write!(
                formatter,
                "renewal expiry {requested_expiry_ms}ms does not extend {current_expiry_ms}ms"
            ),
            Self::GenerationExhausted => formatter.write_str("task lease generation is exhausted"),
        }
    }
}

impl std::error::Error for RemoteWorkerError {}

#[cfg(test)]
mod tests {
    use super::{
        LeaseBook, LeaseId, LeaseState, MAX_LEASE_DURATION_MS, RemoteWorkerDescriptor,
        RemoteWorkerError, RemoteWorkerId, WorkerCapability,
    };
    use crate::task::TaskId;

    fn worker(
        id: &str,
        capabilities: &[&str],
        max_concurrent_leases: u16,
    ) -> RemoteWorkerDescriptor {
        let worker_id = RemoteWorkerId::parse(id).expect("valid worker ID");
        let capabilities = capabilities
            .iter()
            .map(|capability| WorkerCapability::parse(*capability).expect("valid capability"))
            .collect();
        RemoteWorkerDescriptor::new(
            worker_id,
            "linux-x86_64",
            capabilities,
            max_concurrent_leases,
        )
        .expect("valid worker descriptor")
    }

    fn task(id: &str) -> TaskId {
        TaskId::parse(id).expect("valid task ID")
    }

    fn lease(id: &str) -> LeaseId {
        LeaseId::parse(id).expect("valid lease ID")
    }

    #[test]
    fn descriptors_are_versioned_bounded_and_deterministic() {
        let descriptor = worker("worker-1", &["zeta", "build", "read"], 2);
        let capabilities: Vec<&str> = descriptor
            .capabilities()
            .iter()
            .map(WorkerCapability::as_str)
            .collect();
        assert_eq!(capabilities, ["build", "read", "zeta"]);

        assert_eq!(descriptor.protocol_version(), 1);
        assert_eq!(descriptor.platform(), "linux-x86_64");
    }

    #[test]
    fn invalid_descriptor_values_fail_closed() {
        assert!(matches!(
            RemoteWorkerId::parse(""),
            Err(RemoteWorkerError::EmptyField { field: "worker ID" })
        ));
        assert!(matches!(
            WorkerCapability::parse("RunLocalCommands"),
            Err(RemoteWorkerError::InvalidCharacter {
                field: "capability",
                ..
            })
        ));
        assert!(matches!(
            RemoteWorkerDescriptor::with_protocol_version(
                2,
                RemoteWorkerId::parse("worker-1").expect("valid worker ID"),
                "linux",
                vec![WorkerCapability::parse("execute").expect("valid capability")],
                1,
            ),
            Err(RemoteWorkerError::UnsupportedProtocolVersion { found: 2 })
        ));
        assert!(matches!(
            RemoteWorkerDescriptor::new(
                RemoteWorkerId::parse("worker-1").expect("valid worker ID"),
                "linux",
                vec![WorkerCapability::parse("execute").expect("valid capability")],
                0,
            ),
            Err(RemoteWorkerError::InvalidConcurrency { found: 0, .. })
        ));
    }

    #[test]
    fn grants_are_exclusive_and_generational() {
        let worker = worker("worker-1", &["execute"], 2);
        let mut book = LeaseBook::new();
        let first = book
            .grant(&worker, lease("lease-1"), task("P4-M001-T0001"), 100, 200)
            .expect("first grant");
        assert_eq!(first.generation(), 1);
        assert_eq!(first.state(), LeaseState::Active);

        let conflict = book.grant(&worker, lease("lease-2"), task("P4-M001-T0001"), 150, 250);
        assert!(matches!(
            conflict,
            Err(RemoteWorkerError::TaskAlreadyLeased { .. })
        ));

        let second = book
            .grant(&worker, lease("lease-2"), task("P4-M001-T0002"), 150, 250)
            .expect("second grant");
        assert_eq!(second.generation(), 1);
        assert_eq!(book.active_leases_at(199).len(), 2);
    }

    #[test]
    fn concurrency_limit_is_enforced_per_worker() {
        let worker = worker("worker-1", &["execute"], 1);
        let mut book = LeaseBook::new();
        book.grant(&worker, lease("lease-1"), task("task-1"), 100, 200)
            .expect("first grant");
        assert!(matches!(
            book.grant(&worker, lease("lease-2"), task("task-2"), 100, 200),
            Err(RemoteWorkerError::WorkerConcurrencyLimit { .. })
        ));
    }

    #[test]
    fn renewal_requires_owner_generation_and_extension() {
        let worker_descriptor = worker("worker-1", &["execute"], 1);
        let other_worker = worker("worker-2", &["execute"], 1);
        let mut book = LeaseBook::new();
        book.grant(
            &worker_descriptor,
            lease("lease-1"),
            task("task-1"),
            100,
            200,
        )
        .expect("grant");

        assert!(matches!(
            book.renew(&lease("lease-1"), other_worker.worker_id(), 1, 150, 300,),
            Err(RemoteWorkerError::LeaseOwnerMismatch { .. })
        ));
        assert!(matches!(
            book.renew(
                &lease("lease-1"),
                worker_descriptor.worker_id(),
                2,
                150,
                300,
            ),
            Err(RemoteWorkerError::LeaseGenerationMismatch { .. })
        ));
        assert!(matches!(
            book.renew(
                &lease("lease-1"),
                worker_descriptor.worker_id(),
                1,
                150,
                200,
            ),
            Err(RemoteWorkerError::RenewalNotExtended { .. })
        ));

        let renewed = book
            .renew(
                &lease("lease-1"),
                worker_descriptor.worker_id(),
                1,
                150,
                300,
            )
            .expect("renewal");
        assert_eq!(renewed.expires_at_ms(), 300);
    }

    #[test]
    fn expiry_release_and_reclaim_are_explicit_and_deterministic() {
        let worker_descriptor = worker("worker-1", &["execute"], 1);
        let mut book = LeaseBook::new();
        book.grant(
            &worker_descriptor,
            lease("lease-1"),
            task("task-1"),
            100,
            200,
        )
        .expect("grant");

        assert_eq!(book.expire_due(200), vec![lease("lease-1")]);
        assert_eq!(
            book.get(&lease("lease-1")).expect("lease").state(),
            LeaseState::Expired
        );
        assert!(matches!(
            book.renew(
                &lease("lease-1"),
                worker_descriptor.worker_id(),
                1,
                200,
                300,
            ),
            Err(RemoteWorkerError::LeaseExpired { .. })
        ));

        let replacement = book
            .grant(
                &worker_descriptor,
                lease("lease-2"),
                task("task-1"),
                201,
                301,
            )
            .expect("replacement grant");
        assert_eq!(replacement.generation(), 2);
        book.release(&lease("lease-2"), worker_descriptor.worker_id(), 2, 250)
            .expect("release");
        assert_eq!(
            book.get(&lease("lease-2")).expect("lease").state(),
            LeaseState::Released
        );
    }

    #[test]
    fn lease_windows_are_bounded() {
        let worker = worker("worker-1", &["execute"], 1);
        let mut book = LeaseBook::new();
        assert!(matches!(
            book.grant(
                &worker,
                lease("lease-1"),
                task("task-1"),
                100,
                100 + MAX_LEASE_DURATION_MS + 1,
            ),
            Err(RemoteWorkerError::LeaseDurationTooLong { .. })
        ));
    }
}
