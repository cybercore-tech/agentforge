//! Append-only, locally durable orchestration evidence.

use std::collections::BTreeMap;
use std::fmt;
use std::fs::{File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

const MAGIC: &[u8; 5] = b"AFAL\0";
const FORMAT_VERSION: u16 = 1;
const MAX_FIELD: usize = 4096;
const MAX_PAYLOAD_FIELDS: usize = 128;
const MAX_RECORD: usize = 1024 * 1024;
const DIGEST_SIZE: usize = 32;

/// A stable kind of orchestration evidence.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum AuditEventKind {
    /// A task was created.
    TaskCreated,
    /// A human approval was recorded.
    ApprovalRecorded,
    /// A worktree was observed.
    WorktreeObserved,
    /// An agent process started.
    AgentStarted,
    /// An agent process finished.
    AgentFinished,
    /// A local gate finished.
    GateFinished,
    /// A CI observation was recorded.
    CiObserved,
    /// A failure classification was recorded.
    FailureClassified,
    /// A review handoff was recorded.
    ReviewHandoff,
    /// A task lifecycle transition was recorded.
    TaskTransition,
}

impl AuditEventKind {
    fn code(self) -> u8 {
        match self {
            Self::TaskCreated => 1,
            Self::ApprovalRecorded => 2,
            Self::WorktreeObserved => 3,
            Self::AgentStarted => 4,
            Self::AgentFinished => 5,
            Self::GateFinished => 6,
            Self::CiObserved => 7,
            Self::FailureClassified => 8,
            Self::ReviewHandoff => 9,
            Self::TaskTransition => 10,
        }
    }
    fn from_code(code: u8) -> Result<Self, AuditError> {
        Ok(match code {
            1 => Self::TaskCreated,
            2 => Self::ApprovalRecorded,
            3 => Self::WorktreeObserved,
            4 => Self::AgentStarted,
            5 => Self::AgentFinished,
            6 => Self::GateFinished,
            7 => Self::CiObserved,
            8 => Self::FailureClassified,
            9 => Self::ReviewHandoff,
            10 => Self::TaskTransition,
            _ => return Err(AuditError::UnknownEventKind(code)),
        })
    }
}

/// One caller-supplied immutable audit event.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuditEvent {
    sequence: u64,
    event_id: String,
    kind: AuditEventKind,
    actor: String,
    timestamp: u64,
    task_id: Option<String>,
    fields: BTreeMap<String, String>,
}

impl AuditEvent {
    /// Creates an event. Sequence and timestamp are explicit caller metadata.
    #[must_use]
    pub fn new(
        sequence: u64,
        event_id: impl Into<String>,
        kind: AuditEventKind,
        actor: impl Into<String>,
        timestamp: u64,
    ) -> Self {
        Self {
            sequence,
            event_id: event_id.into(),
            kind,
            actor: actor.into(),
            timestamp,
            task_id: None,
            fields: BTreeMap::new(),
        }
    }
    /// Associates the event with a task.
    #[must_use]
    pub fn with_task_id(mut self, task_id: impl Into<String>) -> Self {
        self.task_id = Some(task_id.into());
        self
    }
    /// Adds one deterministic key/value field.
    #[must_use]
    pub fn with_field(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.fields.insert(key.into(), value.into());
        self
    }
    /// Returns the sequence number.
    #[must_use]
    pub const fn sequence(&self) -> u64 {
        self.sequence
    }
    /// Returns the event ID.
    #[must_use]
    pub fn event_id(&self) -> &str {
        &self.event_id
    }
    /// Returns the kind.
    #[must_use]
    pub const fn kind(&self) -> AuditEventKind {
        self.kind
    }
    /// Returns the actor identity.
    #[must_use]
    pub fn actor(&self) -> &str {
        &self.actor
    }
    /// Returns the injected timestamp metadata.
    #[must_use]
    pub const fn timestamp(&self) -> u64 {
        self.timestamp
    }
    /// Returns the optional task ID.
    #[must_use]
    pub fn task_id(&self) -> Option<&str> {
        self.task_id.as_deref()
    }
    /// Returns canonical fields in key order.
    #[must_use]
    pub fn fields(&self) -> &BTreeMap<String, String> {
        &self.fields
    }
}

