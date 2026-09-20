# Plan: P0-M005 — Worktree isolation manager

Status: Approved
Milestone: P0-M005
Created: 2026-09-19

## Goal

Give AgentForge a safe, deterministic manager for creating, inspecting, and retiring isolated Git
worktrees owned by individual AgentForge tasks.

P0-M005 establishes the execution-isolation boundary required before AgentForge can hand work to
external coding agents.

The milestone must provide:

- deterministic task-owned worktree paths;
- deterministic task-owned branch names;
- creation from an exact base commit;
- verification that a worktree belongs to the expected task;
- inspection of managed worktree state;
- dirty-worktree detection;
- non-destructive retirement;
- protection against path escape and accidental deletion;
- no shell command construction from task-controlled input;
- focused integration tests using temporary Git repositories.

## Non-goals

- No external coding-agent execution.
- No scheduler.
- No parallel task dispatcher.
- No automatic merge.
- No automatic rebase.
- No branch deletion during normal retirement.
- No forced worktree removal.
- No git clean.
- No git reset --hard.
- No remote Git operations.
- No GitHub API interaction.
- No CI monitoring.
- No task-state schema replacement.
- No audit/event subsystem.
- No deployment behavior.

## Context

ADR-0002 establishes Git worktrees as AgentForge's task isolation boundary.

P0-M004 established deterministic TaskId values and durable task state. P0-M005 now binds one
implementation task to one isolated Git branch and physical worktree.

Agents must never share a dirty working tree. Worktree creation and retirement therefore need to be
owned by AgentForge rather than left to provider-specific adapters.

P0-M005 base main commit:

`93f88b1af2f129c63b95bb0d9f7ca72677c847af`

## Architecture placement

agentforge-core
- TaskId / task semantics

agentforge-worktree
- WorktreeManager
- WorktreeSpec
- ManagedWorktree
- WorktreeStatus
- WorktreeError
- Git command boundary
- managed worktree lifecycle

physical project layout
- repository working tree
- .forge/state/
- .forge/worktrees/<task-id>/

agentforge-core owns task identity and semantics.

agentforge-worktree owns Git process execution, path validation, worktree inspection, and lifecycle.

No Git subprocess logic belongs in agentforge-core.

## Managed identity

For task P0-M005-T0001:

- branch: agentforge/task/P0-M005-T0001
- worktree: <project-root>/.forge/worktrees/P0-M005-T0001

Task IDs are validated by agentforge-core before participating in path or branch construction.

## Data flow

### Create

1. Receive validated TaskId.
2. Resolve requested base ref to an exact commit.
3. Verify repository root is a Git worktree.
4. Derive deterministic branch and managed path.
5. Verify target path remains beneath the managed root.
6. Reject already-managed tasks.
7. Reject existing conflicting branches.
8. Invoke Git using direct argument passing, never shell interpolation.
9. Create branch and linked worktree from exact base commit.
10. Re-inspect Git worktree registry.
11. Return verified ManagedWorktree.

### Inspect

1. Query git worktree list --porcelain.
2. Parse machine-readable records.
3. Identify only worktrees beneath AgentForge managed root.
4. Match task ownership by deterministic path and branch.
5. Inspect HEAD, branch, lock/prunable state, and dirty state.
6. Return deterministic structured status.

### Retire

1. Re-inspect target.
2. Verify task ID, path, and branch agree.
3. Reject paths outside managed root.
4. Reject dirty worktrees.
5. Reject unresolved Git operation state.
6. Remove through git worktree remove without --force.
7. Verify Git no longer lists the worktree.
8. Preserve task branch.

## Invariants

- One managed task maps to exactly one deterministic worktree path.
- One managed task maps to exactly one deterministic branch.
- Creation starts from an exact resolved commit.
- Worktree paths never escape the configured managed root.
- Task-controlled strings never pass through a shell.
- Existing unrelated branches are never adopted implicitly.
- Existing unrelated worktrees are never removed.
- Dirty worktrees are never retired automatically.
- Worktrees with unresolved Git operations are never retired automatically.
- Normal retirement never uses --force.
- Normal retirement does not delete the task branch.
- No routine path uses git reset --hard.
- No routine path uses git clean.
- No network operation is required.
- agentforge-core remains free of Git/filesystem process orchestration.
- Managed inspection is derived from Git authoritative worktree registry.
- Managed ordering is deterministic by task ID.
- .forge runtime state is ignored by the repository.
- MSRV remains Rust 1.85.0.
- No external Rust dependency is required.

## ADRs

Existing:

- ADR-0002 — Git worktrees as the task isolation boundary.
- ADR-0003 — durable state outside model memory.
- ADR-0005 — repair failures forward.
- ADR-0008 — versioned structured task and result contracts.
- ADR-0009 — versioned task-state snapshots.

New:

- ADR-0010 — deterministic managed worktree lifecycle.

## Public API / CLI

Expected library API shape:

- WorktreeManager
- WorktreeSpec
- ManagedWorktree
- WorktreeStatus
- WorktreeError

Likely operations:

- WorktreeManager::new(...)
- WorktreeManager::create(...)
- WorktreeManager::inspect(...)
- WorktreeManager::list(...)
- WorktreeManager::retire(...)

No stable end-user CLI contract is required in P0-M005.

## Compatibility analysis

- Rust MSRV remains 1.85.0.
- Git must support git worktree.
- Git porcelain output is used for machine parsing.
- Branch/path derivation becomes compatibility-sensitive once persisted.
- P0-M005 must not alter P0-M004 snapshot semantics.
- Existing task IDs remain authoritative.

