#![allow(missing_docs)]

use agentforge_daemon::{DEFAULT_BIND, DaemonError, serve, status};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

static TEMPORARY_ROOT_COUNTER: AtomicU64 = AtomicU64::new(0);
// Bounded like the daemon crate's own lifecycle tests (P2-M023): slow runners must
// never turn a readiness miss into an unbounded join on a live daemon.
const READY_TIMEOUT: Duration = Duration::from_secs(30);
const EXIT_TIMEOUT: Duration = Duration::from_secs(30);

#[test]
fn daemon_status_and_stop_are_operator_commands() {
    let root = temporary_repo();
    let Some(server) = start_foreground(&root) else {
        fs::remove_dir_all(root).expect("cleanup");
        return;
    };

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

    let created_launch = forge(
        &root,
        &[
            "task",
            "create",
            root.to_str().expect("root"),
            "P2-M016-T0001",
            "P2-M016",
            "implementer",
            "exercise daemon launch",
            "--allowed",
            "agentforge-fixture-output.txt",
            "--capability",
            "write_owned_paths",
            "--capability",
            "run_local_commands",
        ],
    );
    assert!(created_launch.status.success(), "{created_launch:?}");
    let launched = forge(
        &root,
        &[
            "daemon",
            "launch",
            root.to_str().expect("root"),
            "P2-M016-T0001",
            executable.to_str().expect("fixture path"),
            "--base",
            "HEAD",
        ],
    );
    assert!(launched.status.success(), "{launched:?}");
    let launch_stdout = String::from_utf8_lossy(&launched.stdout);
    assert!(
        launch_stdout.contains("task=P2-M016-T0001"),
        "{launch_stdout}"
    );
    assert!(
        launch_stdout.contains("worktree-created=true"),
        "{launch_stdout}"
    );
    assert!(
        root.join(".forge/worktrees/P2-M016-T0001/agentforge-fixture-output.txt")
            .is_file()
    );

    fs::create_dir_all(root.join(".forge/agents")).expect("agent profile directory");
    fs::write(
        root.join(".forge/agents/fixture.conf"),
        format!(
            "version=1\nexecutable={}\n",
            executable.to_str().expect("fixture path")
        ),
    )
    .expect("agent profile");
    let created_profile_launch = forge(
        &root,
        &[
            "task",
            "create",
            root.to_str().expect("root"),
            "P2-M016-T0002",
            "P2-M016",
            "implementer",
            "exercise daemon profile launch",
            "--allowed",
            "agentforge-fixture-output.txt",
            "--capability",
            "write_owned_paths",
            "--capability",
            "run_local_commands",
        ],
    );
    assert!(
        created_profile_launch.status.success(),
        "{created_profile_launch:?}"
    );
    let profile_launched = forge(
        &root,
        &[
            "daemon",
            "launch",
            root.to_str().expect("root"),
            "P2-M016-T0002",
            "--profile",
            "fixture",
            "--base",
            "HEAD",
        ],
    );
    assert!(profile_launched.status.success(), "{profile_launched:?}");
    assert!(String::from_utf8_lossy(&profile_launched.stdout).contains("task=P2-M016-T0002"));
    assert!(
        root.join(".forge/worktrees/P2-M016-T0002/agentforge-fixture-output.txt")
            .is_file()
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
    let accepted_launch = forge(
        &root,
        &[
            "task",
            "accept",
            root.to_str().expect("root"),
            "P2-M016-T0001",
            "--actor",
            "operator",
        ],
    );
    assert!(accepted_launch.status.success(), "{accepted_launch:?}");
    fs::remove_file(root.join(".forge/worktrees/P2-M016-T0001/agentforge-fixture-output.txt"))
        .expect("remove launch fixture output before retirement");
    let accepted_profile_launch = forge(
        &root,
        &[
            "task",
            "accept",
            root.to_str().expect("root"),
            "P2-M016-T0002",
            "--actor",
            "operator",
        ],
    );
    assert!(
        accepted_profile_launch.status.success(),
        "{accepted_profile_launch:?}"
    );
    fs::remove_file(root.join(".forge/worktrees/P2-M016-T0002/agentforge-fixture-output.txt"))
        .expect("remove profile fixture output before retirement");

    let status_output = forge(&root, &["daemon", "status", root.to_str().expect("root")]);
    assert!(status_output.status.success(), "{status_output:?}");
    assert!(String::from_utf8_lossy(&status_output.stdout).contains("daemon running at"));
    let stop_output = forge(&root, &["daemon", "stop", root.to_str().expect("root")]);
    assert!(stop_output.status.success(), "{stop_output:?}");
    assert!(wait_for_exit(&server).is_ok());
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
    let retired_launch = forge(
        &root,
        &[
            "worktree",
            "retire",
            root.to_str().expect("root"),
            "P2-M016-T0001",
        ],
    );
    assert!(retired_launch.status.success(), "{retired_launch:?}");
    let retired_profile_launch = forge(
        &root,
        &[
            "worktree",
            "retire",
            root.to_str().expect("root"),
            "P2-M016-T0002",
        ],
    );
    assert!(
        retired_profile_launch.status.success(),
        "{retired_profile_launch:?}"
    );
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn long_daemon_executions_succeed_while_status_stays_available() {
    let root = temporary_repo();
    let root_text = root.to_str().expect("root").to_owned();
    let Some(server) = start_foreground(&root) else {
        fs::remove_dir_all(root).expect("cleanup");
        return;
    };
    assert!(forge(&root, &["init", &root_text]).status.success());
    for id in ["P2-M024-T0001", "P2-M024-T0002"] {
        let created = forge(
            &root,
            &[
                "task",
                "create",
                &root_text,
                id,
                "P2-M024",
                "implementer",
                "long daemon execution",
                "--allowed",
                "notes",
                "--capability",
                "run_local_commands",
            ],
        );
        assert!(created.status.success(), "{created:?}");
    }
    // Three seconds is longer than the two-second control-request timeout that
    // used to apply to executions.
    fs::create_dir_all(root.join(".forge/agents")).expect("agent profile directory");
    fs::write(
        root.join(".forge/agents/slow.conf"),
        format!(
            "version=1\nexecutable={}\nenv.AGENTFORGE_CLI_FIXTURE_MODE=sleep\nenv.AGENTFORGE_FIXTURE_SLEEP_MS=3000\n",
            env!("CARGO_BIN_EXE_agentforge-cli-fixture")
        ),
    )
    .expect("agent profile");

    let launch_root = root.clone();
    let launch_text = root_text.clone();
    let launch = thread::spawn(move || {
        forge(
            &launch_root,
            &[
                "daemon",
                "launch",
                &launch_text,
                "P2-M024-T0001",
                "--profile",
                "slow",
            ],
        )
    });
    // The worktree is created before the agent starts, and the single-task path persists the
    // task state only after the agent finishes, so the worktree is the in-progress signal.
    wait_for_path(&root.join(".forge/worktrees/P2-M024-T0001"));

    let during = forge(&root, &["daemon", "status", &root_text]);
    assert!(
        during.status.success(),
        "status during execution: {during:?}"
    );

    let overlapping = forge(
        &root,
        &[
            "daemon",
            "launch",
            &root_text,
            "P2-M024-T0002",
            "--profile",
            "slow",
        ],
    );
    assert!(!overlapping.status.success(), "{overlapping:?}");
    let overlapping_stderr = String::from_utf8_lossy(&overlapping.stderr);
    assert!(
        overlapping_stderr.contains("busy") && overlapping_stderr.contains("P2-M024-T0001"),
        "{overlapping_stderr}"
    );
    assert!(
        !overlapping_stderr.contains("stale"),
        "{overlapping_stderr}"
    );

    let early_stop = forge(&root, &["daemon", "stop", &root_text]);
    assert!(!early_stop.status.success(), "{early_stop:?}");
    assert!(
        String::from_utf8_lossy(&early_stop.stderr).contains("P2-M024-T0001"),
        "{}",
        String::from_utf8_lossy(&early_stop.stderr)
    );

    let launched = launch.join().expect("launch thread");
    assert!(launched.status.success(), "{launched:?}");
    assert!(
        String::from_utf8_lossy(&launched.stdout).contains("task=P2-M024-T0001"),
        "{}",
        String::from_utf8_lossy(&launched.stdout)
    );
    assert_eq!(state(&root, "P2-M024-T0002"), "pending");

    let stopped = forge(&root, &["daemon", "stop", &root_text]);
    assert!(stopped.status.success(), "{stopped:?}");
    assert!(wait_for_exit(&server).is_ok());
    fs::remove_dir_all(root).expect("cleanup");
}

type ServerResult = Result<(), DaemonError>;

/// Starts a foreground daemon and waits, with a deadline, for its endpoint. Returns `None`
/// only when the platform denies the loopback bind.
fn start_foreground(root: &Path) -> Option<Receiver<ServerResult>> {
    let (sender, receiver) = mpsc::channel();
    let server_root = root.to_path_buf();
    thread::spawn(move || {
        let _ = sender.send(serve(server_root, DEFAULT_BIND));
    });
    let deadline = Instant::now() + READY_TIMEOUT;
    loop {
        match status(root) {
            Ok(_) => return Some(receiver),
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

fn wait_for_exit(server: &Receiver<ServerResult>) -> ServerResult {
    server.recv_timeout(EXIT_TIMEOUT).unwrap_or_else(|error| {
        panic!("daemon thread did not exit within {EXIT_TIMEOUT:?}: {error}")
    })
}

fn state(root: &Path, id: &str) -> String {
    let inspected = forge(root, &["task", "inspect", root.to_str().expect("root"), id]);
    let stdout = String::from_utf8_lossy(&inspected.stdout);
    stdout
        .split_whitespace()
        .find_map(|field| field.strip_prefix("state="))
        .unwrap_or("unknown")
        .to_owned()
}

fn wait_for_path(path: &Path) {
    let deadline = Instant::now() + READY_TIMEOUT;
    while !path.exists() {
        assert!(
            Instant::now() < deadline,
            "{} did not appear within {READY_TIMEOUT:?}",
            path.display()
        );
        thread::sleep(Duration::from_millis(20));
    }
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
    let root = std::env::temp_dir().join(format!(
        "agentforge-cli-daemon-{}-{stamp}-{}",
        std::process::id(),
        TEMPORARY_ROOT_COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
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