/// A verified event and its integrity-chain digests.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuditRecord {
    event: AuditEvent,
    previous_digest: [u8; DIGEST_SIZE],
    digest: [u8; DIGEST_SIZE],
}
impl AuditRecord {
    /// Returns the event.
    #[must_use]
    pub fn event(&self) -> &AuditEvent {
        &self.event
    }
    /// Returns the previous chain digest.
    #[must_use]
    pub fn previous_digest(&self) -> &[u8; DIGEST_SIZE] {
        &self.previous_digest
    }
    /// Returns this record digest.
    #[must_use]
    pub fn digest(&self) -> &[u8; DIGEST_SIZE] {
        &self.digest
    }
}

/// In-memory append-only audit log.
#[derive(Clone, Debug)]
pub struct AuditLog {
    records: Vec<AuditRecord>,
    next_sequence: u64,
    previous_digest: [u8; DIGEST_SIZE],
}
impl Default for AuditLog {
    fn default() -> Self {
        Self::new()
    }
}
impl AuditLog {
    /// Creates an empty log.
    #[must_use]
    pub fn new() -> Self {
        Self::with_origin(1, [0; DIGEST_SIZE])
    }
    /// Creates an empty attempt log positioned after an existing verified tail.
    #[must_use]
    pub fn with_origin(next_sequence: u64, previous_digest: [u8; DIGEST_SIZE]) -> Self {
        Self {
            records: Vec::new(),
            next_sequence,
            previous_digest,
        }
    }
    /// Returns the next sequence number accepted by this log.
    #[must_use]
    pub const fn next_sequence(&self) -> u64 {
        self.next_sequence
    }
    /// Appends and validates one event.
    pub fn append(&mut self, event: AuditEvent) -> Result<&AuditRecord, AuditError> {
        validate_event(&event)?;
        if event.sequence != self.next_sequence {
            return Err(AuditError::Sequence {
                expected: self.next_sequence,
                actual: event.sequence,
            });
        }
        let previous = self.previous_digest;
        let sequence = event.sequence;
        let body = encode_event(&event)?;
        let mut input = body.clone();
        input.extend_from_slice(&previous);
        let digest = digest(&input);
        self.records.push(AuditRecord {
            event,
            previous_digest: previous,
            digest,
        });
        self.next_sequence = sequence.saturating_add(1);
        self.previous_digest = digest;
        Ok(self.records.last().expect("record was just pushed"))
    }
    /// Returns records in sequence order.
    #[must_use]
    pub fn records(&self) -> &[AuditRecord] {
        &self.records
    }
    /// Returns records matching a task ID.
    #[must_use]
    pub fn for_task(&self, task_id: &str) -> Vec<&AuditRecord> {
        self.records
            .iter()
            .filter(|r| r.event.task_id() == Some(task_id))
            .collect()
    }
    /// Returns records matching a kind.
    #[must_use]
    pub fn of_kind(&self, kind: AuditEventKind) -> Vec<&AuditRecord> {
        self.records
            .iter()
            .filter(|r| r.event.kind == kind)
            .collect()
    }
}

/// Trait boundary for durable audit stores.
pub trait AuditStore {
    /// Appends and durably synchronizes an event.
    fn append(&mut self, event: AuditEvent) -> Result<&AuditRecord, AuditError>;
    /// Returns all verified records.
    fn records(&self) -> &[AuditRecord];
}