## Dependency analysis

P0-M005 remains standard-library-only.

Git interaction uses std::process::Command.

## Expected file boundary

Plan checkpoint:

- .plans/ACTIVE
- .plans/P0-M005-worktree-isolation-manager.plan.md
- docs/MILESTONES.md
- docs/WORKTREE_ISOLATION.md
- docs/adr/README.md
- docs/adr/ADR-0010-managed-worktree-lifecycle.md
- PROJECT_STATE.md
- AGENT_HANDOFF.md

Implementation checkpoint:

- .gitignore
- .github/workflows/ci.yml — CI runtime maintenance only
- Cargo.toml
- Cargo.lock
- crates/agentforge-worktree/Cargo.toml
- crates/agentforge-worktree/src/lib.rs
- focused module files beneath crates/agentforge-worktree/src/
- focused integration tests

Existing agentforge-core task semantics should not require changes.

Any expansion requires an explicit plan amendment before implementation.

## Test-first matrix

| Behavior | Expected |
| --- | --- |
| deterministic branch | exact stable branch |
| deterministic path | exact path beneath managed root |
| path escape | rejected |
| resolve base | exact commit captured |
| invalid base | controlled failure |
| create fresh worktree | success |
| created HEAD | exact requested base |
| created branch | deterministic task branch |
| task already managed | reject |
| branch already exists | reject |
| target path already exists | reject |
| list managed worktrees | deterministic order |
| unrelated Git worktree | ignored |
| modified tracked file | dirty |
| untracked file | dirty |
| retire clean managed worktree | success |
| retire dirty worktree | reject |
| unresolved Git operation | reject retirement |
| retire unrelated worktree | reject |
| normal retirement | branch retained |
| command execution | no shell |
| .forge runtime files | ignored |
| core crate | no Git process dependency |

## Implementation sequence

1. Commit this Approved plan-only checkpoint.
2. Require exact plan-head local full gate.
3. Push plan checkpoint.
4. Require exact plan-head GitHub Actions green.
5. Add agentforge-worktree workspace crate.
6. Add temporary Git-repository test helpers.
7. Implement deterministic branch/path derivation.
8. Implement exact base-ref resolution.
9. Implement porcelain worktree parsing.
10. Implement ownership verification.
11. Implement safe create.
12. Implement inspect/list.
13. Implement dirty/unresolved-operation detection.
14. Implement safe non-forced retirement.
15. Add .forge repository ignore boundary.
16. Update GitHub Actions checkout to the current Node 24 release line and pin the Ubuntu runner baseline.
17. Run focused tests.
18. Run full local gate.
19. Commit implementation checkpoint.
20. Require exact implementation CI green.
21. Classify failures before repair.
22. Close P0-M005 separately.
23. Require exact closure CI green.
24. Merge with history preserved.
25. Require post-merge main CI green before P0-M006.

## Failure modes

- Parsing human-formatted worktree output.
- Shell construction from task input.
- Branch/path injection.
- Symbolic base without exact commit resolution.
- Silently adopting an existing branch.
- Deleting branch during normal retirement.
- Removing dirty worktrees.
- Routine forced removal.
- Removing paths based only on naming.
- Trusting filesystem layout without Git verification.
- Destructive cleanup after partial failure.
- Leaking Git orchestration into agentforge-core.

## Documentation impact

- Add docs/WORKTREE_ISOLATION.md.
- Add ADR-0010.
- Update milestone/state/handoff records.

## Quality gates

Local:

- ./scripts/check-text-files
- cargo run -p xtask --locked -- validate
- cargo run -p xtask --locked -- validate-plan-policy
- cargo fmt --all --check
- cargo check --workspace --all-targets --locked
- cargo clippy --workspace --all-targets --locked -- -D warnings
- cargo test --workspace --locked
- ./scripts/gate.sh full

Remote:

- Repository policy
- Stable code gate
- MSRV 1.85.0
- CLI smoke

## Acceptance criteria

- [ ] Managed branch naming is deterministic.
- [ ] Managed worktree paths are deterministic.
- [ ] Worktree paths cannot escape the managed root.
- [ ] Creation resolves and records exact base commit.
- [ ] Creation refuses conflicting branches.
- [ ] Creation refuses conflicting worktrees.
- [ ] Git commands use direct arguments without shell interpolation.
- [ ] Managed worktrees can be inspected deterministically.
- [ ] Unrelated worktrees are not treated as AgentForge-owned.
- [ ] Dirty state is detected.
- [ ] Unresolved Git-operation state is detected.
- [ ] Dirty worktrees cannot be retired normally.
- [ ] Unresolved worktrees cannot be retired normally.
- [ ] Clean managed worktrees can be retired safely.
- [ ] Normal retirement preserves task branch.
- [ ] No normal operation uses forced removal.
- [ ] No normal operation uses git reset --hard.
- [ ] No normal operation uses git clean.
- [ ] .forge runtime data is ignored by Git.
- [ ] GitHub Actions use a Node 24 checkout release.
- [ ] GitHub Actions runner OS is pinned for reproducibility.
- [ ] agentforge-core gains no Git-process dependency.
- [ ] Temporary-repository integration tests cover create/inspect/retire.
- [ ] Exact implementation local gate is green.
- [ ] Exact implementation CI is green.
- [ ] Exact closure CI is green.
- [ ] Post-merge main CI is green.

## Completion record

Implementation commit:
Implementation CI:
Closure commit:
Closure CI:
Post-merge main:
Post-merge CI:
Completed:
Notes:
