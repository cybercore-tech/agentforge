#![allow(missing_docs)]

use agentforge_daemon::{DEFAULT_BIND, DaemonError, serve, status, stop};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[test]
fn daemon_lifecycle_is_loopback_only_and_cooperative() {
    let root = temporary_repo();
    let server_root = root.clone();
    let server = thread::spawn(move || serve(server_root, DEFAULT_BIND));

    let running = (0..50).find_map(|_| match status(&root) {
        Ok(value) => Some(value),
        Err(DaemonError::NotRunning) => {
            thread::sleep(Duration::from_millis(10));
            None
        }
        Err(error) => panic!("unexpected daemon status error: {error}"),
    });
    let running = match running {
        Some(value) => value,
        None => {
            let result = server.join().expect("server thread");
            if matches!(
                &result,
                Err(DaemonError::Io(error))
                    if error.kind() == std::io::ErrorKind::PermissionDenied
            ) {
                fs::remove_dir_all(root).expect("cleanup");
                return;
            }
            panic!("daemon should publish an endpoint; server result: {result:?}");
        }
    };
    assert!(running.endpoint.address.ip().is_loopback());
    assert!(running.endpoint.address.port() > 0);
    assert!(matches!(
        serve(&root, DEFAULT_BIND),
        Err(DaemonError::AlreadyRunning(_))
    ));

    stop(&root).expect("cooperative stop");
    assert!(server.join().expect("server thread").is_ok());
    assert!(matches!(status(&root), Err(DaemonError::NotRunning)));
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

fn temporary_repo() -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("agentforge-daemon-{stamp}"));
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

fn git(root: &Path, arguments: &[&str]) {
    let output = Command::new("git")
        .args(arguments)
        .current_dir(root)
        .output()
        .expect("git command");
    assert!(output.status.success(), "git {:?}: {:?}", arguments, output);
}
