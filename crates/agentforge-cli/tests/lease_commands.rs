//! Lease lifecycle through the real `forge` binary (P4-M004).

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static TEMPORARY_ROOT_COUNTER: AtomicU64 = AtomicU64::new(0);

fn forge(root: &Path, arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_forge"))
        .args(arguments)
        .current_dir(root)
        .output()
        .expect("forge command")
}

fn git(root: &Path, arguments: &[&str]) {
    let output = Command::new("git")
        .args(arguments)
        .current_dir(root)
        .output()
        .expect("git command");
    assert!(output.status.success(), "git {arguments:?}: {output:?}");
}

fn temporary_repo() -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "agentforge-cli-lease-{}-{stamp}-{}",
        std::process::id(),
        TEMPORARY_ROOT_COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&root).expect("temporary root");
    git(&root, &["init", "-q"]);
    git(
        &root,
        &["config", "user.email", "agentforge@example.invalid"],
    );
    git(&root, &["config", "user.name", "AgentForge Test"]);
    fs::write(root.join("README.md"), "fixture\n").expect("fixture");
    fs::write(root.join(".gitignore"), ".forge/\n").expect("gitignore");
    git(&root, &["add", "README.md", ".gitignore"]);
    git(&root, &["commit", "-qm", "fixture"]);
    root
}

