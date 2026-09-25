//! Opt-in automatic dispatch (P4-M006).

use agentforge_audit::{AuditEventKind, AuditStore, FileAuditStore};
use agentforge_core::agent::{AgentRole, AgentTask, ApprovalBoundary};
use agentforge_core::task::{TaskGraph, TaskId};
use agentforge_operator::approve_task;
use agentforge_operator::dispatch::{DispatchPolicy, dispatch_ready};
use agentforge_operator::leases::{expire_leases, list_leases, release_lease};
use agentforge_state::{FileTaskStore, TaskStore};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static SEQUENCE: AtomicU64 = AtomicU64::new(0);

fn task(id: &str, milestone: &str, path: &str) -> AgentTask {
    let mut task = AgentTask::new(id, milestone, AgentRole::Implementer, "dispatch fixture");
    task.allowed_paths = vec![path.to_owned()];
    task
}

fn project(tasks: Vec<AgentTask>, workers: &[(&str, u16)], policy: Option<&str>) -> PathBuf {
    let sequence = SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!(
        "agentforge-dispatch-{}-{sequence}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join(".forge/workers")).expect("root");
    FileTaskStore::for_project_root(&root)
        .save(&TaskGraph::from_tasks(tasks).expect("graph"))
        .expect("snapshot");
    for (worker, slots) in workers {
        fs::write(
            root.join(format!(".forge/workers/{worker}.conf")),
            format!("platform=linux-x86_64\ncapability=rust\nmax_leases={slots}\n"),
        )
        .expect("worker");
    }
    if let Some(policy) = policy {
        fs::write(root.join(".forge/dispatch.conf"), policy).expect("policy");
    }
    root
}

fn granted(pass: &agentforge_operator::dispatch::DispatchPass) -> Vec<(&str, &str)> {
    pass.granted
        .iter()
        .map(|lease| (lease.task_id.as_str(), lease.worker_id.as_str()))
        .collect()
}

fn id(value: &str) -> TaskId {
    TaskId::parse(value).expect("task ID")
}

#[test]
fn policies_load_and_invalid_policies_fail_closed() {
    let root = project(vec![], &[], None);
    assert_eq!(DispatchPolicy::load(&root).expect("missing"), None);
    let write = |text: &str| fs::write(root.join(".forge/dispatch.conf"), text).expect("write");
    write("enabled=true\nmilestone=P4-M006\nmilestone=P9-M001\nttl_ms=60000\nmax_per_tick=2\n");
    let policy = DispatchPolicy::load(&root).expect("valid").expect("some");
    assert!(policy.enabled);
    assert_eq!(policy.milestones.len(), 2);
    assert_eq!((policy.ttl_ms, policy.max_per_tick), (60_000, 2));
    write("enabled=true\nmilestone=P4-M006\n");
    let defaults = DispatchPolicy::load(&root).expect("valid").expect("some");
    assert_eq!((defaults.ttl_ms, defaults.max_per_tick), (900_000, 4));
    for (text, expected) in [
        ("milestone=P4-M006\n", "enabled is missing"),
        ("enabled=true\n", "at least one milestone"),
        ("enabled=yes\nmilestone=P4-M006\n", "true or false"),
        ("enabled=true\nmilestone=P4-M006\ncolor=blue\n", "unknown"),
        ("enabled=true\nmilestone=P4 M006\n", "invalid milestone"),
        ("enabled=true\nmilestone=P4-M006\nttl_ms=0\n", "TTL"),
        (
            "enabled=true\nmilestone=P4-M006\nmax_per_tick=0\n",
            "max_per_tick",
        ),
        (
            "enabled=true\nmilestone=P4-M006\nmax_per_tick=65\n",
            "max_per_tick",
        ),
    ] {
        write(text);
        let error = DispatchPolicy::load(&root).expect_err(text).to_string();
        assert!(error.contains(expected), "{text:?} -> {error}");
    }
    write(&"#".repeat(5 * 1024));
    assert!(DispatchPolicy::load(&root).is_err());
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn without_an_enabled_policy_nothing_is_dispatched() {
    let tasks = vec![task("P4-M006-T0001", "P4-M006", "a")];
    let root = project(tasks.clone(), &[("w-a", 2)], None);
    assert!(
        !dispatch_ready(&root, 1_000, "forged")
            .expect("pass")
            .enabled
    );
    fs::write(
        root.join(".forge/dispatch.conf"),
        "enabled=false\nmilestone=P4-M006\n",
    )
    .expect("policy");
    let pass = dispatch_ready(&root, 1_000, "forged").expect("pass");
    assert!(!pass.enabled && pass.granted.is_empty());
    assert!(list_leases(&root, 1_000).expect("list").is_empty());
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn dispatch_is_scoped_ordered_bounded_and_audited() {
    let root = project(
        vec![
            task("P4-M006-T0001", "P4-M006", "a"),
            task("P4-M006-T0002", "P4-M006", "b"),
            task("P4-M006-T0003", "P4-M006", "c"),
            task("P9-M001-T0001", "P9-M001", "z"),
        ],
        &[("w-a", 1), ("w-b", 1)],
        Some("enabled=true\nmilestone=P4-M006\nmax_per_tick=4\nttl_ms=60000\n"),
    );
    let pass = dispatch_ready(&root, 1_000, "forged").expect("pass");
    assert_eq!(
        granted(&pass),
        [("P4-M006-T0001", "w-a"), ("P4-M006-T0002", "w-b")]
    );
    assert_eq!(
        pass.stopped.as_deref(),
        Some("every registered worker is at capacity")
    );
    assert!(
        list_leases(&root, 1_000)
            .expect("list")
            .iter()
            .all(|lease| lease.task_id != "P9-M001-T0001"),
        "unlisted milestone"
    );
    let audit = FileAuditStore::open(root.join(".forge/audit.log")).expect("audit");
    for record in audit.records() {
        let event = record.event();
        assert_eq!(event.kind(), AuditEventKind::LeaseRecorded);
        assert_eq!(event.actor(), "forged");
        assert_eq!(event.fields()["dispatch"], "auto");
    }

    // Freed capacity: the next pass picks up T0003; the per-tick limit applies.
    release_lease(&root, "P4-M006-T0001.L1", 2_000, "op").expect("release");
    let next = dispatch_ready(&root, 2_000, "forged").expect("pass");
    assert_eq!(granted(&next), [("P4-M006-T0003", "w-a")]);
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn dispatch_runs_once_per_task_and_respects_approvals_and_ownership() {
    let mut gated = task("P4-M006-T0001", "P4-M006", "a");
    gated.required_approvals = vec![
        ApprovalBoundary::ActivateImplementationPlan,
        ApprovalBoundary::MergeProtectedBranch,
    ];
    let root = project(
        vec![
            gated,
            task("P4-M006-T0002", "P4-M006", "b"),
            task("P4-M006-T0003", "P4-M006", "b/inner"),
            task("P4-M006-T0004", "P4-M006", "d"),
        ],
        &[("w-a", 8)],
        Some("enabled=true\nmilestone=P4-M006\nmax_per_tick=2\nttl_ms=100\n"),
    );
    let first = dispatch_ready(&root, 1_000, "forged").expect("pass");
    // T0001 lacks its pre-execution approval (the merge approval comes after review); T0003
    // overlaps leased T0002; the per-tick limit stops after two grants.
    assert_eq!(
        granted(&first),
        [("P4-M006-T0002", "w-a"), ("P4-M006-T0004", "w-a")]
    );
    let reasons = first
        .skipped
        .iter()
        .cloned()
        .collect::<std::collections::BTreeMap<_, _>>();
    assert!(
        reasons["P4-M006-T0001"].contains("activate_implementation_plan"),
        "{reasons:?}"
    );
    assert!(reasons["P4-M006-T0003"].contains("overlaps"), "{reasons:?}");

    approve_task(
        &root,
        &id("P4-M006-T0001"),
        ApprovalBoundary::ActivateImplementationPlan,
        "op",
    )
    .expect("approve");
    // The 100 ms leases have expired; T0002 and T0004 never run, but are not re-dispatched.
    expire_leases(&root, 5_000, "op").expect("expire");
    let second = dispatch_ready(&root, 5_000, "forged").expect("pass");
    assert_eq!(
        granted(&second),
        [("P4-M006-T0001", "w-a"), ("P4-M006-T0003", "w-a")]
    );
    let reasons = second
        .skipped
        .iter()
        .cloned()
        .collect::<std::collections::BTreeMap<_, _>>();
    for task in ["P4-M006-T0002", "P4-M006-T0004"] {
        assert!(reasons[task].contains("already had a lease"), "{reasons:?}");
    }
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn no_registered_workers_stops_the_pass() {
    let root = project(
        vec![task("P4-M006-T0001", "P4-M006", "a")],
        &[],
        Some("enabled=true\nmilestone=P4-M006\n"),
    );
    let pass = dispatch_ready(&root, 1_000, "forged").expect("pass");
    assert!(pass.enabled && pass.granted.is_empty());
    assert_eq!(pass.stopped.as_deref(), Some("no workers are registered"));
    fs::remove_dir_all(root).expect("cleanup");
}
