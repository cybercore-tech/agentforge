//! Safe Git worktree lifecycle management for AgentForge.
//!
//! This crate owns Git process execution and filesystem orchestration for task-isolated worktrees.
//! Task identity remains owned by `agentforge-core`.

use agentforge_core::task::{TaskId, TaskIdError};
use std::ffi::OsStr;
use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// Project-relative directory containing AgentForge-managed linked worktrees.
pub const DEFAULT_MANAGED_RELATIVE_PATH: &str = ".forge/worktrees";

/// Prefix used for deterministic task branches.
pub const TASK_BRANCH_PREFIX: &str = "agentforge/task/";
const MAX_DIFF_OUTPUT: usize = 512 * 1024;

/// Description of one worktree creation request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorktreeSpec {
    task_id: TaskId,
    base_ref: String,
}

impl WorktreeSpec {
    /// Creates a worktree specification from an already validated task ID and a Git base ref.
    #[must_use]
    pub fn new(task_id: TaskId, base_ref: impl Into<String>) -> Self {
        Self {
            task_id,
            base_ref: base_ref.into(),
        }
    }

    /// Returns the task ID.
    #[must_use]
    pub fn task_id(&self) -> &TaskId {
        &self.task_id
    }

    /// Returns the requested Git base ref.
    #[must_use]
    pub fn base_ref(&self) -> &str {
        &self.base_ref
    }
}

/// High-level unresolved Git operation that makes automatic retirement unsafe.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GitOperation {
    /// Merge is in progress.
    Merge,
    /// Rebase is in progress.
    Rebase,
    /// Cherry-pick is in progress.
    CherryPick,
    /// Revert is in progress.
    Revert,
}

/// Verified status for one AgentForge-managed worktree.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorktreeStatus {
    task_id: TaskId,
    path: PathBuf,
    branch: String,
    head: String,
    dirty: bool,
    operation: Option<GitOperation>,
}

impl WorktreeStatus {
    /// Returns the owning task ID.
    #[must_use]
    pub fn task_id(&self) -> &TaskId {
        &self.task_id
    }

    /// Returns the linked-worktree path.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Returns the deterministic local branch name.
    #[must_use]
    pub fn branch(&self) -> &str {
        &self.branch
    }

    /// Returns the exact HEAD commit ID.
    #[must_use]
    pub fn head(&self) -> &str {
        &self.head
    }

    /// Returns whether tracked or untracked changes are present.
    #[must_use]
    pub const fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Returns an unresolved Git operation, when present.
    #[must_use]
    pub const fn operation(&self) -> Option<GitOperation> {
        self.operation
    }
}

/// Successfully created and verified managed worktree.
pub type ManagedWorktree = WorktreeStatus;

/// Bounded, read-only comparison between a task worktree and a target branch.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorktreeDiff {
    task_id: TaskId,
    source_branch: String,
    source_head: String,
    target_branch: String,
    target_head: String,
    merge_base: String,
    changed_files: Vec<String>,
    source_dirty: bool,
}

impl WorktreeDiff {
    /// Returns the task identity.
    #[must_use]
    pub fn task_id(&self) -> &TaskId {
        &self.task_id
    }
    /// Returns the deterministic source branch.
    #[must_use]
    pub fn source_branch(&self) -> &str {
        &self.source_branch
    }
    /// Returns the exact source commit.
    #[must_use]
    pub fn source_head(&self) -> &str {
        &self.source_head
    }
    /// Returns the comparison target branch.
    #[must_use]
    pub fn target_branch(&self) -> &str {
        &self.target_branch
    }
    /// Returns the exact target commit.
    #[must_use]
    pub fn target_head(&self) -> &str {
        &self.target_head
    }
    /// Returns the common ancestor used for the diff.
    #[must_use]
    pub fn merge_base(&self) -> &str {
        &self.merge_base
    }
    /// Returns changed repository-relative paths.
    #[must_use]
    pub fn changed_files(&self) -> &[String] {
        &self.changed_files
    }
    /// Returns whether the source worktree has uncommitted changes.
    #[must_use]
    pub const fn source_dirty(&self) -> bool {
        self.source_dirty
    }
}

/// Verified result of a fast-forward-only integration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IntegrationReport {
    task_id: TaskId,
    source_branch: String,
    source_head: String,
    target_branch: String,
    target_before: String,
    target_after: String,
    already_integrated: bool,
}

