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

    let initialized = forge(&root, &["init", root.to_str().expect("root")]);
    assert!(initialized.status.success(), "{initialized:?}");
    let created = forge(
        &root,
        &[
            "task",
            "create",
            root.to_str().expect("root"),
            "P2-M005-T0001",
            "P2-M005",
            "implementer",
            "exercise daemon",
            "--allowed",
            "agentforge-fixture-output.txt",
            "--capability",
            "write_owned_paths",
            "--capability",
            "run_local_commands",
        ],
    );
    assert!(created.status.success(), "{created:?}");
    let prepared = forge(
        &root,
        &[
            "worktree",
            "create",
            root.to_str().expect("root"),
            "P2-M005-T0001",
            "HEAD",
        ],
    );
    assert!(prepared.status.success(), "{prepared:?}");
    let executable = agent_fixture(&root);

    let run_output = forge(
        &root,
        &[
            "daemon",
            "run",
            root.to_str().expect("root"),
            "P2-M005-T0001",
            executable.to_str().expect("fixture path"),
        ],
    );
    assert!(run_output.status.success(), "{run_output:?}");
    assert!(String::from_utf8_lossy(&run_output.stdout).contains("task=P2-M005-T0001"));
    assert_eq!(
        fs::read_to_string(
            root.join(".forge/worktrees/P2-M005-T0001/agentforge-fixture-output.txt")
        )
        .expect("fixture output"),
        "fixture executed\n"
    );
    let hud_output = forge(&root, &["hud", root.to_str().expect("root")]);
    assert!(hud_output.status.success(), "{hud_output:?}");
    assert!(String::from_utf8_lossy(&hud_output.stdout).contains("P2-M005-T0001"));
    let accepted = forge(
        &root,
        &[
            "task",
            "accept",
            root.to_str().expect("root"),
            "P2-M005-T0001",
            "--actor",
            "operator",
        ],
    );
    assert!(accepted.status.success(), "{accepted:?}");
    fs::remove_file(root.join(".forge/worktrees/P2-M005-T0001/agentforge-fixture-output.txt"))
        .expect("remove fixture output before retirement");

    let status_output = forge(&root, &["daemon", "status", root.to_str().expect("root")]);
    assert!(status_output.status.success(), "{status_output:?}");
    assert!(String::from_utf8_lossy(&status_output.stdout).contains("daemon running at"));
    let stop_output = forge(&root, &["daemon", "stop", root.to_str().expect("root")]);
    assert!(stop_output.status.success(), "{stop_output:?}");
    assert!(server.join().expect("server thread").is_ok());
    let retired = forge(
        &root,
        &[
            "worktree",
            "retire",
            root.to_str().expect("root"),
            "P2-M005-T0001",
        ],
    );
    assert!(retired.status.success(), "{retired:?}");
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

fn agent_fixture(root: &Path) -> PathBuf {
    let _ = root;
    PathBuf::from(env!("CARGO_BIN_EXE_agentforge-cli-fixture"))
}

fn git(root: &Path, arguments: &[&str]) {
    let output = Command::new("git")
        .args(arguments)
        .current_dir(root)
        .output()
        .expect("git command");
    assert!(output.status.success(), "git {:?}: {:?}", arguments, output);
}