fn text(output: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

#[test]
fn lease_lifecycle_blocks_and_then_allows_a_local_launch() {
    let root = temporary_repo();
    let root_text = root.to_str().expect("root");
    assert!(forge(&root, &["init", root_text]).status.success());
    for (task, path) in [
        ("P4-M004-T0001", "agentforge-fixture-output.txt"),
        ("P4-M004-T0002", "other.txt"),
    ] {
        let created = forge(
            &root,
            &[
                "task",
                "create",
                root_text,
                task,
                "P4-M004",
                "implementer",
                "lease fixture",
                "--allowed",
                path,
                "--capability",
                "run_local_commands",
            ],
        );
        assert!(created.status.success(), "{created:?}");
    }

    let none = forge(&root, &["worker", "list", root_text]);
    assert!(text(&none).contains("no workers"), "{none:?}");
    let refused = forge(
        &root,
        &[
            "lease",
            "grant",
            root_text,
            "P4-M004-T0001",
            "--actor",
            "op",
        ],
    );
    assert!(!refused.status.success());
    assert!(
        text(&refused).contains("no workers are registered"),
        "{refused:?}"
    );

    fs::create_dir_all(root.join(".forge/workers")).expect("workers");
    fs::write(
        root.join(".forge/workers/builder-1.conf"),
        "platform=linux-x86_64\ncapability=rust\nmax_leases=2\n",
    )
    .expect("profile");
    let granted = forge(
        &root,
        &[
            "lease",
            "grant",
            root_text,
            "P4-M004-T0001",
            "--actor",
            "op",
        ],
    );
    assert!(granted.status.success(), "{granted:?}");
    assert!(
        text(&granted).contains("granted lease=P4-M004-T0001.L1 task=P4-M004-T0001 worker=builder-1 generation=1 state=active"),
        "{granted:?}"
    );
    let workers = forge(&root, &["worker", "list", root_text]);
    assert!(
        text(&workers).contains("worker id=builder-1 platform=linux-x86_64 capabilities=rust max_leases=2 active_leases=1"),
        "{workers:?}"
    );

    let executable = env!("CARGO_BIN_EXE_agentforge-cli-fixture");
    let blocked = forge(
        &root,
        &[
            "task",
            "launch",
            root_text,
            "P4-M004-T0001",
            executable,
            "--base",
            "HEAD",
        ],
    );
    assert!(!blocked.status.success(), "{blocked:?}");
    assert!(
        text(&blocked).contains("is leased to worker builder-1"),
        "{blocked:?}"
    );
    assert!(!root.join(".forge/worktrees/P4-M004-T0001").exists());

    let renewed = forge(
        &root,
        &[
            "lease",
            "renew",
            root_text,
            "P4-M004-T0001.L1",
            "--ttl-ms",
            "3600000",
            "--actor",
            "op",
        ],
    );
    assert!(renewed.status.success(), "{renewed:?}");
    let released = forge(
        &root,
        &[
            "lease",
            "release",
            root_text,
            "P4-M004-T0001.L1",
            "--actor",
            "op",
        ],
    );
    assert!(text(&released).contains("state=released"), "{released:?}");

    let launched = forge(
        &root,
        &[
            "task",
            "launch",
            root_text,
            "P4-M004-T0001",
            executable,
            "--base",
            "HEAD",
        ],
    );
    assert!(launched.status.success(), "{launched:?}");

    // A 1 ms lease is due almost at once; `expire` records it.
    let short = forge(
        &root,
        &[
            "lease",
            "grant",
            root_text,
            "P4-M004-T0002",
            "--ttl-ms",
            "1",
            "--actor",
            "op",
        ],
    );
    assert!(short.status.success(), "{short:?}");
    std::thread::sleep(std::time::Duration::from_millis(20));
    let expired = forge(&root, &["lease", "expire", root_text, "--actor", "op"]);
    assert!(text(&expired).contains("expired 1 lease(s)"), "{expired:?}");
    let listed = forge(&root, &["lease", "list", root_text]);
    let listed = text(&listed);
    assert!(
        listed.contains("lease=P4-M004-T0001.L1") && listed.contains("state=released"),
        "{listed}"
    );
    assert!(
        listed.contains("lease=P4-M004-T0002.L1") && listed.contains("state=expired"),
        "{listed}"
    );

    let hud = forge(&root, &["hud", root_text]);
    assert!(text(&hud).contains("LeaseRecorded"), "{hud:?}");

    let usage = forge(&root, &["lease", "grant", root_text, "P4-M004-T0002"]);
    assert_eq!(usage.status.code(), Some(2), "{usage:?}");
    assert!(text(&usage).contains("--actor is required"), "{usage:?}");

    fs::remove_file(root.join(".forge/worktrees/P4-M004-T0001/agentforge-fixture-output.txt"))
        .expect("fixture output");
    assert!(
        forge(&root, &["worktree", "retire", root_text, "P4-M004-T0001"])
            .status
            .success()
    );
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn a_worker_process_runs_its_leased_task() {
    let root = temporary_repo();
    let root_text = root.to_str().expect("root");
    assert!(forge(&root, &["init", root_text]).status.success());
    let created = forge(
        &root,
        &[
            "task",
            "create",
            root_text,
            "P4-M005-T0001",
            "P4-M005",
            "implementer",
            "worker fixture",
            "--allowed",
            "agentforge-fixture-output.txt",
            "--capability",
            "run_local_commands",
        ],
    );
    assert!(created.status.success(), "{created:?}");
    fs::create_dir_all(root.join(".forge/workers")).expect("workers");
    fs::write(
        root.join(".forge/workers/builder-1.conf"),
        "platform=linux-x86_64\ncapability=rust\nmax_leases=1\n",
    )
    .expect("profile");
    let executable = env!("CARGO_BIN_EXE_agentforge-cli-fixture");

    let idle = forge(
        &root,
        &[
            "worker",
            "run",
            root_text,
            "builder-1",
            executable,
            "--once",
        ],
    );
    assert!(idle.status.success(), "{idle:?}");
    assert!(text(&idle).contains("no claimable leases"), "{idle:?}");

    let granted = forge(
        &root,
        &[
            "lease",
            "grant",
            root_text,
            "P4-M005-T0001",
            "--actor",
            "op",
        ],
    );
    assert!(granted.status.success(), "{granted:?}");
    let ran = forge(
        &root,
        &[
            "worker",
            "run",
            root_text,
            "builder-1",
            executable,
            "--once",
        ],
    );
    let output = text(&ran);
    assert!(ran.status.success(), "{output}");
    for expected in [
        "worker builder-1 claimed lease=P4-M005-T0001.L1 task=P4-M005-T0001 generation=1",
        "agent-exit=0",
        "worker builder-1 released lease=P4-M005-T0001.L1 state=released",
        "tasks_run=1 failures=0",
    ] {
        assert!(
            output.contains(expected),
            "missing {expected:?} in {output}"
        );
    }
    assert!(
        root.join(".forge/worktrees/P4-M005-T0001/agentforge-fixture-output.txt")
            .is_file()
    );
    let inspected = forge(&root, &["task", "inspect", root_text, "P4-M005-T0001"]);
    assert!(text(&inspected).contains("state=running"), "{inspected:?}");

    let unknown = forge(
        &root,
        &[
            "worker",
            "run",
            root_text,
            "builder-9",
            executable,
            "--once",
        ],
    );
    assert!(!unknown.status.success());
    assert!(text(&unknown).contains("not registered"), "{unknown:?}");

    fs::remove_file(root.join(".forge/worktrees/P4-M005-T0001/agentforge-fixture-output.txt"))
        .expect("fixture output");
    assert!(
        forge(&root, &["worktree", "retire", root_text, "P4-M005-T0001"])
            .status
            .success()
    );
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn dispatch_then_a_worker_runs_the_task_without_a_manual_grant() {
    let root = temporary_repo();
    let root_text = root.to_str().expect("root");
    assert!(forge(&root, &["init", root_text]).status.success());
    let created = forge(
        &root,
        &[
            "task",
            "create",
            root_text,
            "P4-M006-T0001",
            "P4-M006",
            "implementer",
            "dispatch fixture",
            "--allowed",
            "agentforge-fixture-output.txt",
            "--capability",
            "run_local_commands",
        ],
    );
    assert!(created.status.success(), "{created:?}");
    fs::create_dir_all(root.join(".forge/workers")).expect("workers");
    fs::write(
        root.join(".forge/workers/builder-1.conf"),
        "platform=linux-x86_64\ncapability=rust\nmax_leases=1\n",
    )
    .expect("profile");

    let disabled = forge(&root, &["lease", "dispatch", root_text, "--actor", "op"]);
    assert!(disabled.status.success(), "{disabled:?}");
    assert!(
        text(&disabled).contains("dispatch is disabled"),
        "{disabled:?}"
    );

    fs::write(
        root.join(".forge/dispatch.conf"),
        "enabled=true\nmilestone=P4-M006\n",
    )
    .expect("policy");
    let dispatched = forge(&root, &["lease", "dispatch", root_text, "--actor", "op"]);
    let output = text(&dispatched);
    assert!(dispatched.status.success(), "{output}");
    assert!(
        output.contains("dispatched lease=P4-M006-T0001.L1 task=P4-M006-T0001 worker=builder-1"),
        "{output}"
    );
    assert!(output.contains("dispatched 1 task(s)"), "{output}");

    let executable = env!("CARGO_BIN_EXE_agentforge-cli-fixture");
    let ran = forge(
        &root,
        &[
            "worker",
            "run",
            root_text,
            "builder-1",
            executable,
            "--once",
        ],
    );
    assert!(ran.status.success(), "{}", text(&ran));
    assert!(
        text(&ran).contains("tasks_run=1 failures=0"),
        "{}",
        text(&ran)
    );

    // Dispatch-once: nothing further to do.
    let again = forge(&root, &["lease", "dispatch", root_text, "--actor", "op"]);
    assert!(text(&again).contains("dispatched 0 task(s)"), "{again:?}");

    fs::remove_file(root.join(".forge/worktrees/P4-M006-T0001/agentforge-fixture-output.txt"))
        .expect("fixture output");
    assert!(
        forge(&root, &["worktree", "retire", root_text, "P4-M006-T0001"])
            .status
            .success()
    );
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn a_remote_worker_enrolls_and_claims_through_the_worker_api() {
    let root = temporary_repo();
    let root_text = root.to_str().expect("root");
    assert!(forge(&root, &["init", root_text]).status.success());
    let created = forge(
        &root,
        &[
            "task",
            "create",
            root_text,
            "P4-M007-T0001",
            "P4-M007",
            "implementer",
            "remote fixture",
            "--allowed",
            "src",
            "--capability",
            "run_local_commands",
        ],
    );
    assert!(created.status.success(), "{created:?}");
    fs::create_dir_all(root.join(".forge/workers")).expect("workers");
    fs::write(
        root.join(".forge/workers/remote-1.conf"),
        "platform=linux-x86_64\ncapability=rust\nmax_leases=1\n",
    )
    .expect("profile");

    let enrolled = forge(&root, &["worker", "enroll", root_text, "remote-1"]);
    let output = text(&enrolled);
    assert!(enrolled.status.success(), "{output}");
    let secret = output
        .lines()
        .find_map(|line| line.strip_prefix("secret="))
        .expect("secret line")
        .to_owned();
    assert_eq!(secret.len(), 64);
    let again = forge(&root, &["worker", "enroll", root_text, "remote-1"]);
    assert!(!again.status.success() && text(&again).contains("already enrolled"));

    // The worker host keeps its own copy of the secret.
    let secret_file = root.join("remote-1.secret");
    fs::write(&secret_file, format!("{secret}\n")).expect("secret file");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&secret_file, fs::Permissions::from_mode(0o600)).expect("chmod");
    }
    let server = agentforge_daemon::worker_api::WorkerApiServer::start(
        &root,
        "127.0.0.1:0".parse().expect("addr"),
    )
    .expect("worker api");
    let endpoint = server.address.to_string();
    let secret_path = secret_file.to_str().expect("path").to_owned();
    let remote = |action: &str, extra: &[&str]| {
        let mut arguments = vec![
            "worker",
            "remote",
            action,
            "--endpoint",
            &endpoint,
            "--worker",
            "remote-1",
            "--secret-file",
            &secret_path,
        ];
        arguments.extend_from_slice(extra);
        forge(&root, &arguments)
    };

    let idle = remote("claim", &[]);
    assert!(text(&idle).contains("no claimable leases"), "{idle:?}");
    assert!(
        forge(
            &root,
            &[
                "lease",
                "grant",
                root_text,
                "P4-M007-T0001",
                "--actor",
                "op"
            ]
        )
        .status
        .success()
    );
    let contract = root.join("contract.txt");
    let claimed = remote(
        "claim",
        &["--contract-out", contract.to_str().expect("path")],
    );
    let output = text(&claimed);
    assert!(claimed.status.success(), "{output}");
    assert!(
        output.contains("claimed lease=P4-M007-T0001.L1 task=P4-M007-T0001 generation=1"),
        "{output}"
    );
    assert!(
        fs::read(&contract)
            .expect("contract")
            .starts_with(b"agentforge-task-prompt-v1\n")
    );
    let renewed = remote(
        "renew",
        &["--lease", "P4-M007-T0001.L1", "--ttl-ms", "3600000"],
    );
    assert!(renewed.status.success(), "{renewed:?}");
    let released = remote("release", &["--lease", "P4-M007-T0001.L1"]);
    assert!(
        text(&released).contains("released lease=P4-M007-T0001.L1"),
        "{released:?}"
    );

    fs::write(&secret_file, format!("{}\n", "f".repeat(64))).expect("wrong secret");
    let refused = remote("claim", &[]);
    assert!(!refused.status.success());
    assert!(text(&refused).contains("unauthorized"), "{refused:?}");
    drop(server);
    fs::remove_dir_all(root).expect("cleanup");
}
