#![allow(missing_docs)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static TEMPORARY_ROOT_COUNTER: AtomicU64 = AtomicU64::new(0);

#[test]
fn task_launch_prepares_and_runs_one_real_foreground_task() {
    let root = temporary_repo();
    let root_text = root.to_str().expect("root");
    assert!(forge(&root, &["init", root_text]).status.success());
    let created = forge(
        &root,
        &[
            "task",
            "create",
            root_text,
            "P2-M015-T0001",
            "P2-M015",
            "implementer",
            "pilot task",
            "--allowed",
            "README.md",
            "--capability",
            "run_local_commands",
        ],
    );
    assert!(created.status.success(), "{created:?}");

    let executable = env!("CARGO_BIN_EXE_agentforge-cli-fixture");
    let launched = forge(
        &root,
        &[
            "task",
            "launch",
            root_text,
            "P2-M015-T0001",
            executable,
            "--base",
            "HEAD",
        ],
    );
    assert!(launched.status.success(), "{launched:?}");
    let stdout = String::from_utf8_lossy(&launched.stdout);
    assert!(stdout.contains("task P2-M015-T0001 launched"), "{stdout}");
    assert!(stdout.contains("worktree-created=true"), "{stdout}");
    assert!(stdout.contains("recovery:"), "{stdout}");

    let worktree = root.join(".forge/worktrees/P2-M015-T0001");
    assert!(worktree.join("agentforge-fixture-output.txt").is_file());
    let hud = forge(&root, &["hud", root_text]);
    assert!(hud.status.success(), "{hud:?}");
    let hud_text = String::from_utf8_lossy(&hud.stdout);
    assert!(
        hud_text.contains("#1 WorktreeObserved task=P2-M015-T0001"),
        "{hud_text}"
    );

    let repeated = forge(
        &root,
        &["task", "launch", root_text, "P2-M015-T0001", executable],
    );
    assert!(!repeated.status.success(), "{repeated:?}");
    assert!(
        String::from_utf8_lossy(&repeated.stderr).contains("task is not ready"),
        "{}",
        String::from_utf8_lossy(&repeated.stderr)
    );

    fs::remove_file(worktree.join("agentforge-fixture-output.txt")).expect("fixture output");
    let retired = forge(&root, &["worktree", "retire", root_text, "P2-M015-T0001"]);
    assert!(retired.status.success(), "{retired:?}");
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn task_launch_reports_a_failing_agent_and_persists_its_output() {
    let root = temporary_repo();
    let root_text = root.to_str().expect("root");
    assert!(forge(&root, &["init", root_text]).status.success());
    let created = forge(
        &root,
        &[
            "task",
            "create",
            root_text,
            "P2-M029-T0001",
            "P2-M029",
            "implementer",
            "failing agent",
            "--allowed",
            "README.md",
            "--capability",
            "run_local_commands",
        ],
    );
    assert!(created.status.success(), "{created:?}");
    fs::create_dir_all(root.join(".forge/agents")).expect("agent directory");
    fs::write(
        root.join(".forge/agents/failing.conf"),
        format!(
            "version=1\nexecutable={}\nenv.AGENTFORGE_CLI_FIXTURE_MODE=fail\n",
            env!("CARGO_BIN_EXE_agentforge-cli-fixture")
        ),
    )
    .expect("agent profile");

    let launched = forge(
        &root,
        &[
            "task",
            "launch",
            root_text,
            "P2-M029-T0001",
            "--profile",
            "failing",
        ],
    );
    assert_eq!(launched.status.code(), Some(1), "{launched:?}");
    let stdout = String::from_utf8_lossy(&launched.stdout);
    let stderr = String::from_utf8_lossy(&launched.stderr);
    assert!(
        stdout.contains("agent-exit=3 termination=Exited"),
        "{stdout}"
    );
    assert!(
        stdout.contains("evidence stdout=.forge/evidence/P2-M029-T0001/"),
        "{stdout}"
    );
    assert!(stderr.contains("agent did not exit cleanly"), "{stderr}");
    assert!(
        stderr.contains("  | fixture failing by request"),
        "{stderr}"
    );
    let evidence = root.join(".forge/evidence/P2-M029-T0001");
    let logs = fs::read_dir(&evidence)
        .expect("evidence directory")
        .map(|entry| {
            entry
                .expect("entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect::<Vec<_>>();
    let stderr_log = logs
        .iter()
        .find(|name| name.ends_with("-stderr.log"))
        .expect("stderr log");
    assert_eq!(
        fs::read_to_string(evidence.join(stderr_log)).expect("stderr log"),
        "fixture failing by request\n"
    );
    let inspected = forge(&root, &["task", "inspect", root_text, "P2-M029-T0001"]);
    assert!(String::from_utf8_lossy(&inspected.stdout).contains("state=running"));
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn task_launch_rejects_missing_approval_without_creating_worktree() {
    let root = temporary_repo();
    let root_text = root.to_str().expect("root");
    assert!(forge(&root, &["init", root_text]).status.success());
    let created = forge(
        &root,
        &[
            "task",
            "create",
            root_text,
            "P2-M015-T0002",
            "P2-M015",
            "implementer",
            "approval task",
            "--capability",
            "run_local_commands",
            "--approval",
            "activate_implementation_plan",
        ],
    );
    assert!(created.status.success(), "{created:?}");
    let rejected = forge(
        &root,
        &[
            "task",
            "launch",
            root_text,
            "P2-M015-T0002",
            env!("CARGO_BIN_EXE_agentforge-cli-fixture"),
        ],
    );
    assert!(!rejected.status.success(), "{rejected:?}");
    assert!(
        String::from_utf8_lossy(&rejected.stderr).contains("missing approval"),
        "{}",
        String::from_utf8_lossy(&rejected.stderr)
    );
    assert!(!root.join(".forge/worktrees/P2-M015-T0002").exists());
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
    let root = std::env::temp_dir().join(format!(
        "agentforge-cli-task-launch-{}-{stamp}-{}",
        std::process::id(),
        TEMPORARY_ROOT_COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&root).expect("temporary root");
    git(&root, &["init", "-q"]);
    fs::write(root.join("README.md"), "fixture\n").expect("fixture");
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
    assert!(output.status.success(), "git {:?}: {output:?}", arguments);
}
