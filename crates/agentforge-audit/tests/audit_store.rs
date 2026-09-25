//! Integration coverage for durable append-only audit evidence.

use agentforge_audit::{
    AuditError, AuditEvent, AuditEventKind, AuditLog, AuditStore, FileAuditStore,
};
use std::fs;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);

fn path() -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!(
        "agentforge-audit-test-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = fs::remove_file(&path);
    path
}

fn event(sequence: u64, kind: AuditEventKind) -> AuditEvent {
    AuditEvent::new(
        sequence,
        format!("event-{sequence}"),
        kind,
        "operator",
        100 + sequence,
    )
    .with_task_id("P0-M009-T0001")
    .with_field("status", "observed")
}

#[test]
fn memory_log_is_deterministic_and_queryable() {
    let mut log = AuditLog::new();
    log.append(event(1, AuditEventKind::TaskCreated)).unwrap();
    log.append(event(2, AuditEventKind::GateFinished)).unwrap();
    assert_eq!(log.records().len(), 2);
    assert_eq!(log.for_task("P0-M009-T0001").len(), 2);
    assert_eq!(log.of_kind(AuditEventKind::GateFinished).len(), 1);
    assert!(matches!(
        log.append(event(4, AuditEventKind::CiObserved)),
        Err(AuditError::Sequence {
            expected: 3,
            actual: 4
        })
    ));
}

#[test]
fn attempt_log_continues_sequence_and_integrity_tail() {
    let mut history = AuditLog::new();
    history
        .append(event(1, AuditEventKind::TaskCreated))
        .unwrap();
    let tail = *history.records()[0].digest();

    let mut attempt = AuditLog::with_origin(2, tail);
    attempt
        .append(event(2, AuditEventKind::AgentStarted))
        .unwrap();

    assert_eq!(attempt.records()[0].event().sequence(), 2);
    assert_eq!(*attempt.records()[0].previous_digest(), tail);
}

#[test]
fn file_round_trip_preserves_chain_and_fields() {
    let path = path();
    let mut store = FileAuditStore::open(&path).unwrap();
    store.append(event(1, AuditEventKind::TaskCreated)).unwrap();
    store
        .append(event(2, AuditEventKind::TaskTransition))
        .unwrap();
    let first_digest = *store.records()[0].digest();
    drop(store);
    let store = FileAuditStore::open(&path).unwrap();
    assert_eq!(store.records().len(), 2);
    assert_eq!(*store.records()[1].previous_digest(), first_digest);
    assert_eq!(store.records()[1].event().fields()["status"], "observed");
    fs::remove_file(path).unwrap();
}

#[test]
fn lease_event_kind_round_trips_as_versioned_evidence() {
    let path = path();
    let mut store = FileAuditStore::open(&path).unwrap();
    store
        .append(
            event(1, AuditEventKind::LeaseRecorded)
                .with_field("action", "granted")
                .with_field("lease_id", "P4-M004-T0001.L1"),
        )
        .unwrap();
    drop(store);
    let store = FileAuditStore::open(&path).unwrap();
    let recorded = store.records()[0].event();
    assert_eq!(recorded.kind(), AuditEventKind::LeaseRecorded);
    assert_eq!(recorded.fields()["action"], "granted");
    fs::remove_file(path).unwrap();
}

#[test]
fn tool_event_kind_round_trips_as_versioned_evidence() {
    let path = path();
    let mut store = FileAuditStore::open(&path).unwrap();
    store
        .append(
            event(1, AuditEventKind::ToolInvoked)
                .with_field("tool", "check_changes")
                .with_field("decision", "allowed"),
        )
        .unwrap();
    drop(store);
    let store = FileAuditStore::open(&path).unwrap();
    let recorded = store.records()[0].event();
    assert_eq!(recorded.kind(), AuditEventKind::ToolInvoked);
    assert_eq!(recorded.fields()["tool"], "check_changes");
    fs::remove_file(path).unwrap();
}

#[test]
fn integration_event_kind_round_trips_as_versioned_evidence() {
    let path = path();
    let mut store = FileAuditStore::open(&path).unwrap();
    store
        .append(event(1, AuditEventKind::IntegrationRecorded))
        .unwrap();
    drop(store);
    let store = FileAuditStore::open(&path).unwrap();
    assert_eq!(
        store.records()[0].event().kind(),
        AuditEventKind::IntegrationRecorded
    );
    fs::remove_file(path).unwrap();
}

#[test]
fn mutation_truncation_and_trailing_bytes_fail_closed() {
    let path = path();
    let mut store = FileAuditStore::open(&path).unwrap();
    store.append(event(1, AuditEventKind::CiObserved)).unwrap();
    drop(store);
    let original = fs::read(&path).unwrap();

    let mut mutated = original.clone();
    *mutated.last_mut().unwrap() ^= 1;
    fs::write(&path, mutated).unwrap();
    assert!(matches!(
        FileAuditStore::open(&path),
        Err(AuditError::IntegrityMismatch)
    ));

    fs::write(&path, &original[..original.len() - 1]).unwrap();
    assert!(matches!(
        FileAuditStore::open(&path),
        Err(AuditError::InvalidFormat(_))
    ));

    let mut trailing = original;
    trailing.push(0);
    fs::write(&path, trailing).unwrap();
    assert!(matches!(
        FileAuditStore::open(&path),
        Err(AuditError::InvalidFormat(_))
    ));
    fs::remove_file(path).unwrap();
}

#[test]
fn validation_and_compatibility_errors_are_explicit() {
    let mut log = AuditLog::new();
    assert!(matches!(
        log.append(AuditEvent::new(
            1,
            "",
            AuditEventKind::TaskCreated,
            "operator",
            1
        )),
        Err(AuditError::InvalidField("event ID"))
    ));
    assert!(matches!(
        log.append(AuditEvent::new(
            0,
            "id",
            AuditEventKind::TaskCreated,
            "operator",
            1
        )),
        Err(AuditError::InvalidField("sequence"))
    ));
    let path = path();
    fs::write(&path, b"AFAL\0\x02\0").unwrap();
    assert!(matches!(
        FileAuditStore::open(&path),
        Err(AuditError::UnsupportedVersion(2))
    ));
    fs::remove_file(path).unwrap();
}

#[test]
fn oversized_fields_do_not_allocate_unbounded_data() {
    let mut log = AuditLog::new();
    let huge = "x".repeat(5000);
    assert!(matches!(
        log.append(event(1, AuditEventKind::AgentFinished).with_field("output", huge)),
        Err(AuditError::InvalidField("field value"))
    ));
}
