//! Coordinated appends across handles and threads (P0-M013, finding 9).

use agentforge_audit::{AuditEvent, AuditEventKind, AuditStore, FileAuditStore};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn path() -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "agentforge-audit-concurrent-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir.join("audit.log")
}

fn event(sequence: u64, label: &str) -> AuditEvent {
    AuditEvent::new(
        sequence,
        format!("{label}-{sequence}"),
        AuditEventKind::TaskTransition,
        "test",
        1,
    )
}

fn next(store: &FileAuditStore) -> u64 {
    store
        .records()
        .last()
        .map_or(1, |record| record.event().sequence() + 1)
}

#[test]
fn a_stale_handle_no_longer_corrupts_the_log() {
    let path = path();
    let mut first = FileAuditStore::open(&path).unwrap();
    let mut second = FileAuditStore::open(&path).unwrap();
    first.append(event(next(&first), "first")).unwrap();
    // `second` still believes the log is empty.
    let appended = second.append(event(next(&second), "second")).unwrap();
    assert_eq!(
        appended.event().sequence(),
        2,
        "renumbered after the other writer"
    );
    assert_eq!(
        appended.event().event_id(),
        "second-2",
        "ID suffix follows the sequence"
    );
    second.append(event(next(&second), "second")).unwrap();

    let reopened = FileAuditStore::open(&path).expect("chain verifies");
    let ids = reopened
        .records()
        .iter()
        .map(|record| record.event().event_id().to_owned())
        .collect::<Vec<_>>();
    assert_eq!(ids, ["first-1", "second-2", "second-3"]);
    fs::remove_dir_all(path.parent().unwrap()).unwrap();
}

#[test]
fn unrelated_sequences_are_still_refused() {
    let path = path();
    let mut store = FileAuditStore::open(&path).unwrap();
    assert!(store.append(event(5, "gap")).is_err());
    store.append(event(1, "ok")).unwrap();
    assert!(store.append(event(1, "duplicate")).is_err());
    assert_eq!(FileAuditStore::open(&path).unwrap().records().len(), 1);
    fs::remove_dir_all(path.parent().unwrap()).unwrap();
}

#[test]
fn stale_batches_are_renumbered_as_a_block() {
    let path = path();
    let mut writer = FileAuditStore::open(&path).unwrap();
    let mut executor = FileAuditStore::open(&path).unwrap();
    let mut attempt = executor.new_attempt_log();
    attempt.append(event(1, "agent-started")).unwrap();
    attempt.append(event(2, "agent-finished")).unwrap();
    // Another writer appends while the "execution" runs.
    writer.append(event(1, "approval")).unwrap();
    let batch = attempt
        .records()
        .iter()
        .map(|record| record.event().clone())
        .collect::<Vec<_>>();
    executor.append_batch(batch).unwrap();
    let ids = FileAuditStore::open(&path)
        .unwrap()
        .records()
        .iter()
        .map(|record| record.event().event_id().to_owned())
        .collect::<Vec<_>>();
    assert_eq!(ids, ["approval-1", "agent-started-2", "agent-finished-3"]);

    let gap = vec![event(4, "a"), event(6, "b")];
    assert!(executor.append_batch(gap).is_err());
    assert_eq!(FileAuditStore::open(&path).unwrap().records().len(), 3);
    fs::remove_dir_all(path.parent().unwrap()).unwrap();
}

#[test]
fn a_held_append_lock_fails_clearly_and_writes_nothing() {
    let path = path();
    let mut store = FileAuditStore::open(&path).unwrap();
    let lock = path.with_file_name("audit.log.lock");
    fs::write(&lock, "pid=1\n").unwrap();
    let error = store.append(event(1, "blocked")).unwrap_err().to_string();
    assert!(error.contains("audit.log.lock"), "{error}");
    assert!(FileAuditStore::open(&path).unwrap().records().is_empty());
    fs::remove_file(&lock).unwrap();
    store.append(event(1, "unblocked")).unwrap();
    fs::remove_dir_all(path.parent().unwrap()).unwrap();
}

#[test]
fn a_log_truncated_underneath_a_handle_fails_closed() {
    let path = path();
    let mut store = FileAuditStore::open(&path).unwrap();
    store.append(event(1, "a")).unwrap();
    store.append(event(2, "b")).unwrap();
    let bytes = fs::read(&path).unwrap();
    fs::write(&path, &bytes[..bytes.len() - 10]).unwrap();
    assert!(store.append(event(3, "c")).is_err());
    fs::remove_dir_all(path.parent().unwrap()).unwrap();
}

#[test]
fn concurrent_writers_keep_one_verified_chain() {
    let path = path();
    FileAuditStore::open(&path).unwrap();
    let threads = (0..4)
        .map(|thread| {
            let path = path.clone();
            std::thread::spawn(move || {
                let mut store = FileAuditStore::open(&path).unwrap();
                for _ in 0..25 {
                    let sequence = next(&store);
                    store
                        .append(event(sequence, &format!("thread{thread}")))
                        .unwrap();
                }
            })
        })
        .collect::<Vec<_>>();
    for thread in threads {
        thread.join().unwrap();
    }
    let reopened = FileAuditStore::open(&path).expect("chain verifies");
    assert_eq!(reopened.records().len(), 100);
    for (index, record) in reopened.records().iter().enumerate() {
        assert_eq!(record.event().sequence(), index as u64 + 1);
    }
    fs::remove_dir_all(path.parent().unwrap()).unwrap();
}