impl IntegrationReport {
    /// Returns the task identity.
    #[must_use]
    pub fn task_id(&self) -> &TaskId {
        &self.task_id
    }
    /// Returns the deterministic source branch.
    #[must_use]
    pub fn source_branch(&self) -> &str {
        &self.source_branch
    }
    /// Returns the exact source commit.
    #[must_use]
    pub fn source_head(&self) -> &str {
        &self.source_head
    }
    /// Returns the target branch.
    #[must_use]
    pub fn target_branch(&self) -> &str {
        &self.target_branch
    }
    /// Returns the target commit before integration.
    #[must_use]
    pub fn target_before(&self) -> &str {
        &self.target_before
    }
    /// Returns the verified target commit after integration.
    #[must_use]
    pub fn target_after(&self) -> &str {
        &self.target_after
    }
    /// Returns whether the source was already contained by the target.
    #[must_use]
    pub const fn already_integrated(&self) -> bool {
        self.already_integrated
    }
}

/// Safe manager for task-owned linked Git worktrees.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorktreeManager {
    project_root: PathBuf,
    managed_root: PathBuf,
}

impl WorktreeManager {
    /// Creates a manager for one repository root.
    ///
    /// # Errors
    ///
    /// Returns [`WorktreeError`] when the path is not a Git working tree root or filesystem
    /// canonicalization fails.
    pub fn new(project_root: impl AsRef<Path>) -> Result<Self, WorktreeError> {
        let supplied = fs::canonicalize(project_root.as_ref())?;
        let actual = git_text(&supplied, ["rev-parse", "--show-toplevel"])?;
        let actual = fs::canonicalize(actual.trim())?;

        if !paths_equivalent_or_same(&supplied, &actual)? {
            return Err(WorktreeError::NotRepositoryRoot { supplied, actual });
        }

        Ok(Self {
            managed_root: actual.join(DEFAULT_MANAGED_RELATIVE_PATH),
            project_root: actual,
        })
    }

    /// Returns the repository root owned by this manager.
    #[must_use]
    pub fn project_root(&self) -> &Path {
        &self.project_root
    }

    /// Returns the managed worktree root.
    #[must_use]
    pub fn managed_root(&self) -> &Path {
        &self.managed_root
    }

    /// Returns the checked-out branch of the project root.
    pub fn current_branch(&self) -> Result<String, WorktreeError> {
        let branch = git_text(&self.project_root, ["symbolic-ref", "--short", "HEAD"])?;
        let branch = branch.trim().to_owned();
        if branch.is_empty() {
            return Err(WorktreeError::DetachedTarget);
        }
        Ok(branch)
    }

    /// Returns the deterministic branch name for a task.
    #[must_use]
    pub fn branch_for(task_id: &TaskId) -> String {
        format!("{TASK_BRANCH_PREFIX}{task_id}")
    }

    /// Returns the deterministic managed path for a task.
    #[must_use]
    pub fn path_for(&self, task_id: &TaskId) -> PathBuf {
        self.managed_root.join(task_id.as_str())
    }

    /// Resolves a Git ref to an exact commit ID.
    ///
    /// # Errors
    ///
    /// Returns [`WorktreeError`] when Git cannot resolve the ref to a commit.
    pub fn resolve_base(&self, base_ref: &str) -> Result<String, WorktreeError> {
        if base_ref.trim().is_empty() {
            return Err(WorktreeError::InvalidBaseRef);
        }

        let expression = format!("{base_ref}^{{commit}}");
        git_text(&self.project_root, ["rev-parse", "--verify", &expression])
            .map(|value| value.trim().to_owned())
    }

