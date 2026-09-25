//! The authenticated remote-worker API (P4-M007).

use agentforge_adapter::parse_task_prompt;
use agentforge_audit::{AuditEventKind, AuditStore, FileAuditStore};
use agentforge_core::agent::{AgentRole, AgentTask, Capability};
use agentforge_core::task::{TaskGraph, TaskId};
use agentforge_daemon::worker_api::{WorkerApiServer, WorkerClient, load_worker_api_bind};
use agentforge_operator::leases::{grant_lease, list_leases, now_ms};
use agentforge_operator::secrets::enroll_worker;
use agentforge_state::{FileTaskStore, TaskStore};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

static SEQUENCE: AtomicU64 = AtomicU64::new(0);

fn git(root: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .current_dir(root)
        .args(args)
        .output()
        .expect("git");
    assert!(output.status.success(), "git {args:?}: {output:?}");
    String::from_utf8(output.stdout)
        .expect("utf8")
        .trim()
        .to_owned()
}

fn task() -> AgentTask {
    let mut task = AgentTask::new(
        "P4-M007-T0001",
        "P4-M007",
        AgentRole::Implementer,
        "remote\nworker goal",
    );
    task.capabilities = vec![Capability::RunLocalCommands];
    task.allowed_paths = vec!["src".into()];
    task.required_gates = vec!["workspace".into()];
    task
}

/// A committed repo with one task and two enrolled workers; returns (root, secret-a, secret-b).
fn project() -> (PathBuf, String, String) {
    let root = std::env::temp_dir().join(format!(
        "agentforge-worker-api-{}-{}",
        std::process::id(),
        SEQUENCE.fetch_add(1, Ordering::Relaxed)
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
    FileTaskStore::for_project_root(&root)
        .save(&TaskGraph::from_tasks([task()]).expect("graph"))
        .expect("snapshot");
    let mut secrets = Vec::new();
    for worker in ["remote-a", "remote-b"] {
        fs::write(
            root.join(format!(".forge/workers/{worker}.conf")),
            "platform=linux-x86_64\ncapability=rust\nmax_leases=2\n",
        )
        .expect("worker");
        // A fixed secret per worker keeps the fixture portable (enrollment needs /dev/urandom;
        // `secrets_are_private_and_enrolled_once` covers it on Unix).
        let secret = if worker == "remote-a" { "a" } else { "b" }.repeat(64);
        write_secret(&root, worker, &secret);
        secrets.push(secret);
    }
    let b = secrets.pop().expect("b");
    let a = secrets.pop().expect("a");
    (root, a, b)
}

fn write_secret(root: &Path, worker: &str, secret: &str) {
    let path = agentforge_operator::secrets::worker_secret_path(root, worker);
    fs::write(&path, format!("{secret}\n")).expect("secret");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).expect("chmod");
    }
}

fn start(root: &Path) -> WorkerApiServer {
    WorkerApiServer::start(root, "127.0.0.1:0".parse().expect("addr")).expect("server")
}

fn lease_events(root: &Path) -> Vec<String> {
    let path = root.join(".forge/audit.log");
    if !path.exists() {
        return Vec::new();
    }
    FileAuditStore::open(path)
        .expect("audit")
        .records()
        .iter()
        .map(|record| record.event())
        .filter(|event| event.kind() == AuditEventKind::LeaseRecorded)
        .map(|event| {
            format!(
                "{}:{}:{}",
                event.actor(),
                event.fields()["action"],
                event.fields().get("channel").map_or("-", String::as_str)
            )
        })
        .collect()
}

