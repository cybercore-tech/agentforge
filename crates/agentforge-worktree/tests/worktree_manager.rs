//! Integration tests for the AgentForge managed Git worktree lifecycle.
use agentforge_core::task::TaskId;
use agentforge_worktree::{WorktreeError, WorktreeManager, WorktreeSpec};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

static TEST_SEQUENCE: AtomicU64 = AtomicU64::new(0);

struct TestRepository {
    root: PathBuf,
}

impl TestRepository {
    fn new() -> Self {
        let sequence = TEST_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "agentforge-worktree-test-{}-{sequence}",
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

fn task(sequence: u32) -> TaskId {
    TaskId::for_sequence("P0-M005", sequence).unwrap()
}

#[test]
fn branch_and_path_are_deterministic() {
    let repo = TestRepository::new();
    let manager = repo.manager();
    let task_id = task(1);

    assert_eq!(
        WorktreeManager::branch_for(&task_id),
        "agentforge/task/P0-M005-T0001"
    );
    assert_eq!(
        manager.path_for(&task_id),
        repo.root.join(".forge/worktrees/P0-M005-T0001")
    );
}

#[test]
fn create_inspect_list_and_retire_preserve_branch() {
    let repo = TestRepository::new();
    let manager = repo.manager();
    let task_id = task(1);
    let base = manager.resolve_base("main").unwrap();

    let created = manager
        .create(&WorktreeSpec::new(task_id.clone(), "main"))
        .unwrap();

    assert_eq!(created.head(), base);
    assert_eq!(created.branch(), "agentforge/task/P0-M005-T0001");
    assert!(!created.is_dirty());
    assert!(created.operation().is_none());

    let inspected = manager.inspect(&task_id).unwrap().unwrap();
    assert_eq!(inspected, created);

    let listed = manager.list().unwrap();
    assert_eq!(listed, vec![created]);

    manager.retire(&task_id).unwrap();
    assert!(manager.inspect(&task_id).unwrap().is_none());

    let status = Command::new("git")
        .current_dir(&repo.root)
        .args([
            "show-ref",
            "--verify",
            "--quiet",
            "refs/heads/agentforge/task/P0-M005-T0001",
        ])
        .status()
        .unwrap();
    assert!(status.success());
}

#[test]
fn dirty_worktree_cannot_be_retired() {
    let repo = TestRepository::new();
    let manager = repo.manager();
    let task_id = task(1);
    let created = manager
        .create(&WorktreeSpec::new(task_id.clone(), "main"))
        .unwrap();

    fs::write(created.path().join("untracked.txt"), "dirty\n").unwrap();

    let inspected = manager.inspect(&task_id).unwrap().unwrap();
    assert!(inspected.is_dirty());
    assert!(matches!(
        manager.retire(&task_id),
        Err(WorktreeError::DirtyWorktree(found)) if found == task_id
    ));

    fs::remove_file(created.path().join("untracked.txt")).unwrap();
    manager.retire(&task_id).unwrap();
}

#[test]
fn existing_task_branch_is_rejected() {
    let repo = TestRepository::new();
    let manager = repo.manager();
    let task_id = task(1);
    run(
        &repo.root,
        &["branch", "agentforge/task/P0-M005-T0001", "main"],
    );

    assert!(matches!(
        manager.create(&WorktreeSpec::new(task_id, "main")),
        Err(WorktreeError::BranchConflict(branch))
            if branch == "agentforge/task/P0-M005-T0001"
    ));
}

#[test]
fn unrelated_worktree_is_not_listed_as_managed() {
    let repo = TestRepository::new();
    let manager = repo.manager();
    let unrelated = repo.root.join("unrelated-worktree");
    let unrelated_text = unrelated.to_string_lossy().into_owned();

    run(
        &repo.root,
        &[
            "worktree",
            "add",
            "-b",
            "unrelated",
            &unrelated_text,
            "main",
        ],
    );

    assert!(manager.list().unwrap().is_empty());

    run(&repo.root, &["worktree", "remove", &unrelated_text]);
}