    /// Creates and verifies a new task-owned linked worktree from an exact base commit.
    ///
    /// # Errors
    ///
    /// Returns [`WorktreeError`] for branch/path conflicts, unsafe managed-root state, Git
    /// failures, or post-create ownership mismatches.
    pub fn create(&self, spec: &WorktreeSpec) -> Result<ManagedWorktree, WorktreeError> {
        self.ensure_managed_root_safe()?;

        let path = self.path_for(spec.task_id());
        self.ensure_target_beneath_managed_root(&path)?;

        if path.exists() {
            return Err(WorktreeError::PathConflict(path));
        }

        if self.inspect(spec.task_id())?.is_some() {
            return Err(WorktreeError::AlreadyManaged(spec.task_id().clone()));
        }

        let branch = Self::branch_for(spec.task_id());
        let branch_ref = format!("refs/heads/{branch}");
        if git_success(
            &self.project_root,
            ["show-ref", "--verify", "--quiet", &branch_ref],
        )? {
            return Err(WorktreeError::BranchConflict(branch));
        }

        let base = self.resolve_base(spec.base_ref())?;
        let path_text = git_path_text(&path);
        git_text(
            &self.project_root,
            ["worktree", "add", "-b", &branch, &path_text, &base],
        )?;

        match self.inspect(spec.task_id())? {
            Some(status) if status.branch() == branch && status.head() == base => Ok(status),
            Some(status) => Err(WorktreeError::OwnershipMismatch {
                task_id: spec.task_id().clone(),
                path: status.path().to_path_buf(),
            }),
            None => Err(WorktreeError::PostCreateVerification(
                spec.task_id().clone(),
            )),
        }
    }

    /// Inspects one task-owned worktree.
    ///
    /// # Errors
    ///
    /// Returns [`WorktreeError`] when Git inspection fails or a path under the managed root has
    /// invalid task ownership metadata.
    pub fn inspect(&self, task_id: &TaskId) -> Result<Option<WorktreeStatus>, WorktreeError> {
        let expected_path = self.path_for(task_id);
        let expected_branch = Self::branch_for(task_id);

        for record in self.raw_worktrees()? {
            if !paths_equivalent_or_same(&record.path, &expected_path)? {
                continue;
            }

            self.ensure_target_beneath_managed_root(&record.path)?;

            let Some(branch) = record.branch else {
                return Err(WorktreeError::OwnershipMismatch {
                    task_id: task_id.clone(),
                    path: record.path,
                });
            };

            if branch != expected_branch {
                return Err(WorktreeError::OwnershipMismatch {
                    task_id: task_id.clone(),
                    path: record.path,
                });
            }

            let dirty = !git_text(
                &record.path,
                ["status", "--porcelain=v1", "--untracked-files=normal"],
            )?
            .trim()
            .is_empty();

            let operation = detect_operation(&record.path)?;

            return Ok(Some(WorktreeStatus {
                task_id: task_id.clone(),
                path: record.path,
                branch,
                head: record.head,
                dirty,
                operation,
            }));
        }

        Ok(None)
    }

    /// Lists verified AgentForge-managed worktrees in deterministic task-ID order.
    ///
    /// # Errors
    ///
    /// Returns [`WorktreeError`] when Git inspection fails or a candidate managed entry has an
    /// invalid ownership shape.
    pub fn list(&self) -> Result<Vec<WorktreeStatus>, WorktreeError> {
        let mut managed = Vec::new();

        for record in self.raw_worktrees()? {
            if !path_is_within(&record.path, &self.managed_root)? {
                continue;
            }

            let Some(name) = record.path.file_name().and_then(OsStr::to_str) else {
                continue;
            };
            let task_id = match TaskId::parse(name.to_owned()) {
                Ok(task_id) => task_id,
                Err(_) => continue,
            };
            let expected_path = self.path_for(&task_id);
            let expected_branch = Self::branch_for(&task_id);

            if !paths_equivalent_or_same(&record.path, &expected_path)? {
                continue;
            }
            if record.branch.as_deref() != Some(expected_branch.as_str()) {
                continue;
            }

            let dirty = !git_text(
                &record.path,
                ["status", "--porcelain=v1", "--untracked-files=normal"],
            )?
            .trim()
            .is_empty();
            let operation = detect_operation(&record.path)?;

            managed.push(WorktreeStatus {
                task_id,
                path: record.path,
                branch: expected_branch,
                head: record.head,
                dirty,
                operation,
            });
        }

        managed.sort_by(|left, right| left.task_id.cmp(&right.task_id));
        Ok(managed)
    }

