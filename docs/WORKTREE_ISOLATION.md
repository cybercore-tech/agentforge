# Worktree Isolation

AgentForge uses Git linked worktrees as the physical isolation boundary for implementation tasks.

## Ownership

A validated AgentForge task ID deterministically owns:

- one branch named `agentforge/task/<task-id>`;
- one worktree rooted at `.forge/worktrees/<task-id>`.

Git's worktree registry remains authoritative. Filesystem naming alone does not prove ownership.

## Creation

Creation resolves the selected base ref to an exact commit before creating the task branch and
worktree.

Existing branches or paths are conflicts rather than silently adopted.

## Inspection

Managed worktrees are discovered through `git worktree list --porcelain`.

Inspection reports task identity, path, branch, HEAD commit, cleanliness, and relevant Git operation
state.

On Windows, Git may report a normal path while the filesystem returns a verbatim canonical path.
The manager compares normalized case-insensitive path keys and accepts only equivalent forms; it
still requires every managed entry to remain directly beneath the deterministic managed root.
The managed lifecycle integration suite runs in the Linux, macOS, and Windows CI matrix so this
path-equivalence behavior is exercised through the same create, inspect, dirty-state, and retire
workflow on each supported host.

## Retirement

Normal retirement is intentionally conservative.

A worktree must:

- be owned by the expected task;
- remain beneath the managed root;
- be clean;
- have no unresolved merge, rebase, cherry-pick, or revert operation.

Normal retirement removes the linked worktree but preserves its branch.

AgentForge does not use forced removal, `git clean`, or `git reset --hard` as routine cleanup.

## Security boundary

Task-controlled values are passed to Git as process arguments and are never interpolated into shell
commands.