#[test]
fn an_enrolled_worker_claims_renews_and_releases_its_lease() {
    let (root, secret_a, _) = project();
    let server = start(&root);
    let endpoint = server.address.to_string();
    let client = WorkerClient::new(&endpoint, "remote-a", &secret_a).expect("client");
    assert_eq!(client.claim().expect("claim"), None, "nothing leased yet");

    grant_lease(
        &root,
        &TaskId::parse("P4-M007-T0001").expect("id"),
        Some("remote-a"),
        60_000,
        now_ms(),
        "op",
    )
    .expect("grant");
    let claim = client.claim().expect("claim").expect("claimed");
    assert_eq!(claim.lease_id, "P4-M007-T0001.L1");
    assert_eq!(claim.task_id, "P4-M007-T0001");
    assert_eq!(claim.generation, 1);
    assert_eq!(claim.base_commit, git(&root, &["rev-parse", "HEAD"]));
    assert_eq!(
        parse_task_prompt(&claim.contract).expect("contract"),
        task()
    );

    let expires = client.renew(&claim.lease_id, 120_000).expect("renew");
    assert!(expires > claim.expires_at_ms);
    client.release(&claim.lease_id).expect("release");
    assert_eq!(
        list_leases(&root, now_ms()).expect("list")[0].state,
        "released"
    );
    assert_eq!(
        lease_events(&root),
        [
            "op:granted:-",
            "worker:remote-a:claimed:remote",
            "worker:remote-a:renewed:remote",
            "worker:remote-a:released:remote",
        ]
    );
    drop(server);
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn bad_credentials_and_other_workers_leases_change_nothing() {
    let (root, secret_a, secret_b) = project();
    grant_lease(
        &root,
        &TaskId::parse("P4-M007-T0001").expect("id"),
        Some("remote-a"),
        60_000,
        now_ms(),
        "op",
    )
    .expect("grant");
    let server = start(&root);
    let endpoint = server.address.to_string();
    let before = lease_events(&root);

    for (worker, secret) in [
        ("remote-a", secret_b.as_str()),
        ("remote-a", &"0".repeat(64)),
        ("remote-z", secret_a.as_str()),
        ("remote-a", "short"),
    ] {
        let client = WorkerClient::new(&endpoint, worker, secret).expect("client");
        let error = client.claim().expect_err("refused");
        assert_eq!(error, "unauthorized", "{worker}");
    }
    // remote-b is authenticated but may not touch remote-a's lease.
    let other = WorkerClient::new(&endpoint, "remote-b", &secret_b).expect("client");
    assert_eq!(other.claim().expect("claim"), None);
    let error = other.release("P4-M007-T0001.L1").expect_err("not owner");
    assert!(error.contains("belongs to worker remote-a"), "{error}");
    let error = other
        .renew("P4-M007-T0001.L1", 60_000)
        .expect_err("not owner");
    assert!(error.contains("belongs to worker remote-a"), "{error}");
    assert_eq!(lease_events(&root), before, "refusals write nothing");
    assert_eq!(
        list_leases(&root, now_ms()).expect("list")[0].state,
        "active"
    );

    // A malformed frame is refused.
    let mut raw = std::net::TcpStream::connect(server.address).expect("connect");
    std::io::Write::write_all(&mut raw, b"HELLO\n").expect("write");
    let mut reply = String::new();
    std::io::Read::read_to_string(&mut raw, &mut reply).expect("read");
    assert_eq!(reply, "AFW1\tERR\tmalformed request\n");
    drop(server);
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn the_api_is_off_by_default_and_binds_only_loopback() {
    let (root, secret_a, _) = project();
    assert_eq!(load_worker_api_bind(&root).expect("off"), None);
    fs::write(root.join(".forge/worker-api.conf"), "bind=0.0.0.0:7420\n").expect("conf");
    assert!(
        load_worker_api_bind(&root)
            .expect_err("public")
            .contains("loopback")
    );
    fs::write(root.join(".forge/worker-api.conf"), "bind=127.0.0.1:7420\n").expect("conf");
    assert_eq!(
        load_worker_api_bind(&root).expect("ok"),
        Some("127.0.0.1:7420".parse().expect("addr"))
    );
    assert!(WorkerApiServer::start(&root, "0.0.0.0:0".parse().expect("addr")).is_err());
    assert!(
        WorkerClient::new("192.0.2.1:7420", "remote-a", &secret_a)
            .expect_err("non-loopback client")
            .contains("not loopback")
    );
    fs::remove_dir_all(root).expect("cleanup");
}

#[cfg(unix)]
#[test]
fn secrets_are_private_and_enrolled_once() {
    use agentforge_operator::secrets::{load_worker_secret, read_secret_file, worker_secret_path};
    use std::os::unix::fs::PermissionsExt;
    let (root, _, _) = project();
    fs::write(
        root.join(".forge/workers/remote-c.conf"),
        "platform=linux-x86_64\ncapability=rust\nmax_leases=1\n",
    )
    .expect("worker");
    let secret = enroll_worker(&root, "remote-c").expect("enroll");
    let path = worker_secret_path(&root, "remote-c");
    assert_eq!(
        fs::metadata(&path).expect("meta").permissions().mode() & 0o777,
        0o600
    );
    assert_eq!(secret.len(), 64);
    assert_eq!(
        load_worker_secret(&root, "remote-c").expect("load"),
        Some(secret)
    );
    assert!(
        enroll_worker(&root, "remote-c").is_err(),
        "already enrolled"
    );
    assert!(enroll_worker(&root, "remote-z").is_err(), "unregistered");
    fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).expect("chmod");
    assert!(
        read_secret_file(&path)
            .expect_err("world-readable")
            .to_string()
            .contains("chmod 600")
    );
    fs::remove_dir_all(root).expect("cleanup");
}