    /// Retires a clean managed worktree while preserving its task branch.
    ///
    /// # Errors
    ///
    /// Returns [`WorktreeError`] when the worktree is absent, dirty, in the middle of a Git
    /// operation, fails ownership checks, or Git refuses non-forced removal.
    pub fn retire(&self, task_id: &TaskId) -> Result<(), WorktreeError> {
        let status = self
            .inspect(task_id)?
            .ok_or_else(|| WorktreeError::NotManaged(task_id.clone()))?;

        if status.is_dirty() {
            return Err(WorktreeError::DirtyWorktree(task_id.clone()));
        }
        if let Some(operation) = status.operation() {
            return Err(WorktreeError::UnresolvedOperation {
                task_id: task_id.clone(),
                operation,
            });
        }

        self.ensure_target_beneath_managed_root(status.path())?;
        let path_text = git_path_text(status.path());
        git_text(&self.project_root, ["worktree", "remove", &path_text])?;

        if self.inspect(task_id)?.is_some() {
            return Err(WorktreeError::PostRetireVerification(task_id.clone()));
        }

        let branch_ref = format!("refs/heads/{}", Self::branch_for(task_id));
        if !git_success(
            &self.project_root,
            ["show-ref", "--verify", "--quiet", &branch_ref],
        )? {
            return Err(WorktreeError::BranchMissingAfterRetire(task_id.clone()));
        }

        Ok(())
    }

    /// Produces a bounded, read-only diff against a target branch.
    pub fn diff(
        &self,
        task_id: &TaskId,
        target_branch: &str,
    ) -> Result<WorktreeDiff, WorktreeError> {
        validate_branch_name(target_branch)?;
        let source = self
            .inspect(task_id)?
            .ok_or_else(|| WorktreeError::NotManaged(task_id.clone()))?;
        let target_head = self.resolve_branch_head(target_branch)?;
        let merge_base = git_text(
            &self.project_root,
            ["merge-base", &target_head, source.head()],
        )?
        .trim()
        .to_owned();
        let changed = git_text_bounded(
            &self.project_root,
            [
                "diff",
                "--name-status",
                "--no-renames",
                &merge_base,
                source.head(),
            ],
            MAX_DIFF_OUTPUT,
        )?;
        let changed_files = changed
            .lines()
            .filter_map(|line| line.split_once('\t').map(|(_, path)| path.to_owned()))
            .collect();
        Ok(WorktreeDiff {
            task_id: task_id.clone(),
            source_branch: source.branch().to_owned(),
            source_head: source.head().to_owned(),
            target_branch: target_branch.to_owned(),
            target_head,
            merge_base,
            changed_files,
            source_dirty: source.is_dirty(),
        })
    }

    /// Integrates a task branch using a serialized, literal fast-forward-only Git operation.
    pub fn integrate(
        &self,
        task_id: &TaskId,
        target_branch: &str,
    ) -> Result<IntegrationReport, WorktreeError> {
        self.integrate_checked(task_id, target_branch, None)
    }

    /// Integrates a task branch only if its head is exactly `expected_head`.
    ///
    /// The head is checked again while the integration lock is held, so the fast-forward uses the
    /// same commit an operator approved.
    pub fn integrate_expecting(
        &self,
        task_id: &TaskId,
        target_branch: &str,
        expected_head: &str,
    ) -> Result<IntegrationReport, WorktreeError> {
        self.integrate_checked(task_id, target_branch, Some(expected_head))
    }

    fn integrate_checked(
        &self,
        task_id: &TaskId,
        target_branch: &str,
        expected_head: Option<&str>,
    ) -> Result<IntegrationReport, WorktreeError> {
        validate_branch_name(target_branch)?;
        let source = self
            .inspect(task_id)?
            .ok_or_else(|| WorktreeError::NotManaged(task_id.clone()))?;
        if source.is_dirty() {
            return Err(WorktreeError::DirtyWorktree(task_id.clone()));
        }
        if let Some(operation) = source.operation() {
            return Err(WorktreeError::UnresolvedOperation {
                task_id: task_id.clone(),
                operation,
            });
        }
        let current = self.current_branch()?;
        if current != target_branch {
            return Err(WorktreeError::TargetBranchMismatch {
                expected: target_branch.to_owned(),
                actual: current,
            });
        }
        if git_status_dirty(&self.project_root)? {
            return Err(WorktreeError::TargetDirty);
        }
        if let Some(operation) = detect_operation(&self.project_root)? {
            return Err(WorktreeError::TargetUnresolvedOperation(operation));
        }
        let target_before = self.resolve_branch_head(target_branch)?;
        let lock = IntegrationLock::acquire(self.project_root.join(".forge/integration.lock"))?;
        let _lock = lock;
        let current_source = self
            .inspect(task_id)?
            .ok_or_else(|| WorktreeError::NotManaged(task_id.clone()))?;
        let expected = expected_head.unwrap_or_else(|| source.head());
        if current_source.head() != expected {
            return Err(WorktreeError::StaleSource {
                expected: expected.to_owned(),
                actual: current_source.head().to_owned(),
            });
        }
        if git_success(
            &self.project_root,
            ["merge-base", "--is-ancestor", source.head(), &target_before],
        )? {
            return Ok(IntegrationReport {
                task_id: task_id.clone(),
                source_branch: source.branch().to_owned(),
                source_head: source.head().to_owned(),
                target_branch: target_branch.to_owned(),
                target_before: target_before.clone(),
                target_after: target_before,
                already_integrated: true,
            });
        }
        if !git_success(
            &self.project_root,
            ["merge-base", "--is-ancestor", &target_before, source.head()],
        )? {
            return Err(WorktreeError::NonFastForward {
                source: source.head().to_owned(),
                target: target_before,
            });
        }
        git_text(&self.project_root, ["merge", "--ff-only", source.branch()])?;
        let target_after = self.resolve_branch_head(target_branch)?;
        if target_after != source.head() {
            return Err(WorktreeError::PostIntegrationVerification {
                expected: source.head().to_owned(),
                actual: target_after,
            });
        }
        Ok(IntegrationReport {
            task_id: task_id.clone(),
            source_branch: source.branch().to_owned(),
            source_head: source.head().to_owned(),
            target_branch: target_branch.to_owned(),
            target_before,
            target_after,
            already_integrated: false,
        })
    }

