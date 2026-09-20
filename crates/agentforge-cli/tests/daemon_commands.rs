#![allow(missing_docs)]

use agentforge_daemon::{DEFAULT_BIND, DaemonError, serve, status};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[test]
fn daemon_status_and_stop_are_operator_commands() {
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
    if running.is_none() {
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

    let status_output = forge(&root, &["daemon", "status", root.to_str().expect("root")]);
    assert!(status_output.status.success(), "{status_output:?}");
    assert!(String::from_utf8_lossy(&status_output.stdout).contains("daemon running at"));
    let stop_output = forge(&root, &["daemon", "stop", root.to_str().expect("root")]);
    assert!(stop_output.status.success(), "{stop_output:?}");
    assert!(server.join().expect("server thread").is_ok());
    fs::remove_dir_all(root).expect("cleanup");
}

fn forge(root: &Path, arguments: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_forge"))
        .args(arguments)
        .current_dir(root)
        .output()
        .expect("forge command")
}

fn temporary_repo() -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("agentforge-cli-daemon-{stamp}"));
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
