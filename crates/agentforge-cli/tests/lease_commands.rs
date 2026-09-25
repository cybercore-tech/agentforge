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

// `forge worker enroll` needs /dev/urandom; the worker API itself is covered portably in the
// daemon crate's tests.
#[cfg(unix)]
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

/// Runs `git` in `directory` and returns trimmed stdout.
#[cfg(unix)]
fn git_out(directory: &Path, arguments: &[&str]) -> String {
    let output = Command::new("git")
        .args(arguments)
        .current_dir(directory)
        .output()
        .expect("git");
    assert!(output.status.success(), "git {arguments:?}: {output:?}");
    String::from_utf8(output.stdout)
        .expect("utf8")
        .trim()
        .to_owned()
}

/// A coordinator with one task and an enrolled worker, a running worker API, and a clone for the
/// "remote" host. Returns (coordinator, clone, endpoint, secret file, server).
#[cfg(unix)]
fn remote_setup(
    allowed: &str,
) -> (
    PathBuf,
    PathBuf,
    String,
    PathBuf,
    agentforge_daemon::worker_api::WorkerApiServer,
) {
    use std::os::unix::fs::PermissionsExt;
    let root = temporary_repo();
    let root_text = root.to_str().expect("root");
    assert!(forge(&root, &["init", root_text]).status.success());
    let created = forge(
        &root,
        &[
            "task",
            "create",
            root_text,
            "P4-M008-T0001",
            "P4-M008",
            "implementer",
            "remote execution fixture",
            "--allowed",
            allowed,
            "--capability",
            "run_local_commands",
            "--capability",
            "merge_protected_branch",
            "--approval",
            "merge_protected_branch",
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
    let secret = text(&enrolled)
        .lines()
        .find_map(|line| line.strip_prefix("secret=").map(str::to_owned))
        .expect("secret");
    let host = root.with_extension("remote-host");
    let _ = fs::remove_dir_all(&host);
    fs::create_dir_all(&host).expect("host");
    let secret_file = host.join("remote-1.secret");
    fs::write(&secret_file, format!("{secret}\n")).expect("secret");
    fs::set_permissions(&secret_file, fs::Permissions::from_mode(0o600)).expect("chmod");
    git_out(&host, &["clone", "-q", root_text, "clone"]);
    let clone = host.join("clone");
    git_out(&clone, &["config", "user.name", "Remote Worker"]);
    git_out(&clone, &["config", "user.email", "remote@example.invalid"]);
    let server = agentforge_daemon::worker_api::WorkerApiServer::start(
        &root,
        "127.0.0.1:0".parse().expect("addr"),
    )
    .expect("worker api");
    let endpoint = server.address.to_string();
    assert!(
        forge(
            &root,
            &[
                "lease",
                "grant",
                root_text,
                "P4-M008-T0001",
                "--actor",
                "op"
            ]
        )
        .status
        .success()
    );
    (root, clone, endpoint, secret_file, server)
}

#[cfg(unix)]
fn remote_run(root: &Path, clone: &Path, endpoint: &str, secret: &Path) -> Output {
    forge(
        root,
        &[
            "worker",
            "remote",
            "run",
            "--endpoint",
            endpoint,
            "--worker",
            "remote-1",
            "--secret-file",
            secret.to_str().expect("secret"),
            "--repo",
            clone.to_str().expect("clone"),
            "--executable",
            env!("CARGO_BIN_EXE_agentforge-cli-fixture"),
            "--once",
        ],
    )
}

#[cfg(unix)]
#[test]
fn a_remote_worker_runs_its_task_and_the_result_lands_through_review() {
    let (root, clone, endpoint, secret, server) = remote_setup("agentforge-fixture-output.txt");
    let root_text = root.to_str().expect("root");
    let ran = remote_run(&root, &clone, &endpoint, &secret);
    let output = text(&ran);
    assert!(ran.status.success(), "{output}");
    assert!(
        output.contains("worker remote-1 claimed lease=P4-M008-T0001.L1"),
        "{output}"
    );
    assert!(output.contains("agent-exit=0"), "{output}");
    assert!(
        output.contains("imported state=running gates=0/0"),
        "{output}"
    );
    assert!(
        output.contains("claimed=1 imported=1 abandoned=0"),
        "{output}"
    );
    let head = output
        .lines()
        .find_map(|line| line.strip_prefix("result head="))
        .expect("head")
        .to_owned();

    // The coordinator holds exactly the reported commit, and the lease is released.
    assert_eq!(
        git_out(&root, &["rev-parse", "agentforge/task/P4-M008-T0001"]),
        head
    );
    assert!(text(&forge(&root, &["lease", "list", root_text])).contains("state=released"));
    let inspected = text(&forge(
        &root,
        &["task", "inspect", root_text, "P4-M008-T0001"],
    ));
    assert!(inspected.contains("state=running"), "{inspected}");

    // Normal review: diff, accept, approve (bound to the imported SHA), integrate.
    let diff = text(&forge(&root, &["task", "diff", root_text, "P4-M008-T0001"]));
    assert!(diff.contains("agentforge-fixture-output.txt"), "{diff}");
    for command in [
        vec![
            "task",
            "accept",
            root_text,
            "P4-M008-T0001",
            "--actor",
            "op",
        ],
        vec![
            "task",
            "approve",
            root_text,
            "P4-M008-T0001",
            "merge_protected_branch",
            "--actor",
            "op",
        ],
    ] {
        let output = forge(&root, &command);
        assert!(output.status.success(), "{output:?}");
    }
    let branch = git_out(&root, &["rev-parse", "--abbrev-ref", "HEAD"]);
    let integrated = forge(
        &root,
        &[
            "task",
            "integrate",
            root_text,
            "P4-M008-T0001",
            "--target",
            &branch,
            "--actor",
            "op",
        ],
    );
    assert!(integrated.status.success(), "{integrated:?}");
    assert_eq!(git_out(&root, &["rev-parse", "HEAD"]), head);
    // The worker archived its attempt under the lease, freeing the task's names (finding 17).
    assert_eq!(
        git_out(
            &clone,
            &[
                "for-each-ref",
                "--format=%(refname)",
                "refs/heads/agentforge"
            ]
        ),
        "refs/heads/agentforge/remote/P4-M008-T0001.L1"
    );
    assert_eq!(
        git_out(&clone, &["rev-parse", "agentforge/remote/P4-M008-T0001.L1"]),
        head
    );
    drop(server);
    let _ = fs::remove_dir_all(clone.parent().expect("host"));
    fs::remove_dir_all(root).expect("cleanup");
}

#[cfg(unix)]
#[test]
fn a_remote_worker_that_writes_out_of_bounds_sends_nothing() {
    let (root, clone, endpoint, secret, server) = remote_setup("src");
    let root_text = root.to_str().expect("root");
    let ran = remote_run(&root, &clone, &endpoint, &secret);
    let output = text(&ran);
    assert!(!ran.status.success(), "{output}");
    assert!(
        output.contains("agentforge-fixture-output.txt is outside the task's allowed paths"),
        "{output}"
    );
    assert!(output.contains("lease released"), "{output}");
    assert!(text(&forge(&root, &["lease", "list", root_text])).contains("state=released"));
    let inspected = text(&forge(
        &root,
        &["task", "inspect", root_text, "P4-M008-T0001"],
    ));
    assert!(inspected.contains("state=pending"), "{inspected}");
    let refs = git_out(
        &root,
        &["for-each-ref", "refs/heads/agentforge", "refs/agentforge"],
    );
    assert_eq!(refs, "", "nothing reached the coordinator");

    // Leased again to the same host, the task fails for the same reason, not on the first
    // attempt's leftovers (finding 17). Each attempt is archived under its own lease.
    let granted = forge(
        &root,
        &[
            "lease",
            "grant",
            root_text,
            "P4-M008-T0001",
            "--actor",
            "op",
        ],
    );
    assert!(granted.status.success(), "{granted:?}");
    let again = text(&remote_run(&root, &clone, &endpoint, &secret));
    assert!(
        again.contains("agentforge-fixture-output.txt is outside the task's allowed paths"),
        "{again}"
    );
    assert!(!again.contains("already exists"), "{again}");
    assert_eq!(
        git_out(
            &clone,
            &[
                "for-each-ref",
                "--format=%(refname)",
                "refs/heads/agentforge"
            ]
        ),
        "refs/heads/agentforge/remote/P4-M008-T0001.L1\n\
         refs/heads/agentforge/remote/P4-M008-T0001.L2"
    );
    for lease in ["P4-M008-T0001.L1", "P4-M008-T0001.L2"] {
        let kept = clone.join(".forge/remote-abandoned").join(lease);
        assert!(
            kept.join("agentforge-fixture-output.txt").is_file(),
            "the dirty attempt is kept for inspection at {}",
            kept.display()
        );
    }
    assert!(!clone.join(".forge/worktrees/P4-M008-T0001").exists());

    // A worker killed mid-task leaves a dirty worktree on the task's names. The next claim
    // archives it as `<lease>.stale` before starting (finding 17).
    let managed = clone.join(".forge/worktrees/P4-M008-T0001");
    git_out(
        &clone,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "agentforge/task/P4-M008-T0001",
            managed.to_str().expect("path"),
            "HEAD",
        ],
    );
    fs::write(managed.join("killed.txt"), "interrupted work\n").expect("leftover");
    let granted = forge(
        &root,
        &[
            "lease",
            "grant",
            root_text,
            "P4-M008-T0001",
            "--actor",
            "op",
        ],
    );
    assert!(granted.status.success(), "{granted:?}");
    let third = text(&remote_run(&root, &clone, &endpoint, &secret));
    assert!(
        third.contains("agentforge-fixture-output.txt is outside the task's allowed paths"),
        "{third}"
    );
    let stale = clone.join(".forge/remote-abandoned/P4-M008-T0001.L3.stale");
    assert!(stale.join("killed.txt").is_file(), "{third}");
    assert!(
        git_out(
            &clone,
            &[
                "for-each-ref",
                "--format=%(refname)",
                "refs/heads/agentforge"
            ]
        )
        .ends_with(
            "refs/heads/agentforge/remote/P4-M008-T0001.L3\n\
             refs/heads/agentforge/remote/P4-M008-T0001.L3.stale"
        )
    );
    drop(server);
    let _ = fs::remove_dir_all(clone.parent().expect("host"));
    fs::remove_dir_all(root).expect("cleanup");
}

#[cfg(unix)]
#[test]
fn the_doctor_reports_the_host_and_run_refuses_a_failing_one() {
    let (root, clone, endpoint, secret, server) = remote_setup("agentforge-fixture-output.txt");
    let root_text = root.to_str().expect("root");
    let secret_text = secret.to_str().expect("secret");
    let clone_text = clone.to_str().expect("clone");
    let doctor = forge(
        &root,
        &[
            "worker",
            "remote",
            "doctor",
            "--endpoint",
            &endpoint,
            "--worker",
            "remote-1",
            "--secret-file",
            secret_text,
            "--repo",
            clone_text,
            "--executable",
            env!("CARGO_BIN_EXE_agentforge-cli-fixture"),
        ],
    );
    let output = text(&doctor);
    assert!(doctor.status.success(), "{output}");
    for expected in [
        "doctor ok repo:",
        "doctor ok git-identity: Remote Worker <remote@example.invalid>",
        // This fixture project tracks no hooks, so the doctor only warns.
        "doctor warn hooks:",
        "doctor ok agent:",
        "doctor ok secret:",
        "doctor ok endpoint:",
        "0 failing check(s)",
    ] {
        assert!(
            output.contains(expected),
            "missing {expected:?} in {output}"
        );
    }

    let refused = forge(
        &root,
        &[
            "worker",
            "remote",
            "run",
            "--endpoint",
            &endpoint,
            "--worker",
            "remote-1",
            "--secret-file",
            secret_text,
            "--repo",
            clone_text,
            "--executable",
            "/nonexistent/agent",
            "--once",
        ],
    );
    let output = text(&refused);
    assert!(!refused.status.success(), "{output}");
    assert!(output.contains("doctor fail agent:"), "{output}");
    assert!(output.contains("worker remote run refused"), "{output}");
    // Nothing was claimed: the lease is still active and the task pending.
    let leases = text(&forge(&root, &["lease", "list", root_text]));
    assert!(leases.contains("state=active"), "{leases}");
    let inspected = text(&forge(
        &root,
        &["task", "inspect", root_text, "P4-M008-T0001"],
    ));
    assert!(inspected.contains("state=pending"), "{inspected}");
    drop(server);
    let _ = fs::remove_dir_all(clone.parent().expect("host"));
    fs::remove_dir_all(root).expect("cleanup");
}