    fn resolve_branch_head(&self, branch: &str) -> Result<String, WorktreeError> {
        let expression = format!("refs/heads/{branch}^{{commit}}");
        git_text(&self.project_root, ["rev-parse", "--verify", &expression])
            .map(|value| value.trim().to_owned())
    }

    fn ensure_managed_root_safe(&self) -> Result<(), WorktreeError> {
        let forge_root = self.project_root.join(".forge");
        reject_symlink_if_present(&forge_root)?;
        reject_symlink_if_present(&self.managed_root)?;
        fs::create_dir_all(&self.managed_root)?;
        self.ensure_target_beneath_managed_root(&self.managed_root)
    }

    fn ensure_target_beneath_managed_root(&self, path: &Path) -> Result<(), WorktreeError> {
        if paths_equivalent_or_same(path, &self.managed_root)? {
            let project = fs::canonicalize(&self.project_root)?;
            let managed = fs::canonicalize(&self.managed_root)?;
            if path_is_within(&managed, &project)? {
                return Ok(());
            }
            return Err(WorktreeError::PathEscape(path.to_path_buf()));
        }

        let parent_matches = path
            .parent()
            .map(|parent| paths_equivalent_or_same(parent, &self.managed_root))
            .transpose()?
            .unwrap_or(false);
        if !parent_matches {
            return Err(WorktreeError::PathEscape(path.to_path_buf()));
        }

        reject_symlink_if_present(path)?;
        Ok(())
    }

    fn raw_worktrees(&self) -> Result<Vec<RawWorktree>, WorktreeError> {
        let output = git_output(
            &self.project_root,
            ["worktree", "list", "--porcelain", "-z"],
        )?;
        parse_worktree_porcelain(&output.stdout)
    }
}

#[derive(Debug)]
struct RawWorktree {
    path: PathBuf,
    head: String,
    branch: Option<String>,
}

fn parse_worktree_porcelain(bytes: &[u8]) -> Result<Vec<RawWorktree>, WorktreeError> {
    let mut result = Vec::new();
    let mut current_path: Option<PathBuf> = None;
    let mut current_head: Option<String> = None;
    let mut current_branch: Option<String> = None;

    for field in bytes
        .split(|byte| *byte == 0)
        .filter(|field| !field.is_empty())
    {
        let text = std::str::from_utf8(field).map_err(|_| WorktreeError::NonUtf8GitOutput)?;

        if let Some(path) = text.strip_prefix("worktree ") {
            if let Some(previous) = current_path.take() {
                result.push(RawWorktree {
                    path: previous,
                    head: current_head
                        .take()
                        .ok_or(WorktreeError::MalformedPorcelain)?,
                    branch: current_branch.take(),
                });
            }
            current_path = Some(PathBuf::from(path));
        } else if let Some(head) = text.strip_prefix("HEAD ") {
            current_head = Some(head.to_owned());
        } else if let Some(branch) = text.strip_prefix("branch refs/heads/") {
            current_branch = Some(branch.to_owned());
        }
    }

    if let Some(path) = current_path {
        result.push(RawWorktree {
            path,
            head: current_head.ok_or(WorktreeError::MalformedPorcelain)?,
            branch: current_branch,
        });
    }

    Ok(result)
}

