//! Integration tests for local-process adapter preflight and supervision.

use agentforge_adapter::{
    AdapterError, AdapterRequest, AgentAdapter, ExecutionTermination, ProcessAdapter,
    ProcessAdapterConfig,
};
use agentforge_core::agent::{AgentRole, AgentTask, ApprovalBoundary, Capability};
use agentforge_core::task::TaskId;
use agentforge_worktree::{WorktreeManager, WorktreeSpec};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

static TEST_SEQUENCE: AtomicU64 = AtomicU64::new(0);

struct TestRepository {
    root: PathBuf,
}

impl TestRepository {
    fn new() -> Self {
        let sequence = TEST_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "agentforge-adapter-test-{}-{sequence}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        run(&root, &["init", "-b", "main"]);
        run(&root, &["config", "user.name", "AgentForge Test"]);
        run(
            &root,
            &["config", "user.email", "agentforge@example.invalid"],
        );
        fs::write(root.join("README.md"), "fixture\n").unwrap();
        run(&root, &["add", "README.md"]);
        run(&root, &["commit", "-m", "initial"]);
        Self { root }
    }
    fn manager(&self) -> WorktreeManager {
        WorktreeManager::new(&self.root).unwrap()
    }
}

impl Drop for TestRepository {
    fn drop(&mut self) {
        let _ = Command::new("git")
            .current_dir(&self.root)
            .env_remove("GIT_INDEX_FILE")
            .args(["worktree", "prune"])
            .status();
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn run(root: &Path, args: &[&str]) {
    let output = Command::new("git")
        .current_dir(root)
        .env_remove("GIT_INDEX_FILE")
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {} failed: {}",
        args.join(" "),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn task() -> AgentTask {
    let mut task = AgentTask::new(
        "P0-M006-T0001",
        "P0-M006",
        AgentRole::Implementer,
        "write café\nwithout shell parsing",
    );
    task.capabilities.push(Capability::RunLocalCommands);
    task.allowed_paths.push("crates/example path".to_owned());
    task.non_goals.push("do not use $HOME".to_owned());
    task
}

fn created_worktree(repo: &TestRepository, task: &AgentTask) -> WorktreeManager {
    let manager = repo.manager();
    let task_id = TaskId::parse(task.task_id.clone()).unwrap();
    manager.create(&WorktreeSpec::new(task_id, "main")).unwrap();
    manager
}

fn adapter(mode: &str) -> ProcessAdapter {
    ProcessAdapter::new(
        ProcessAdapterConfig::new(
            "fixture",
            PathBuf::from(env!("CARGO_BIN_EXE_agentforge-adapter-fixture")),
        )
        .with_argument("literal ; $(not-a-command)")
        .with_environment("AGENTFORGE_FIXTURE_MODE", mode)
        .with_environment("AGENTFORGE_TEST_VALUE", "configured")
        .with_timeout(Duration::from_millis(250))
        .with_max_output_bytes(4096),
    )
    .unwrap()
}

#[test]
fn process_adapter_runs_in_verified_worktree_with_explicit_environment() {
    let repo = TestRepository::new();
    let task = task();
    let manager = created_worktree(&repo, &task);
    let report = adapter("echo")
        .execute(AdapterRequest {
            task: &task,
            worktrees: &manager,
            acknowledged_approvals: &[],
        })
        .unwrap();
    assert_eq!(report.termination(), ExecutionTermination::Exited);
    assert_eq!(report.exit_code(), Some(0));
    let output = String::from_utf8_lossy(report.stdout());
    assert!(output.contains(&format!("cwd={}", report.worktree_path().display())));
    assert!(output.contains("value=configured"));
    assert!(output.contains("argument=literal ; $(not-a-command)"));
    assert!(output.contains("git_index=\n"));
    assert!(output.contains("write café\nwithout shell parsing"));
    assert_eq!(report.stderr(), b"fixture stderr\n");
}

#[test]
fn preflight_rejects_missing_capability_and_approval_before_spawn() {
    let repo = TestRepository::new();
    let mut task = task();
    task.capabilities.clear();
    task.required_approvals
        .push(ApprovalBoundary::ActivateImplementationPlan);
    let manager = repo.manager();
    assert!(matches!(
        adapter("echo").execute(AdapterRequest {
            task: &task,
            worktrees: &manager,
            acknowledged_approvals: &[]
        }),
        Err(AdapterError::MissingCapability(
            Capability::RunLocalCommands
        ))
    ));
    task.capabilities.push(Capability::RunLocalCommands);
    assert!(matches!(
        adapter("echo").execute(AdapterRequest {
            task: &task,
            worktrees: &manager,
            acknowledged_approvals: &[]
        }),
        Err(AdapterError::MissingApproval(
            ApprovalBoundary::ActivateImplementationPlan
        ))
    ));

    // Post-execution approvals are recorded after review and never gate the agent (P1-M008).
    task.required_approvals = vec![
        ApprovalBoundary::MergeProtectedBranch,
        ApprovalBoundary::PublishRelease,
        ApprovalBoundary::DeployProduction,
    ];
    assert!(matches!(
        adapter("echo").execute(AdapterRequest {
            task: &task,
            worktrees: &manager,
            acknowledged_approvals: &[]
        }),
        Err(AdapterError::WorktreeNotFound(_))
    ));

    task.required_approvals.clear();
    task.task_id = "P0-M006-T0000".to_owned();
    assert!(matches!(
        adapter("echo").execute(AdapterRequest {
            task: &task,
            worktrees: &manager,
            acknowledged_approvals: &[]
        }),
        Err(AdapterError::TaskMilestoneMismatch { .. })
    ));
}

#[test]
fn dirty_or_missing_worktrees_are_never_started() {
    let repo = TestRepository::new();
    let task = task();
    let manager = repo.manager();
    assert!(matches!(
        adapter("echo").execute(AdapterRequest {
            task: &task,
            worktrees: &manager,
            acknowledged_approvals: &[]
        }),
        Err(AdapterError::WorktreeNotFound(_))
    ));
    let manager = created_worktree(&repo, &task);
    let path = manager.path_for(&TaskId::parse(task.task_id.clone()).unwrap());
    fs::write(path.join("dirty.txt"), "dirty\n").unwrap();
    assert!(matches!(
        adapter("echo").execute(AdapterRequest {
            task: &task,
            worktrees: &manager,
            acknowledged_approvals: &[]
        }),
        Err(AdapterError::DirtyWorktree(_))
    ));
}

#[test]
fn timeout_and_output_limits_reap_the_direct_child() {
    let repo = TestRepository::new();
    let task = task();
    let manager = created_worktree(&repo, &task);
    let timed_out = adapter("sleep")
        .execute(AdapterRequest {
            task: &task,
            worktrees: &manager,
            acknowledged_approvals: &[],
        })
        .unwrap();
    assert_eq!(timed_out.termination(), ExecutionTermination::TimedOut);
    assert!(timed_out.exit_code().is_none());
    let limited = adapter("flood")
        .execute(AdapterRequest {
            task: &task,
            worktrees: &manager,
            acknowledged_approvals: &[],
        })
        .unwrap();
    assert_eq!(
        limited.termination(),
        ExecutionTermination::OutputLimitExceeded
    );
    assert!(limited.output_truncated());
    assert!(limited.stdout().len() + limited.stderr().len() <= 4096);
}

#[test]
fn nonzero_exit_is_evidence_not_task_completion() {
    let repo = TestRepository::new();
    let task = task();
    let manager = created_worktree(&repo, &task);
    let report = adapter("fail")
        .execute(AdapterRequest {
            task: &task,
            worktrees: &manager,
            acknowledged_approvals: &[],
        })
        .unwrap();
    assert_eq!(report.termination(), ExecutionTermination::Exited);
    assert_eq!(report.exit_code(), Some(23));
    assert_eq!(report.task_id().as_str(), task.task_id);
}
