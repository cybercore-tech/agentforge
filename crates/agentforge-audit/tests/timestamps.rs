//! Wall-clock audit timestamps (P0-M015, dogfooding finding 12).

use agentforge_audit::{
    AuditEvent, AuditEventKind, AuditStore, FileAuditStore, UNSET_TIMESTAMP_BELOW, wall_clock_ms,
};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn path() -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "agentforge-audit-time-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir.join("audit.log")
}

#[test]
fn now_stamps_the_current_time() {
    let before = wall_clock_ms();
    let event = AuditEvent::now(1, "a-1", AuditEventKind::TaskTransition, "test");
    let after = wall_clock_ms();
    assert!((before..=after).contains(&event.timestamp()));
    assert!(event.has_timestamp());
    assert!(!AuditEvent::new(1, "a-1", AuditEventKind::TaskTransition, "test", 1).has_timestamp());
    assert!(UNSET_TIMESTAMP_BELOW < before);
}

#[test]
fn the_store_stamps_unset_events_and_keeps_real_times() {
    let path = path();
    let mut store = FileAuditStore::open(&path).unwrap();
    let explicit = 1_790_000_000_000;
    let before = wall_clock_ms();
    store
        .append(AuditEvent::new(
            1,
            "legacy-1",
            AuditEventKind::TaskTransition,
            "test",
            1,
        ))
        .unwrap();
    let after = wall_clock_ms();
    store
        .append(AuditEvent::new(
            2,
            "explicit-2",
            AuditEventKind::TaskTransition,
            "test",
            explicit,
        ))
        .unwrap();
    let reopened = FileAuditStore::open(&path).expect("chain verifies");
    let stamped = reopened.records()[0].event().timestamp();
    assert!(
        (before..=after).contains(&stamped),
        "backstop stamped {stamped}"
    );
    assert_eq!(reopened.records()[1].event().timestamp(), explicit);
    fs::remove_dir_all(path.parent().unwrap()).unwrap();
}

#[test]
fn a_batch_keeps_each_event_s_creation_time() {
    let path = path();
    let mut store = FileAuditStore::open(&path).unwrap();
    let mut attempt = store.new_attempt_log();
    attempt
        .append(AuditEvent::new(
            1,
            "started-1",
            AuditEventKind::AgentStarted,
            "test",
            1_790_000_000_000,
        ))
        .unwrap();
    attempt
        .append(AuditEvent::new(
            2,
            "finished-2",
            AuditEventKind::AgentFinished,
            "test",
            1_790_000_434_000,
        ))
        .unwrap();
    let events = attempt
        .records()
        .iter()
        .map(|record| record.event().clone())
        .collect::<Vec<_>>();
    store.append_batch(events).unwrap();
    let times = FileAuditStore::open(&path)
        .unwrap()
        .records()
        .iter()
        .map(|record| record.event().timestamp())
        .collect::<Vec<_>>();
    assert_eq!(times, [1_790_000_000_000, 1_790_000_434_000]);
    fs::remove_dir_all(path.parent().unwrap()).unwrap();
}

/// Guard (P0-M015): production code creates events with `AuditEvent::now`, so each event carries
/// its creation time. `AuditEvent::new` (explicit time) stays available to tests only.
#[test]
fn production_code_never_uses_explicit_placeholder_timestamps() {
    let crates = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let mut offenders = Vec::new();
    for entry in fs::read_dir(&crates).unwrap() {
        let src = entry.unwrap().path().join("src");
        if !src.is_dir() || src.starts_with(crates.join("agentforge-audit")) {
            continue;
        }
        let mut stack = vec![src];
        while let Some(directory) = stack.pop() {
            for file in fs::read_dir(&directory).unwrap() {
                let path = file.unwrap().path();
                if path.is_dir() {
                    stack.push(path);
                } else if path.extension().is_some_and(|extension| extension == "rs") {
                    let text = fs::read_to_string(&path).unwrap();
                    let production = text.split("#[cfg(test)]").next().unwrap_or_default();
                    if production.contains("AuditEvent::new(") {
                        offenders.push(path.display().to_string());
                    }
                }
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "use AuditEvent::now in production code: {offenders:?}"
    );
}