fn detect_operation(worktree: &Path) -> Result<Option<GitOperation>, WorktreeError> {
    let git_dir_text = git_text(worktree, ["rev-parse", "--git-dir"])?;
    let git_dir_raw = PathBuf::from(git_dir_text.trim());
    let git_dir = if git_dir_raw.is_absolute() {
        git_dir_raw
    } else {
        worktree.join(git_dir_raw)
    };

    if git_dir.join("MERGE_HEAD").exists() {
        return Ok(Some(GitOperation::Merge));
    }
    if git_dir.join("rebase-merge").exists() || git_dir.join("rebase-apply").exists() {
        return Ok(Some(GitOperation::Rebase));
    }
    if git_dir.join("CHERRY_PICK_HEAD").exists() {
        return Ok(Some(GitOperation::CherryPick));
    }
    if git_dir.join("REVERT_HEAD").exists() {
        return Ok(Some(GitOperation::Revert));
    }

    Ok(None)
}

fn reject_symlink_if_present(path: &Path) -> Result<(), WorktreeError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            Err(WorktreeError::SymlinkPath(path.to_path_buf()))
        }
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(WorktreeError::Io(error)),
    }
}

fn path_is_within(path: &Path, root: &Path) -> Result<bool, WorktreeError> {
    if !path.exists() || !root.exists() {
        return Ok(false);
    }
    let canonical_path = fs::canonicalize(path)?;
    let canonical_root = fs::canonicalize(root)?;
    let path_key = path_comparison_key(&canonical_path);
    let root_key = path_comparison_key(&canonical_root);
    Ok(path_key == root_key || path_key.starts_with(&path_prefix(&root_key)))
}

fn paths_equivalent_or_same(left: &Path, right: &Path) -> Result<bool, WorktreeError> {
    if left.exists() && right.exists() {
        let left = fs::canonicalize(left)?;
        let right = fs::canonicalize(right)?;
        return Ok(path_comparison_key(&left) == path_comparison_key(&right));
    }
    Ok(path_comparison_key(left) == path_comparison_key(right))
}

fn path_prefix(root: &str) -> String {
    if root.ends_with('/') || root.ends_with('\\') {
        root.to_owned()
    } else {
        format!("{root}/")
    }
}

#[cfg(windows)]
fn git_path_text(path: &Path) -> String {
    let value = path.to_string_lossy();
    if let Some(unc) = value.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{unc}")
    } else if let Some(device) = value.strip_prefix(r"\\?\") {
        device.to_owned()
    } else {
        value.into_owned()
    }
}

#[cfg(not(windows))]
fn git_path_text(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

#[cfg(windows)]
fn path_comparison_key(path: &Path) -> String {
    let mut value = path.to_string_lossy().replace('\\', "/");
    if let Some(unc) = value.strip_prefix("//?/UNC/") {
        value = format!("//{unc}");
    } else if let Some(device) = value.strip_prefix("//?/") {
        value = device.to_owned();
    }
    value.trim_end_matches('/').to_ascii_lowercase()
}

#[cfg(not(windows))]
fn path_comparison_key(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn git_text<I, S>(cwd: &Path, args: I) -> Result<String, WorktreeError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let output = git_output(cwd, args)?;
    if !output.status.success() {
        return Err(WorktreeError::GitFailure {
            status: output.status.code(),
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        });
    }
    String::from_utf8(output.stdout).map_err(|_| WorktreeError::NonUtf8GitOutput)
}

fn git_text_bounded<I, S>(cwd: &Path, args: I, limit: usize) -> Result<String, WorktreeError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let output = git_output(cwd, args)?;
    if !output.status.success() {
        return Err(WorktreeError::GitFailure {
            status: output.status.code(),
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        });
    }
    if output.stdout.len() > limit {
        return Err(WorktreeError::DiffOutputLimit);
    }
    String::from_utf8(output.stdout).map_err(|_| WorktreeError::NonUtf8GitOutput)
}

fn git_status_dirty(cwd: &Path) -> Result<bool, WorktreeError> {
    Ok(!git_text(
        cwd,
        [
            "status",
            "--porcelain=v1",
            "--untracked-files=normal",
            "--",
            ".",
            ":(exclude).forge",
        ],
    )?
    .trim()
    .is_empty())
}

