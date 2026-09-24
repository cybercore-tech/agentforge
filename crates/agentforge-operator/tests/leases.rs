//! Coverage for operator lease operations (P4-M004).

use agentforge_audit::{AuditEventKind, AuditStore, FileAuditStore};
use agentforge_core::agent::{AgentRole, AgentTask};
use agentforge_core::task::{TaskGraph, TaskId, TaskState};
use agentforge_operator::leases::{
    LEASE_LOCK_RELATIVE_PATH, expire_leases, grant_lease, list_leases, list_workers, load_workers,
    local_run_conflict, release_lease, renew_lease, try_expire_leases,
};
use agentforge_operator::transition_task;
use agentforge_state::{FileTaskStore, TaskStore};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static SEQUENCE: AtomicU64 = AtomicU64::new(0);

fn root() -> PathBuf {
    let sequence = SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!(
        "agentforge-leases-{}-{sequence}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join(".forge/workers")).expect("root");
    root
}

fn task(id: &str, path: &str) -> AgentTask {
    let mut task = AgentTask::new(id, "P4-M004", AgentRole::Implementer, "lease fixture");
    task.allowed_paths = vec![path.to_owned()];
    task
}

/// Four tasks: T0001 owns `src/a`, T0002 `src/b`, T0003 `src/a/inner` (overlaps T0001), and
/// T0004 `src/d`. Two workers with one slot each.
fn fixture() -> PathBuf {
    let root = root();
    let graph = TaskGraph::from_tasks([
        task("P4-M004-T0001", "src/a"),
        task("P4-M004-T0002", "src/b"),
        task("P4-M004-T0003", "src/a/inner"),
        task("P4-M004-T0004", "src/d"),
    ])
    .expect("graph");
    FileTaskStore::for_project_root(&root)
        .save(&graph)
        .expect("snapshot");
    FileAuditStore::open(root.join(".forge/audit.log")).expect("audit");
    worker(
        &root,
        "w-a",
        "platform=linux-x86_64\ncapability=rust\nmax_leases=1\n",
    );
    worker(
        &root,
        "w-b",
        "platform=linux-x86_64\ncapability=rust\nmax_leases=1\n",
    );
    root
}

fn worker(root: &Path, id: &str, text: &str) {
    fs::write(root.join(format!(".forge/workers/{id}.conf")), text).expect("worker");
}

fn id(value: &str) -> TaskId {
    TaskId::parse(value).expect("task ID")
}

fn lease_events(root: &Path) -> Vec<(String, String)> {
    FileAuditStore::open(root.join(".forge/audit.log"))
        .expect("audit")
        .records()
        .iter()
        .map(|record| record.event())
        .filter(|event| event.kind() == AuditEventKind::LeaseRecorded)
        .map(|event| {
            (
                event.fields()["action"].clone(),
                event.fields()["lease_id"].clone(),
            )
        })
        .collect()
}

#[test]
fn worker_profiles_load_and_invalid_profiles_fail_closed() {
    let root = fixture();
    let workers = load_workers(&root).expect("workers");
    assert_eq!(
        workers
            .iter()
            .map(|worker| worker.worker_id().as_str())
            .collect::<Vec<_>>(),
        ["w-a", "w-b"]
    );
    assert_eq!(workers[0].platform(), "linux-x86_64");
    assert_eq!(workers[0].max_concurrent_leases(), 1);

    for (text, expected) in [
        (
            "platform=x\ncapability=rust\nmax_leases=1\ncolor=blue\n",
            "unknown",
        ),
        (
            "platform=x\ncapability=rust\nmax_leases=lots\n",
            "not a number",
        ),
        ("platform=x\ncapability=rust\nmax_leases=0\n", "w-bad"),
        ("platform=x\nmax_leases=1\n", "w-bad"),
        ("capability=rust\nmax_leases=1\n", "platform is missing"),
        (
            "platform=x\nplatform=y\ncapability=rust\nmax_leases=1\n",
            "repeated",
        ),
    ] {
        worker(&root, "w-bad", text);
        let error = load_workers(&root).expect_err(text).to_string();
        assert!(error.contains(expected), "{text:?} -> {error}");
    }
    fs::write(root.join(".forge/workers/w-bad.conf"), "#".repeat(5 * 1024)).expect("oversize");
    assert!(load_workers(&root).is_err());
    fs::remove_file(root.join(".forge/workers/w-bad.conf")).expect("remove");
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(
            root.join(".forge/workers/w-a.conf"),
            root.join(".forge/workers/w-link.conf"),
        )
        .expect("symlink");
        assert!(
            load_workers(&root)
                .expect_err("symlink")
                .to_string()
                .contains("not a regular file")
        );
    }
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn grants_respect_capacity_ownership_and_one_lease_per_task() {
    let root = fixture();
    let first = grant_lease(&root, &id("P4-M004-T0001"), None, 10_000, 1_000, "op").expect("grant");
    assert_eq!(
        (first.lease_id.as_str(), first.worker_id.as_str()),
        ("P4-M004-T0001.L1", "w-a")
    );
    assert_eq!((first.generation, first.expires_at_ms), (1, 11_000));
    let second = grant_lease(&root, &id("P4-M004-T0002"), None, 10_000, 1_000, "op").expect("2nd");
    assert_eq!(second.worker_id, "w-b", "w-a is full");

    let before = lease_events(&root);
    let again = grant_lease(&root, &id("P4-M004-T0001"), None, 10_000, 1_000, "op");
    assert!(again.is_err(), "one active lease per task");
    let overlap = grant_lease(&root, &id("P4-M004-T0003"), None, 10_000, 1_000, "op")
        .expect_err("overlap")
        .to_string();
    assert!(
        overlap.contains("src/a/inner") && overlap.contains("P4-M004-T0001"),
        "{overlap}"
    );
    let full = grant_lease(&root, &id("P4-M004-T0004"), None, 10_000, 1_000, "op");
    assert!(full.is_err(), "both workers are full");
    let unknown = grant_lease(
        &root,
        &id("P4-M004-T0004"),
        Some("w-z"),
        10_000,
        1_000,
        "op",
    )
    .expect_err("unknown worker")
    .to_string();
    assert!(unknown.contains("not registered"), "{unknown}");
    assert_eq!(lease_events(&root), before, "refusals write nothing");
    assert_eq!(
        list_leases(&root, 1_000).expect("list").len(),
        2,
        "refusals leave the snapshot unchanged"
    );

    let workers = list_workers(&root, 1_000).expect("workers");
    assert!(workers.iter().all(|worker| worker.active_leases == 1));
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn renew_release_and_regrant_advance_the_generation() {
    let root = fixture();
    grant_lease(
        &root,
        &id("P4-M004-T0001"),
        Some("w-b"),
        10_000,
        1_000,
        "op",
    )
    .expect("grant");
    let renewed = renew_lease(&root, "P4-M004-T0001.L1", 20_000, 5_000, "op").expect("renew");
    assert_eq!(renewed.expires_at_ms, 25_000);
    let released = release_lease(&root, "P4-M004-T0001.L1", 6_000, "op").expect("release");
    assert_eq!(released.state, "released");
    assert!(release_lease(&root, "P4-M004-T0001.L1", 6_000, "op").is_err());

    let regrant =
        grant_lease(&root, &id("P4-M004-T0001"), None, 10_000, 7_000, "op").expect("again");
    assert_eq!(
        (regrant.lease_id.as_str(), regrant.generation),
        ("P4-M004-T0001.L2", 2)
    );
    assert_eq!(
        lease_events(&root)
            .into_iter()
            .map(|(action, _)| action)
            .collect::<Vec<_>>(),
        ["granted", "renewed", "released", "granted"]
    );
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn due_leases_expire_once_and_cannot_be_renewed() {
    let root = fixture();
    assert!(
        expire_leases(&root, 1, "op").expect("no state").is_empty(),
        "no lease state is not an error"
    );
    grant_lease(&root, &id("P4-M004-T0001"), None, 10, 1_000, "op").expect("short");
    grant_lease(&root, &id("P4-M004-T0002"), None, 100_000, 1_000, "op").expect("long");
    assert_eq!(list_leases(&root, 2_000).expect("list")[0].state, "expired");

    let expired = expire_leases(&root, 2_000, "op").expect("expire");
    assert_eq!(expired.len(), 1);
    assert_eq!(expired[0].lease_id, "P4-M004-T0001.L1");
    assert!(expire_leases(&root, 2_000, "op").expect("again").is_empty());
    assert!(renew_lease(&root, "P4-M004-T0001.L1", 10_000, 2_000, "op").is_err());
    assert_eq!(
        lease_events(&root),
        [
            ("granted".into(), "P4-M004-T0001.L1".into()),
            ("granted".into(), "P4-M004-T0002.L1".into()),
            ("expired".into(), "P4-M004-T0001.L1".into()),
        ]
    );
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn renewing_a_past_due_lease_records_its_expiry() {
    let root = fixture();
    grant_lease(&root, &id("P4-M004-T0001"), None, 10, 1_000, "op").expect("grant");
    let error = renew_lease(&root, "P4-M004-T0001.L1", 10_000, 5_000, "op").expect_err("late");
    assert!(error.to_string().contains("expired"), "{error}");
    assert_eq!(list_leases(&root, 5_000).expect("list")[0].state, "expired");
    assert_eq!(
        lease_events(&root)
            .last()
            .map(|(action, _)| action.as_str()),
        Some("expired")
    );
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn held_lock_and_non_pending_tasks_are_refused_without_writes() {
    let root = fixture();
    fs::create_dir_all(root.join(".forge/state")).expect("state");
    fs::write(root.join(LEASE_LOCK_RELATIVE_PATH), "pid=1\n").expect("lock");
    let locked = grant_lease(&root, &id("P4-M004-T0001"), None, 10_000, 1_000, "op")
        .expect_err("locked")
        .to_string();
    assert!(locked.contains("locked"), "{locked}");
    assert_eq!(
        try_expire_leases(&root, 1_000, "forged").expect("try"),
        Some(Vec::new()),
        "no lease state yet, so nothing to lock"
    );
    fs::remove_file(root.join(LEASE_LOCK_RELATIVE_PATH)).expect("unlock");

    grant_lease(&root, &id("P4-M004-T0002"), None, 10, 1_000, "op").expect("grant");
    fs::write(root.join(LEASE_LOCK_RELATIVE_PATH), "pid=1\n").expect("lock");
    assert_eq!(
        try_expire_leases(&root, 5_000, "forged").expect("busy"),
        None,
        "the daemon sweeper skips a busy lock"
    );
    fs::remove_file(root.join(LEASE_LOCK_RELATIVE_PATH)).expect("unlock");

    transition_task(&root, &id("P4-M004-T0004"), TaskState::Running, "op").expect("running");
    let running = grant_lease(&root, &id("P4-M004-T0004"), None, 10_000, 1_000, "op")
        .expect_err("running")
        .to_string();
    assert!(running.contains("only pending tasks"), "{running}");
    assert_eq!(lease_events(&root).len(), 1);
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn local_runs_are_refused_for_leased_and_overlapping_tasks() {
    let root = fixture();
    let graph = FileTaskStore::for_project_root(&root)
        .load()
        .expect("load")
        .expect("graph");
    assert_eq!(
        local_run_conflict(&root, &graph, &id("P4-M004-T0001"), 1_000).expect("none"),
        None
    );
    grant_lease(&root, &id("P4-M004-T0001"), None, 10_000, 1_000, "op").expect("grant");
    let leased = local_run_conflict(&root, &graph, &id("P4-M004-T0001"), 2_000)
        .expect("check")
        .expect("leased");
    assert!(leased.contains("leased to worker w-a"), "{leased}");
    let overlap = local_run_conflict(&root, &graph, &id("P4-M004-T0003"), 2_000)
        .expect("check")
        .expect("overlap");
    assert!(overlap.contains("overlaps task P4-M004-T0001"), "{overlap}");
    assert_eq!(
        local_run_conflict(&root, &graph, &id("P4-M004-T0002"), 2_000).expect("free"),
        None
    );
    assert_eq!(
        local_run_conflict(&root, &graph, &id("P4-M004-T0001"), 20_000).expect("due"),
        None,
        "a lease past its expiry no longer blocks local runs"
    );
    fs::remove_dir_all(root).expect("cleanup");
}
