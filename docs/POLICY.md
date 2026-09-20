# Capability and permission policy

`agentforge-policy` is the provider-neutral validation boundary for task authority. It evaluates
only the explicit `AgentTask` contract, requested capability and paths, and caller-supplied approval
evidence. Decisions are deterministic and deny by default.

The engine rejects unsupported contracts, invalid or duplicate grants, missing capabilities,
forbidden paths, paths outside the task's owned scope, and unmet approval boundaries. Roles remain
descriptive and never imply authority. The engine performs no process spawning, filesystem mutation,
network access, secret access, worktree changes, or environment inspection; callers must enforce an
allowed decision at their side-effect boundary, and this crate is not an OS sandbox.