fn validate_branch_name(branch: &str) -> Result<(), WorktreeError> {
    if branch.trim().is_empty()
        || branch.starts_with('-')
        || branch
            .chars()
            .any(|character| character.is_control() || character.is_whitespace())
        || branch.contains("..")
        || branch.ends_with('/')
    {
        return Err(WorktreeError::InvalidTargetBranch(branch.to_owned()));
    }
    Ok(())
}

struct IntegrationLock {
    path: PathBuf,
}

impl IntegrationLock {
    fn acquire(path: PathBuf) -> Result<Self, WorktreeError> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut file = match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                return Err(WorktreeError::IntegrationLocked(path));
            }
            Err(error) => return Err(WorktreeError::Io(error)),
        };
        writeln!(file, "pid={}", std::process::id())?;
        file.sync_all()?;
        Ok(Self { path })
    }
}

impl Drop for IntegrationLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

fn git_success<I, S>(cwd: &Path, args: I) -> Result<bool, WorktreeError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    Ok(git_output(cwd, args)?.status.success())
}

fn git_output<I, S>(cwd: &Path, args: I) -> Result<Output, WorktreeError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    Command::new("git")
        .current_dir(cwd)
        .env_remove("GIT_INDEX_FILE")
        .args(args)
        .output()
        .map_err(WorktreeError::Io)
}

/// Worktree lifecycle failure.
#[derive(Debug)]
pub enum WorktreeError {
    /// Filesystem or process I/O failed.
    Io(std::io::Error),
    /// Supplied root is inside a repository but is not its top-level working tree.
    NotRepositoryRoot {
        /// Root supplied by the caller.
        supplied: PathBuf,
        /// Root reported by Git.
        actual: PathBuf,
    },
    /// Requested base ref is empty.
    InvalidBaseRef,
    /// Git command failed.
    GitFailure {
        /// Process exit status code, when available.
        status: Option<i32>,
        /// Git stderr.
        stderr: String,
    },
    /// Git returned text that was not valid UTF-8.
    NonUtf8GitOutput,
    /// `git worktree list --porcelain -z` output was incomplete.
    MalformedPorcelain,
    /// A managed path escaped the configured worktree root.
    PathEscape(PathBuf),
    /// A managed path component is a symbolic link.
    SymlinkPath(PathBuf),
    /// The deterministic worktree path already exists.
    PathConflict(PathBuf),
    /// The deterministic task branch already exists.
    BranchConflict(String),
    /// A task already has a managed worktree.
    AlreadyManaged(TaskId),
    /// A worktree path exists but branch/task ownership does not match.
    OwnershipMismatch {
        /// Expected task owner.
        task_id: TaskId,
        /// Conflicting path.
        path: PathBuf,
    },
    /// Creation succeeded at the Git layer but could not be verified afterward.
    PostCreateVerification(TaskId),
    /// Requested task does not currently own a managed worktree.
    NotManaged(TaskId),
    /// Managed worktree has tracked or untracked changes.
    DirtyWorktree(TaskId),
    /// Managed worktree has an unresolved Git operation.
    UnresolvedOperation {
        /// Owning task.
        task_id: TaskId,
        /// Operation blocking retirement.
        operation: GitOperation,
    },
    /// Retirement returned but the worktree still exists in Git registry state.
    PostRetireVerification(TaskId),
    /// Retirement unexpectedly removed the preserved task branch.
    BranchMissingAfterRetire(TaskId),
    /// Invalid task identity encountered while reconstructing managed state.
    TaskId(TaskIdError),
    /// Target branch name is not a safe literal branch identifier.
    InvalidTargetBranch(String),
    /// The project root is detached and has no target branch.
    DetachedTarget,
    /// The checked-out target branch differs from the requested branch.
    TargetBranchMismatch {
        /// Requested target branch.
        expected: String,
        /// Checked-out branch.
        actual: String,
    },
    /// The project root contains tracked or untracked changes.
    TargetDirty,
    /// The project root has an unresolved Git operation.
    TargetUnresolvedOperation(GitOperation),
    /// Another integration currently owns the serialized integration lock.
    IntegrationLocked(PathBuf),
    /// The task branch changed between preflight and integration.
    StaleSource {
        /// Source head observed during preflight.
        expected: String,
        /// Source head observed after locking.
        actual: String,
    },
    /// The source and target histories cannot be fast-forwarded.
    NonFastForward {
        /// Source commit.
        source: String,
        /// Target commit.
        target: String,
    },
    /// A bounded diff exceeded its output budget.
    DiffOutputLimit,
    /// The target did not point at the source after a successful merge.
    PostIntegrationVerification {
        /// Expected target commit.
        expected: String,
        /// Actual target commit.
        actual: String,
    },
}