/// A local append-only audit file.
#[derive(Debug)]
pub struct FileAuditStore {
    path: PathBuf,
    log: AuditLog,
}
impl FileAuditStore {
    /// Opens or creates a versioned audit file and verifies all existing records.
    pub fn open(path: impl Into<PathBuf>) -> Result<Self, AuditError> {
        let path = path.into();
        if !path.exists() {
            let mut file = OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&path)
                .map_err(AuditError::Io)?;
            file.write_all(MAGIC).map_err(AuditError::Io)?;
            file.write_all(&FORMAT_VERSION.to_le_bytes())
                .map_err(AuditError::Io)?;
            file.sync_all().map_err(AuditError::Io)?;
        }
        let mut bytes = Vec::new();
        File::open(&path)
            .map_err(AuditError::Io)?
            .read_to_end(&mut bytes)
            .map_err(AuditError::Io)?;
        let log = decode_log(&bytes)?;
        Ok(Self { path, log })
    }
    /// Returns the backing path.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }
    /// Returns records matching a task ID.
    #[must_use]
    pub fn for_task(&self, task_id: &str) -> Vec<&AuditRecord> {
        self.log.for_task(task_id)
    }
    /// Returns records matching a kind.
    #[must_use]
    pub fn of_kind(&self, kind: AuditEventKind) -> Vec<&AuditRecord> {
        self.log.of_kind(kind)
    }
    /// Creates an in-memory attempt log positioned after the verified file tail.
    #[must_use]
    pub fn new_attempt_log(&self) -> AuditLog {
        self.log
            .records
            .last()
            .map_or_else(AuditLog::new, |record| {
                AuditLog::with_origin(record.event.sequence.saturating_add(1), record.digest)
            })
    }
}
impl AuditStore for FileAuditStore {
    fn append(&mut self, event: AuditEvent) -> Result<&AuditRecord, AuditError> {
        validate_event(&event)?;
        let expected = self.log.records.last().map_or(1, |r| r.event.sequence + 1);
        if event.sequence != expected {
            return Err(AuditError::Sequence {
                expected,
                actual: event.sequence,
            });
        }
        let previous = self
            .log
            .records
            .last()
            .map_or([0; DIGEST_SIZE], |r| r.digest);
        let body = encode_event(&event)?;
        let mut input = body.clone();
        input.extend_from_slice(&previous);
        let current = digest(&input);
        let mut frame = body;
        frame.extend_from_slice(&previous);
        frame.extend_from_slice(&current);
        if frame.len() > MAX_RECORD {
            return Err(AuditError::Limit("record"));
        }
        let mut file = OpenOptions::new()
            .append(true)
            .open(&self.path)
            .map_err(AuditError::Io)?;
        let length = u32::try_from(frame.len()).map_err(|_| AuditError::Limit("record"))?;
        file.write_all(&length.to_le_bytes())
            .map_err(AuditError::Io)?;
        file.write_all(&frame).map_err(AuditError::Io)?;
        file.sync_all().map_err(AuditError::Io)?;
        self.log.records.push(AuditRecord {
            event,
            previous_digest: previous,
            digest: current,
        });
        Ok(self.log.records.last().expect("record was just pushed"))
    }
    fn records(&self) -> &[AuditRecord] {
        self.log.records()
    }
}

/// Audit validation, framing, and storage failure.
#[derive(Debug)]
pub enum AuditError {
    /// Filesystem I/O failed.
    Io(io::Error),
    /// A field or record exceeded a documented limit.
    Limit(&'static str),
    /// An identifier or payload field was invalid.
    InvalidField(&'static str),
    /// Sequence was not contiguous.
    Sequence {
        /// The next permitted sequence.
        expected: u64,
        /// The sequence supplied by the caller or file.
        actual: u64,
    },
    /// File header or framing was invalid.
    InvalidFormat(&'static str),
    /// A future or unknown format version was encountered.
    UnsupportedVersion(u16),
    /// A future event kind was encountered.
    UnknownEventKind(u8),
    /// A digest or previous-link verification failed.
    IntegrityMismatch,
}
impl fmt::Display for AuditError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "audit I/O error: {e}"),
            Self::Limit(s) => write!(f, "audit {s} exceeds its limit"),
            Self::InvalidField(s) => write!(f, "invalid audit field: {s}"),
            Self::Sequence { expected, actual } => {
                write!(f, "audit sequence expected {expected}, got {actual}")
            }
            Self::InvalidFormat(s) => write!(f, "invalid audit format: {s}"),
            Self::UnsupportedVersion(v) => write!(f, "unsupported audit version: {v}"),
            Self::UnknownEventKind(k) => write!(f, "unknown audit event kind: {k}"),
            Self::IntegrityMismatch => f.write_str("audit integrity chain mismatch"),
        }
    }
}
impl std::error::Error for AuditError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        if let Self::Io(e) = self {
            Some(e)
        } else {
            None
        }
    }
}

