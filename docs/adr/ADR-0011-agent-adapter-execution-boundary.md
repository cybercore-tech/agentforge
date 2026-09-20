# ADR-0011: Provider-neutral agent adapter execution boundary

- Status: Proposed
- Date: 2026-09-19
- Decision owners: AgentForge project

## Context

AgentForge has task/result contracts and task-owned worktrees. External coding-agent execution
needs a boundary that preserves those contracts and does not mistake a process exit for accepted
task completion.

## Proposed decision

A separate `agentforge-adapter` crate will define a synchronous provider-neutral adapter trait.
Its first implementation will invoke an explicitly configured foreground executable in a worktree
re-inspected through `WorktreeManager` immediately before launch.

The adapter will validate task identity, local-command capability, and caller acknowledgements
of required human approvals. The caller remains responsible for actually obtaining approvals.

Tasks will be rendered completely and deterministically to stdin. Executable, arguments, and
environment will come from operator configuration, using direct process arguments and an explicit
environment. The executor will bound captured output, enforce a deadline, and reap the direct child.

An execution report will describe process evidence. Exit zero will not imply `TaskOutcome::Completed`.
Separately reported agent results must match task identity and contract version; acceptance, gates,
and state transitions remain caller responsibilities.

## Consequences

The interface can be tested offline with a fixture executable and later extended by provider adapters.
No provider SDK or external Rust dependency is needed for this milestone.

The generic process adapter expects stdin-compatible foreground executables or operator wrappers.
Background processes, descendant supervision, and OS sandboxing are outside this decision. Passing
capabilities and owned paths into a prompt does not enforce them on an arbitrary process.

## References

- `.plans/P0-M006-agent-adapter-interface.plan.md`
- `docs/TASK_CONTRACT.md`
- `docs/WORKTREE_ISOLATION.md`
- `docs/APPROVAL_BOUNDARIES.md`