impl fmt::Display for WorktreeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "worktree I/O error: {error}"),
            Self::NotRepositoryRoot { supplied, actual } => write!(
                formatter,
                "worktree manager root {} is not repository root {}",
                supplied.display(),
                actual.display()
            ),
            Self::InvalidBaseRef => formatter.write_str("worktree base ref must not be empty"),
            Self::GitFailure { status, stderr } => {
                write!(
                    formatter,
                    "git command failed with status {status:?}: {stderr}"
                )
            }
            Self::NonUtf8GitOutput => formatter.write_str("git returned non-UTF-8 output"),
            Self::MalformedPorcelain => {
                formatter.write_str("malformed git worktree porcelain output")
            }
            Self::PathEscape(path) => write!(
                formatter,
                "managed worktree path escapes root: {}",
                path.display()
            ),
            Self::SymlinkPath(path) => write!(
                formatter,
                "managed worktree path is a symlink: {}",
                path.display()
            ),
            Self::PathConflict(path) => write!(
                formatter,
                "managed worktree path already exists: {}",
                path.display()
            ),
            Self::BranchConflict(branch) => {
                write!(formatter, "managed task branch already exists: {branch}")
            }
            Self::AlreadyManaged(task_id) => {
                write!(formatter, "task already has a managed worktree: {task_id}")
            }
            Self::OwnershipMismatch { task_id, path } => write!(
                formatter,
                "managed worktree ownership mismatch for {task_id}: {}",
                path.display()
            ),
            Self::PostCreateVerification(task_id) => {
                write!(formatter, "unable to verify created worktree for {task_id}")
            }
            Self::NotManaged(task_id) => {
                write!(formatter, "task has no managed worktree: {task_id}")
            }
            Self::DirtyWorktree(task_id) => {
                write!(formatter, "managed worktree is dirty: {task_id}")
            }
            Self::UnresolvedOperation { task_id, operation } => write!(
                formatter,
                "managed worktree {task_id} has unresolved Git operation: {operation:?}"
            ),
            Self::PostRetireVerification(task_id) => write!(
                formatter,
                "retired worktree still appears in Git registry: {task_id}"
            ),
            Self::BranchMissingAfterRetire(task_id) => write!(
                formatter,
                "task branch missing after worktree retirement: {task_id}"
            ),
            Self::TaskId(error) => write!(formatter, "invalid managed task ID: {error}"),
            Self::InvalidTargetBranch(branch) => {
                write!(formatter, "invalid target branch name: {branch}")
            }
            Self::DetachedTarget => {
                formatter.write_str("project root is detached; target branch is unavailable")
            }
            Self::TargetBranchMismatch { expected, actual } => write!(
                formatter,
                "checked-out target branch mismatch: expected {expected}, found {actual}"
            ),
            Self::TargetDirty => formatter.write_str("target branch worktree is dirty"),
            Self::TargetUnresolvedOperation(operation) => {
                write!(
                    formatter,
                    "target branch has unresolved Git operation: {operation:?}"
                )
            }
            Self::IntegrationLocked(path) => {
                write!(
                    formatter,
                    "integration lock is already held: {}",
                    path.display()
                )
            }
            Self::StaleSource { expected, actual } => {
                write!(
                    formatter,
                    "task source changed during integration: expected {expected}, found {actual}"
                )
            }
            Self::NonFastForward { source, target } => {
                write!(
                    formatter,
                    "source {source} is not a fast-forward of target {target}"
                )
            }
            Self::DiffOutputLimit => {
                formatter.write_str("task diff exceeds the bounded output limit")
            }
            Self::PostIntegrationVerification { expected, actual } => write!(
                formatter,
                "integration verification mismatch: expected {expected}, found {actual}"
            ),
        }
    }
}

impl std::error::Error for WorktreeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::TaskId(error) => Some(error),
            _ => None,
        }
    }
}

impl From<std::io::Error> for WorktreeError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<TaskIdError> for WorktreeError {
    fn from(error: TaskIdError) -> Self {
        Self::TaskId(error)
    }
}