fn validate_event(event: &AuditEvent) -> Result<(), AuditError> {
    if event.sequence == 0 {
        return Err(AuditError::InvalidField("sequence"));
    }
    valid_text(&event.event_id, "event ID")?;
    valid_text(&event.actor, "actor")?;
    if let Some(task) = &event.task_id {
        valid_text(task, "task ID")?;
    }
    if event.fields.len() > MAX_PAYLOAD_FIELDS {
        return Err(AuditError::Limit("fields"));
    }
    for (k, v) in &event.fields {
        valid_text(k, "field key")?;
        valid_text(v, "field value")?;
    }
    Ok(())
}
fn valid_text(value: &str, name: &'static str) -> Result<(), AuditError> {
    if value.is_empty() || value.len() > MAX_FIELD || value.chars().any(char::is_control) {
        return Err(AuditError::InvalidField(name));
    }
    Ok(())
}
fn put_u32(out: &mut Vec<u8>, n: usize) -> Result<(), AuditError> {
    out.extend_from_slice(
        &u32::try_from(n)
            .map_err(|_| AuditError::Limit("field"))?
            .to_le_bytes(),
    );
    Ok(())
}
fn put_text(out: &mut Vec<u8>, value: &str) -> Result<(), AuditError> {
    put_u32(out, value.len())?;
    out.extend_from_slice(value.as_bytes());
    Ok(())
}
fn get_u32(bytes: &[u8], pos: &mut usize) -> Result<usize, AuditError> {
    if bytes.len().saturating_sub(*pos) < 4 {
        return Err(AuditError::InvalidFormat("truncated length"));
    }
    let n = u32::from_le_bytes(bytes[*pos..*pos + 4].try_into().unwrap()) as usize;
    *pos += 4;
    if n > MAX_FIELD {
        return Err(AuditError::Limit("field"));
    }
    Ok(n)
}
fn get_text(bytes: &[u8], pos: &mut usize) -> Result<String, AuditError> {
    let n = get_u32(bytes, pos)?;
    if bytes.len().saturating_sub(*pos) < n {
        return Err(AuditError::InvalidFormat("truncated text"));
    }
    let s = std::str::from_utf8(&bytes[*pos..*pos + n])
        .map_err(|_| AuditError::InvalidFormat("invalid UTF-8"))?
        .to_owned();
    *pos += n;
    if s.is_empty() || s.chars().any(char::is_control) {
        return Err(AuditError::InvalidField("text"));
    }
    Ok(s)
}
fn encode_event(event: &AuditEvent) -> Result<Vec<u8>, AuditError> {
    let mut out = Vec::new();
    out.extend_from_slice(&FORMAT_VERSION.to_le_bytes());
    out.extend_from_slice(&event.sequence.to_le_bytes());
    out.extend_from_slice(&event.timestamp.to_le_bytes());
    out.push(event.kind.code());
    put_text(&mut out, &event.event_id)?;
    put_text(&mut out, &event.actor)?;
    match &event.task_id {
        Some(t) => {
            out.push(1);
            put_text(&mut out, t)?;
        }
        None => out.push(0),
    }
    out.extend_from_slice(&(event.fields.len() as u16).to_le_bytes());
    for (k, v) in &event.fields {
        put_text(&mut out, k)?;
        put_text(&mut out, v)?;
    }
    if out.len() > MAX_RECORD {
        return Err(AuditError::Limit("record"));
    }
    Ok(out)
}
fn decode_event(bytes: &[u8]) -> Result<AuditEvent, AuditError> {
    let mut p = 0;
    if bytes.len() < 2 + 8 + 8 + 1 {
        return Err(AuditError::InvalidFormat("truncated event"));
    }
    let version = u16::from_le_bytes(bytes[0..2].try_into().unwrap());
    p += 2;
    if version != FORMAT_VERSION {
        return Err(AuditError::UnsupportedVersion(version));
    }
    let sequence = u64::from_le_bytes(bytes[p..p + 8].try_into().unwrap());
    p += 8;
    let timestamp = u64::from_le_bytes(bytes[p..p + 8].try_into().unwrap());
    p += 8;
    let kind = AuditEventKind::from_code(bytes[p])?;
    p += 1;
    let event_id = get_text(bytes, &mut p)?;
    let actor = get_text(bytes, &mut p)?;
    if p >= bytes.len() {
        return Err(AuditError::InvalidFormat("truncated task marker"));
    }
    let task_id = match bytes[p] {
        0 => {
            p += 1;
            None
        }
        1 => {
            p += 1;
            Some(get_text(bytes, &mut p)?)
        }
        _ => return Err(AuditError::InvalidFormat("invalid task marker")),
    };
    if bytes.len().saturating_sub(p) < 2 {
        return Err(AuditError::InvalidFormat("truncated field count"));
    }
    let count = u16::from_le_bytes(bytes[p..p + 2].try_into().unwrap()) as usize;
    p += 2;
    if count > MAX_PAYLOAD_FIELDS {
        return Err(AuditError::Limit("fields"));
    }
    let mut fields = BTreeMap::new();
    for _ in 0..count {
        let k = get_text(bytes, &mut p)?;
        let v = get_text(bytes, &mut p)?;
        if fields.insert(k, v).is_some() {
            return Err(AuditError::InvalidFormat("duplicate field"));
        }
    }
    if p != bytes.len() {
        return Err(AuditError::InvalidFormat("trailing event bytes"));
    }
    let event = AuditEvent {
        sequence,
        event_id,
        kind,
        actor,
        timestamp,
        task_id,
        fields,
    };
    validate_event(&event)?;
    Ok(event)
}
fn decode_log(bytes: &[u8]) -> Result<AuditLog, AuditError> {
    if bytes.len() < MAGIC.len() + 2 || &bytes[..MAGIC.len()] != MAGIC {
        return Err(AuditError::InvalidFormat("header"));
    }
    let version = u16::from_le_bytes(bytes[5..7].try_into().unwrap());
    if version != FORMAT_VERSION {
        return Err(AuditError::UnsupportedVersion(version));
    }
    let mut p = 7;
    let mut log = AuditLog::new();
    while p < bytes.len() {
        if bytes.len().saturating_sub(p) < 4 {
            return Err(AuditError::InvalidFormat("truncated frame length"));
        }
        let n = u32::from_le_bytes(bytes[p..p + 4].try_into().unwrap()) as usize;
        p += 4;
        if n == 0 || n > MAX_RECORD || bytes.len().saturating_sub(p) < n {
            return Err(AuditError::InvalidFormat("truncated frame"));
        }
        if n < DIGEST_SIZE * 2 {
            return Err(AuditError::InvalidFormat("short frame"));
        }
        let frame = &bytes[p..p + n];
        p += n;
        let event = decode_event(&frame[..n - DIGEST_SIZE * 2])?;
        let previous: [u8; DIGEST_SIZE] = frame[n - DIGEST_SIZE * 2..n - DIGEST_SIZE]
            .try_into()
            .unwrap();
        let current: [u8; DIGEST_SIZE] = frame[n - DIGEST_SIZE..].try_into().unwrap();
        let expected_prev = log.records.last().map_or([0; DIGEST_SIZE], |r| r.digest);
        if previous != expected_prev {
            return Err(AuditError::IntegrityMismatch);
        }
        let mut input = frame[..n - DIGEST_SIZE].to_vec();
        let expected = digest(&input);
        input.fill(0);
        if current != expected {
            return Err(AuditError::IntegrityMismatch);
        }
        log.append_decoded(event, previous, current)?;
    }
    Ok(log)
}
impl AuditLog {
    fn append_decoded(
        &mut self,
        event: AuditEvent,
        previous: [u8; DIGEST_SIZE],
        current: [u8; DIGEST_SIZE],
    ) -> Result<(), AuditError> {
        if event.sequence != self.next_sequence {
            return Err(AuditError::Sequence {
                expected: self.next_sequence,
                actual: event.sequence,
            });
        }
        self.records.push(AuditRecord {
            event,
            previous_digest: previous,
            digest: current,
        });
        self.next_sequence = self
            .records
            .last()
            .expect("record was just pushed")
            .event
            .sequence
            .saturating_add(1);
        self.previous_digest = current;
        Ok(())
    }
}

fn digest(bytes: &[u8]) -> [u8; DIGEST_SIZE] {
    let seeds = [
        0xcbf29ce484222325_u64,
        0x84222325cbf29ce4_u64,
        0x9e3779b185ebca87_u64,
        0x517cc1b727220a95_u64,
    ];
    let mut out = [0; DIGEST_SIZE];
    for (i, seed) in seeds.into_iter().enumerate() {
        let mut h = seed;
        for &b in bytes {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x100000001b3);
        }
        out[i * 8..i * 8 + 8].copy_from_slice(&h.to_le_bytes());
    }
    out
}
