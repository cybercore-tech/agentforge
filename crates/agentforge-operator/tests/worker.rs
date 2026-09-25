//! Same-host worker coverage (P4-M005). Unix-only: the fixture agents are `true`, `false`, and
//! `sleep`; the CLI test covers the worker portably with its fixture binary.
#![cfg(unix)]

use agentforge_adapter::{ProcessAdapter, ProcessAdapterConfig};
use agentforge_audit::{AuditEventKind, AuditStore, FileAuditStore};
use agentforge_core::agent::{AgentRole, AgentTask, Capability};
use agentforge_core::task::{TaskGraph, TaskId, TaskState};
use agentforge_operator::leases::{claim_lease, grant_lease, list_leases, now_ms};
use agentforge_operator::transition_task;
use agentforge_operator::worker::{WorkerOptions, WorkerReport, run_worker};
use agentforge_state::{FileTaskStore, TaskStore};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

static SEQUENCE: AtomicU64 = AtomicU64::new(0);

fn git(root: &Path, args: &[&str]) {
    let output = Command::new("git")
        .current_dir(root)
        .args(args)
        .output()
        .expect("git");
    assert!(output.status.success(), "git {args:?}: {output:?}");
}

/// A git project with tasks `T0001` (`a`) and `T0002` (`b`), and workers `w-a` and `w-b`.
fn project() -> PathBuf {
    let sequence = SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!(
        "agentforge-worker-{}-{sequence}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join(".forge/workers")).expect("root");
    git(&root, &["init", "-q", "-b", "main"]);
    git(&root, &["config", "user.name", "AgentForge Test"]);
    git(
        &root,
        &["config", "user.email", "agentforge@example.invalid"],
    );
    fs::write(root.join(".gitignore"), ".forge/\n").expect("gitignore");
    git(&root, &["add", ".gitignore"]);
    git(&root, &["commit", "-qm", "init"]);
    let task = |id: &str, path: &str| {
        let mut task = AgentTask::new(id, "P4-M005", AgentRole::Implementer, "worker fixture");
        task.capabilities = vec![Capability::RunLocalCommands];
        task.allowed_paths = vec![path.to_owned()];
        task
    };
    FileTaskStore::for_project_root(&root)
        .save(
            &TaskGraph::from_tasks([task("P4-M005-T0001", "a"), task("P4-M005-T0002", "b")])
                .expect("graph"),
        )
        .expect("snapshot");
    for worker in ["w-a", "w-b"] {
        fs::write(
            root.join(format!(".forge/workers/{worker}.conf")),
            "platform=linux-x86_64\ncapability=rust\nmax_leases=2\n",
        )
        .expect("worker");
    }
    root
}

fn adapter(program: &str, arguments: &[&str]) -> ProcessAdapter {
    let mut config = ProcessAdapterConfig::new("worker-fixture", program);
    for argument in arguments {
        config = config.with_argument(*argument);
    }
    ProcessAdapter::new(config).expect("adapter")
}

fn once() -> WorkerOptions {
    WorkerOptions {
        base_ref: "HEAD".into(),
        once: true,
        poll: Duration::from_millis(10),
    }
}

fn id(value: &str) -> TaskId {
    TaskId::parse(value).expect("task ID")
}

fn state(root: &Path, task: &str) -> TaskState {
    FileTaskStore::for_project_root(root)
        .load()
        .expect("load")
        .expect("graph")
        .get(&id(task))
        .expect("task")
        .state()
}

/// `(actor, action)` for every lease event, plus `AgentStarted` markers, in audit order.
fn timeline(root: &Path) -> Vec<String> {
    FileAuditStore::open(root.join(".forge/audit.log"))
        .expect("audit")
        .records()
        .iter()
        .map(|record| record.event())
        .filter_map(|event| match event.kind() {
            AuditEventKind::LeaseRecorded => {
                Some(format!("{}:{}", event.actor(), event.fields()["action"]))
            }
            AuditEventKind::AgentStarted => Some("agent".into()),
            _ => None,
        })
        .collect()
}

#[test]
fn a_worker_runs_its_leased_task_and_releases_the_lease() {
    let root = project();
    grant_lease(
        &root,
        &id("P4-M005-T0001"),
        Some("w-a"),
        600_000,
        now_ms(),
        "op",
    )
    .expect("grant");
    let mut claimed = Vec::new();
    let summary = run_worker(
        &root,
        "w-a",
        &adapter("/usr/bin/true", &[]),
        &once(),
        |report| {
            if let WorkerReport::Claimed { task_id, .. } = report {
                claimed.push(task_id.to_string());
            }
        },
    )
    .expect("worker");
    assert_eq!(claimed, ["P4-M005-T0001"]);
    assert_eq!((summary.tasks_run, summary.failures), (1, 0));
    assert_eq!(state(&root, "P4-M005-T0001"), TaskState::Running);
    assert_eq!(
        list_leases(&root, now_ms()).expect("list")[0].state,
        "released"
    );
    assert_eq!(
        timeline(&root),
        [
            "op:granted",
            "worker:w-a:claimed",
            "agent",
            "worker:w-a:released"
        ]
    );
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn a_worker_ignores_other_workers_leases_and_unregistered_workers_are_refused() {
    let root = project();
    grant_lease(
        &root,
        &id("P4-M005-T0001"),
        Some("w-b"),
        600_000,
        now_ms(),
        "op",
    )
    .expect("grant");
    let mut idle = 0;
    let summary = run_worker(
        &root,
        "w-a",
        &adapter("/usr/bin/true", &[]),
        &once(),
        |report| {
            if matches!(report, WorkerReport::Idle) {
                idle += 1;
            }
        },
    )
    .expect("worker");
    assert_eq!((summary.tasks_run, idle), (0, 1));
    assert_eq!(state(&root, "P4-M005-T0001"), TaskState::Pending);

    let error = run_worker(
        &root,
        "w-z",
        &adapter("/usr/bin/true", &[]),
        &once(),
        |_| {},
    )
    .expect_err("unregistered");
    assert!(error.to_string().contains("not registered"), "{error}");
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn claims_are_refused_for_other_workers_released_leases_and_started_tasks() {
    let root = project();
    grant_lease(
        &root,
        &id("P4-M005-T0001"),
        Some("w-a"),
        600_000,
        now_ms(),
        "op",
    )
    .expect("grant");
    let events = || timeline(&root).len();
    let before = events();
    let other = claim_lease(&root, "P4-M005-T0001.L1", "w-b", now_ms()).expect_err("other");
    assert!(
        other.to_string().contains("belongs to worker w-a"),
        "{other}"
    );
    let late = claim_lease(&root, "P4-M005-T0001.L1", "w-a", now_ms() + 3_600_000);
    assert!(late.is_err(), "past-due lease");
    transition_task(&root, &id("P4-M005-T0002"), TaskState::Running, "op").expect("running");
    assert_eq!(
        events(),
        before + 1,
        "only the past-due expiry was recorded"
    );

    let root2 = project();
    grant_lease(
        &root2,
        &id("P4-M005-T0001"),
        Some("w-a"),
        600_000,
        now_ms(),
        "op",
    )
    .expect("grant");
    transition_task(&root2, &id("P4-M005-T0001"), TaskState::Running, "op").expect("running");
    let started = claim_lease(&root2, "P4-M005-T0001.L1", "w-a", now_ms()).expect_err("started");
    assert!(
        started.to_string().contains("only pending tasks"),
        "{started}"
    );
    fs::remove_dir_all(root).expect("cleanup");
    fs::remove_dir_all(root2).expect("cleanup");
}

#[test]
fn a_slow_agent_keeps_its_lease_renewed() {
    let root = project();
    // A 1.5 s window renews about every 500 ms; the agent runs for about 3 s. Each renewal has
    // about 1 s of slack, which absorbs a slow CI runner's lock and fsync time (P5-M002).
    grant_lease(
        &root,
        &id("P4-M005-T0001"),
        Some("w-a"),
        1_500,
        now_ms(),
        "op",
    )
    .expect("grant");
    let mut renewed = 0;
    let mut failures = Vec::new();
    let summary = run_worker(
        &root,
        "w-a",
        &adapter("/bin/sleep", &["3"]),
        &once(),
        |report| {
            if let WorkerReport::Renewals {
                renewed: count,
                failures: failed,
            } = report
            {
                renewed = count;
                failures = failed.to_vec();
            }
        },
    )
    .expect("worker");
    assert_eq!(summary.failures, 0);
    assert!(failures.is_empty(), "renewal failed: {failures:?}");
    assert!(renewed >= 3, "renewed {renewed} times");
    let timeline = timeline(&root);
    assert!(
        !timeline.iter().any(|event| event.ends_with(":expired")),
        "{timeline:?}"
    );
    assert_eq!(
        timeline.last().map(String::as_str),
        Some("worker:w-a:released")
    );
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn a_failing_agent_still_releases_its_lease() {
    let root = project();
    grant_lease(
        &root,
        &id("P4-M005-T0001"),
        Some("w-a"),
        600_000,
        now_ms(),
        "op",
    )
    .expect("grant");
    let summary = run_worker(
        &root,
        "w-a",
        &adapter("/usr/bin/false", &[]),
        &once(),
        |_| {},
    )
    .expect("worker");
    assert_eq!((summary.tasks_run, summary.failures), (1, 1));
    assert_eq!(
        list_leases(&root, now_ms()).expect("list")[0].state,
        "released"
    );
    assert_ne!(state(&root, "P4-M005-T0001"), TaskState::Pending);
    fs::remove_dir_all(root).expect("cleanup");
}
