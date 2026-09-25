#![allow(missing_docs)]

use agentforge_daemon::{
    DEFAULT_BIND, DaemonError, DaemonStatus, restart_with_program, serve, start_with_program,
    status, stop,
};
use std::fs;
use std::net::{Shutdown, TcpStream};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::sync::{Mutex, MutexGuard};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

static TEMPORARY_REPO_COUNTER: AtomicU64 = AtomicU64::new(0);
// Startup canonicalizes the root, runs `git rev-parse`, and fsyncs the lock and
// endpoint files. Slow CI runners can take well over a second; a short fixed
// poll budget previously sent the tests into an unbounded join on a daemon that
// had started successfully.
const READY_TIMEOUT: Duration = Duration::from_secs(30);
const EXIT_TIMEOUT: Duration = Duration::from_secs(30);
// Windows can leave one foreground loopback daemon transition in flight while
// another test thread is tearing down its listener. Keep lifecycle ownership
// explicit within this test binary; each test still exercises the complete
// start/status/stop assertions against its own isolated repository.
static DAEMON_LIFECYCLE_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn daemon_lifecycle_is_loopback_only_and_cooperative() {
    let _lifecycle_guard = daemon_lifecycle_guard();
    let root = temporary_repo();
    let Some((server, running)) = start_foreground(&root) else {
        fs::remove_dir_all(root).expect("cleanup");
        return;
    };
    assert!(running.endpoint.address.ip().is_loopback());
    assert!(running.endpoint.address.port() > 0);
    assert!(matches!(
        serve(&root, DEFAULT_BIND),
        Err(DaemonError::AlreadyRunning(_))
    ));

    stop(&root).expect("cooperative stop");
    assert!(wait_for_exit(&server).is_ok());
    assert!(matches!(status(&root), Err(DaemonError::NotRunning)));

    let (restarted, _) = start_foreground(&root).expect("daemon should restart");
    stop(&root).expect("cooperative restart stop");
    assert!(wait_for_exit(&restarted).is_ok());
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn malformed_endpoint_fails_closed() {
    let root = temporary_repo();
    let daemon_dir = root.join(".forge/daemon");
    fs::create_dir_all(&daemon_dir).expect("daemon directory");
    fs::write(
        daemon_dir.join("endpoint"),
        b"version=1\npid=1\naddress=0.0.0.0:1\n",
    )
    .expect("endpoint");
    assert!(matches!(status(&root), Err(DaemonError::Protocol(_))));
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn stale_identity_fails_closed() {
    let root = temporary_repo();
    let daemon_dir = root.join(".forge/daemon");
    let canonical_daemon_dir = root
        .canonicalize()
        .expect("canonical temporary repository")
        .join(".forge/daemon");
    fs::create_dir_all(&daemon_dir).expect("daemon directory");
    fs::write(daemon_dir.join("lock"), b"pid=1\n").expect("lock");
    fs::write(
        daemon_dir.join("endpoint"),
        b"version=1\npid=1\naddress=127.0.0.1:1\n",
    )
    .expect("endpoint");
    assert!(matches!(
        serve(&root, DEFAULT_BIND),
        Err(DaemonError::StaleInstance(path)) if path == canonical_daemon_dir.join("lock")
    ));
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn disconnected_client_does_not_stop_daemon() {
    let _lifecycle_guard = daemon_lifecycle_guard();
    let root = temporary_repo();
    let Some((server, running)) = start_foreground(&root) else {
        fs::remove_dir_all(root).expect("cleanup");
        return;
    };

    let client = TcpStream::connect(running.endpoint.address).expect("connect client");
    client.shutdown(Shutdown::Both).expect("disconnect client");
    assert!(status(&root).is_ok(), "daemon should survive a disconnect");

    stop(&root).expect("cooperative stop");
    assert!(wait_for_exit(&server).is_ok());
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn stalled_client_does_not_block_stop() {
    let _lifecycle_guard = daemon_lifecycle_guard();
    let root = temporary_repo();
    let Some((server, running)) = start_foreground(&root) else {
        fs::remove_dir_all(root).expect("cleanup");
        return;
    };

    // Connect and hold the connection open without sending a frame.
    let stalled = TcpStream::connect(running.endpoint.address).expect("connect stalled client");
    stop(&root).expect("stop should not wait on a stalled client");
    assert!(wait_for_exit(&server).is_ok());
    drop(stalled);
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn spawned_daemon_start_and_restart_are_bounded_and_cooperative() {
    let _lifecycle_guard = daemon_lifecycle_guard();
    let root = temporary_repo();
    let forged = env!("CARGO_BIN_EXE_forged");
    let running = match start_with_program(&root, DEFAULT_BIND, forged) {
        Ok(value) => value,
        Err(DaemonError::Execution(message)) if message.contains("Operation not permitted") => {
            fs::remove_dir_all(root).expect("cleanup");
            return;
        }
        Err(error) => panic!("spawn daemon: {error}"),
    };
    assert!(running.endpoint.address.ip().is_loopback());
    let restarted =
        restart_with_program(&root, DEFAULT_BIND, forged).expect("restart daemon cooperatively");
    assert!(restarted.endpoint.address.ip().is_loopback());
    stop(&root).expect("stop restarted daemon");
    assert!(
        matches!(status(&root), Err(DaemonError::NotRunning)),
        "daemon metadata should be removed after stop"
    );
    fs::remove_dir_all(root).expect("cleanup");
}

fn temporary_repo() -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let sequence = TEMPORARY_REPO_COUNTER.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!("agentforge-daemon-{stamp}-{sequence}"));
    fs::create_dir(&root).expect("temporary root");
    git(&root, &["init", "-q"]);
    fs::write(root.join("README.md"), "daemon fixture\n").expect("fixture");
    git(
        &root,
        &["config", "user.email", "agentforge@example.invalid"],
    );
    git(&root, &["config", "user.name", "AgentForge Test"]);
    git(&root, &["add", "README.md"]);
    git(&root, &["commit", "-qm", "fixture"]);
    root
}

type ServerResult = Result<(), DaemonError>;

/// Starts a foreground daemon thread and waits, with a deadline, for its
/// endpoint. Returns `None` only when the platform denies the loopback bind.
fn start_foreground(root: &Path) -> Option<(Receiver<ServerResult>, DaemonStatus)> {
    let (sender, receiver) = mpsc::channel();
    let server_root = root.to_path_buf();
    thread::spawn(move || {
        let _ = sender.send(serve(server_root, DEFAULT_BIND));
    });
    let deadline = Instant::now() + READY_TIMEOUT;
    loop {
        match status(root) {
            Ok(running) => return Some((receiver, running)),
            Err(DaemonError::NotRunning) => {}
            Err(error) => panic!("unexpected daemon status error: {error}"),
        }
        match receiver.try_recv() {
            Ok(Err(DaemonError::Io(error)))
                if error.kind() == std::io::ErrorKind::PermissionDenied =>
            {
                return None;
            }
            Ok(result) => panic!("daemon exited before publishing an endpoint: {result:?}"),
            Err(TryRecvError::Disconnected) => panic!("daemon thread panicked during startup"),
            Err(TryRecvError::Empty) => {}
        }
        assert!(
            Instant::now() < deadline,
            "daemon did not publish an endpoint within {READY_TIMEOUT:?}"
        );
        thread::sleep(Duration::from_millis(10));
    }
}

/// Waits, with a deadline, for a stopped foreground daemon thread to finish.
fn wait_for_exit(server: &Receiver<ServerResult>) -> ServerResult {
    server.recv_timeout(EXIT_TIMEOUT).unwrap_or_else(|error| {
        panic!("daemon thread did not exit within {EXIT_TIMEOUT:?}: {error}")
    })
}

fn daemon_lifecycle_guard() -> MutexGuard<'static, ()> {
    DAEMON_LIFECYCLE_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn git(root: &Path, arguments: &[&str]) {
    let output = Command::new("git")
        .args(arguments)
        .current_dir(root)
        .output()
        .expect("git command");
    assert!(output.status.success(), "git {:?}: {:?}", arguments, output);
}

#[test]
fn running_daemon_expires_due_leases() {
    use agentforge_core::remote::{
        LeaseBook, LeaseId, LeaseState, RemoteWorkerDescriptor, RemoteWorkerId, WorkerCapability,
    };
    use agentforge_state::{FileLeaseStore, LeaseStore};
    let _lifecycle_guard = daemon_lifecycle_guard();
    let root = temporary_repo();
    let worker = RemoteWorkerDescriptor::new(
        RemoteWorkerId::parse("w-a").expect("worker"),
        "linux-x86_64",
        vec![WorkerCapability::parse("rust").expect("capability")],
        1,
    )
    .expect("descriptor");
    let now = agentforge_orchestrator::wall_clock_ms();
    let mut book = LeaseBook::new();
    book.grant(
        &worker,
        LeaseId::parse("P4-M004-T0001.L1").expect("lease"),
        agentforge_core::task::TaskId::parse("P4-M004-T0001").expect("task"),
        now,
        now + 100,
    )
    .expect("grant");
    let store = FileLeaseStore::for_project_root(&root);
    store.save(&book).expect("save");
    // A real project: the task snapshot must exist before an audit log is created.
    agentforge_state::TaskStore::save(
        &agentforge_state::FileTaskStore::for_project_root(&root),
        &agentforge_core::task::TaskGraph::from_tasks([agentforge_core::agent::AgentTask::new(
            "P4-M004-T0001",
            "P4-M004",
            agentforge_core::agent::AgentRole::Implementer,
            "sweep fixture",
        )])
        .expect("graph"),
    )
    .expect("task snapshot");

    let Some((server, _)) = start_foreground(&root) else {
        fs::remove_dir_all(root).expect("cleanup");
        return;
    };
    // The sweep runs every 5 s; allow a generous margin on slow CI runners.
    let deadline = Instant::now() + Duration::from_secs(20);
    let state = loop {
        let state = store
            .load()
            .expect("load")
            .expect("book")
            .leases()
            .next()
            .expect("lease")
            .state();
        if state == LeaseState::Expired || Instant::now() >= deadline {
            break state;
        }
        thread::sleep(Duration::from_millis(100));
    };
    stop(&root).expect("cooperative stop");
    assert!(wait_for_exit(&server).is_ok());
    assert_eq!(state, LeaseState::Expired);
    use agentforge_audit::AuditStore as _;
    let audit =
        agentforge_audit::FileAuditStore::open(root.join(".forge/audit.log")).expect("audit");
    let event = audit.records()[0].event();
    assert_eq!(
        event.kind(),
        agentforge_audit::AuditEventKind::LeaseRecorded
    );
    assert_eq!(event.actor(), "forged");
    assert_eq!(event.fields()["action"], "expired");
    assert!(!root.join(".forge/state/remote-leases.lock").exists());
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn running_daemon_dispatches_ready_tasks_under_an_enabled_policy() {
    use agentforge_audit::AuditStore as _;
    use agentforge_state::{FileLeaseStore, LeaseStore, TaskStore};
    let _lifecycle_guard = daemon_lifecycle_guard();
    let root = temporary_repo();
    let mut task = agentforge_core::agent::AgentTask::new(
        "P4-M006-T0001",
        "P4-M006",
        agentforge_core::agent::AgentRole::Implementer,
        "dispatch fixture",
    );
    task.allowed_paths = vec!["a.txt".into()];
    agentforge_state::FileTaskStore::for_project_root(&root)
        .save(&agentforge_core::task::TaskGraph::from_tasks([task]).expect("graph"))
        .expect("task snapshot");
    fs::create_dir_all(root.join(".forge/workers")).expect("workers");
    fs::write(
        root.join(".forge/workers/w-a.conf"),
        "platform=linux-x86_64\ncapability=rust\nmax_leases=1\n",
    )
    .expect("worker");
    fs::write(
        root.join(".forge/dispatch.conf"),
        "enabled=true\nmilestone=P4-M006\n",
    )
    .expect("policy");

    let Some((server, _)) = start_foreground(&root) else {
        fs::remove_dir_all(root).expect("cleanup");
        return;
    };
    let store = FileLeaseStore::for_project_root(&root);
    let deadline = Instant::now() + Duration::from_secs(20);
    let leased = loop {
        let leased = store
            .load()
            .expect("load")
            .is_some_and(|book| book.leases().count() == 1);
        if leased || Instant::now() >= deadline {
            break leased;
        }
        thread::sleep(Duration::from_millis(100));
    };
    stop(&root).expect("cooperative stop");
    assert!(wait_for_exit(&server).is_ok());
    assert!(leased, "the daemon dispatched within the tick");
    let audit =
        agentforge_audit::FileAuditStore::open(root.join(".forge/audit.log")).expect("audit");
    let event = audit.records()[0].event();
    assert_eq!(event.actor(), "forged");
    assert_eq!(event.fields()["action"], "granted");
    assert_eq!(event.fields()["dispatch"], "auto");
    assert_eq!(event.fields()["worker_id"], "w-a");
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn forged_serves_the_worker_api_from_its_configuration() {
    use agentforge_daemon::worker_api::WorkerClient;
    use agentforge_state::TaskStore;
    let _lifecycle_guard = daemon_lifecycle_guard();
    let root = temporary_repo();
    agentforge_state::FileTaskStore::for_project_root(&root)
        .save(&agentforge_core::task::TaskGraph::from_tasks([]).expect("graph"))
        .expect("task snapshot");
    fs::create_dir_all(root.join(".forge/workers")).expect("workers");
    fs::write(
        root.join(".forge/workers/remote-a.conf"),
        "platform=linux-x86_64\ncapability=rust\nmax_leases=1\n",
    )
    .expect("worker");
    // A fixed secret keeps this portable; enrollment itself is covered on Unix in worker_api.rs.
    let secret = "c".repeat(64);
    let secret_path = agentforge_operator::secrets::worker_secret_path(&root, "remote-a");
    fs::write(&secret_path, format!("{secret}\n")).expect("secret");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&secret_path, fs::Permissions::from_mode(0o600)).expect("chmod");
    }
    // Reserve a free loopback port for the configuration.
    let port = std::net::TcpListener::bind("127.0.0.1:0")
        .expect("probe")
        .local_addr()
        .expect("addr")
        .port();
    fs::write(
        root.join(".forge/worker-api.conf"),
        format!("bind=127.0.0.1:{port}\n"),
    )
    .expect("conf");

    let Some((server, _)) = start_foreground(&root) else {
        fs::remove_dir_all(root).expect("cleanup");
        return;
    };
    let client =
        WorkerClient::new(&format!("127.0.0.1:{port}"), "remote-a", &secret).expect("client");
    let claimed = client.claim();
    stop(&root).expect("cooperative stop");
    assert!(wait_for_exit(&server).is_ok());
    assert_eq!(
        claimed.expect("claim"),
        None,
        "authenticated, nothing leased"
    );
    assert!(
        std::net::TcpStream::connect(("127.0.0.1", port)).is_err(),
        "the worker API stops with the daemon"
    );

    // A non-loopback bind stops the daemon from starting.
    fs::write(root.join(".forge/worker-api.conf"), "bind=0.0.0.0:7420\n").expect("conf");
    assert!(matches!(
        serve(&root, DEFAULT_BIND),
        Err(DaemonError::Protocol(reason)) if reason.contains("loopback")
    ));
    fs::remove_dir_all(root).expect("cleanup");
}

/// Finding 16 (P4-M010 Amendment 1): a daemon started by `start_with_program` outlives its
/// starter's stderr pipe. Its log lines must never kill the lease sweep or a worker API connection.
#[test]
fn a_spawned_daemon_keeps_working_after_its_starter_is_gone() {
    use agentforge_audit::AuditStore as _;
    use agentforge_daemon::worker_api::{ClientError, WorkerClient};
    use agentforge_operator::leases::{grant_lease, now_ms};
    use agentforge_state::TaskStore;
    let _lifecycle_guard = daemon_lifecycle_guard();
    let root = temporary_repo();
    let mut task = agentforge_core::agent::AgentTask::new(
        "P4-M010-T0001",
        "P4-M010",
        agentforge_core::agent::AgentRole::Implementer,
        "sweep after the starter exits",
    );
    task.allowed_paths = vec!["a.txt".into()];
    agentforge_state::FileTaskStore::for_project_root(&root)
        .save(&agentforge_core::task::TaskGraph::from_tasks([task]).expect("graph"))
        .expect("task snapshot");
    fs::create_dir_all(root.join(".forge/workers")).expect("workers");
    fs::write(
        root.join(".forge/workers/remote-a.conf"),
        "platform=linux-x86_64\ncapability=rust\nmax_leases=1\n",
    )
    .expect("worker");
    let secret_path = agentforge_operator::secrets::worker_secret_path(&root, "remote-a");
    fs::write(&secret_path, format!("{}\n", "c".repeat(64))).expect("secret");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&secret_path, fs::Permissions::from_mode(0o600)).expect("chmod");
    }
    let port = std::net::TcpListener::bind("127.0.0.1:0")
        .expect("probe")
        .local_addr()
        .expect("addr")
        .port();
    fs::write(
        root.join(".forge/worker-api.conf"),
        format!("bind=127.0.0.1:{port}\n"),
    )
    .expect("conf");

    // `start_with_program` returns once the daemon answers; whatever it used to capture startup
    // errors is gone after that, exactly as when `forge daemon start` exits.
    match start_with_program(&root, DEFAULT_BIND, env!("CARGO_BIN_EXE_forged")) {
        Ok(_) => {}
        Err(DaemonError::Execution(message)) if message.contains("Operation not permitted") => {
            fs::remove_dir_all(root).expect("cleanup");
            return;
        }
        Err(error) => panic!("spawn daemon: {error}"),
    }

    // A refused worker gets an answer, not a dropped connection: the refusal is logged first.
    let client = WorkerClient::new(&format!("127.0.0.1:{port}"), "remote-a", &"x".repeat(64))
        .expect("client");
    let refused = client.claim();

    // Two consecutive expiries: the sweep logs after the first and must still run for the second.
    let task_id = agentforge_core::task::TaskId::parse("P4-M010-T0001").expect("task");
    let expired = |root: &Path| {
        let path = root.join(".forge/audit.log");
        if !path.exists() {
            return 0;
        }
        agentforge_audit::FileAuditStore::open(path)
            .expect("audit")
            .records()
            .iter()
            .filter(|record| {
                record.event().fields().get("action").map(String::as_str) == Some("expired")
            })
            .count()
    };
    let mut recorded = Vec::new();
    for round in 1..=2 {
        grant_lease(&root, &task_id, Some("remote-a"), 1, now_ms(), "op").expect("grant");
        // The sweep runs every 5 s; allow a generous margin on slow CI runners.
        let deadline = Instant::now() + Duration::from_secs(20);
        while expired(&root) < round && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(100));
        }
        recorded.push(expired(&root));
    }
    // A sweep records an expiry, then logs it; `stop` waits out an in-progress sweep, so the log
    // is complete only after it (a CI run once read it in between).
    stop(&root).expect("cooperative stop");
    let log = fs::read_to_string(root.join(".forge/daemon/forged.log")).unwrap_or_default();

    assert_eq!(
        refused.expect_err("wrong secret"),
        ClientError::Refused("unauthorized".into())
    );
    assert_eq!(recorded, [1, 2], "the sweep survived its own log line");
    assert!(
        log.contains("forged: worker API listening on 127.0.0.1:"),
        "{log}"
    );
    assert!(
        log.contains("refused an unauthenticated CLAIM request"),
        "{log}"
    );
    assert_eq!(
        log.matches("forged: expired 1 lease(s)").count(),
        2,
        "{log}"
    );
    fs::remove_dir_all(root).expect("cleanup");
}
