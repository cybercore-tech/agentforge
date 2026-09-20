# Agent Adapters

P0-M006 defines the process boundary for external coding agents. The
`agentforge-adapter` crate uses `AgentTask` and `AgentResult` from `agentforge-core`.

## Preflight

Before launch, the adapter validates the task contract and task ID, requires
`run_local_commands`, checks caller acknowledgements for declared approval boundaries, and
re-inspects the deterministic managed worktree. The worktree must exist, be clean, and have no
unresolved Git operation. Callers cannot supply an arbitrary working directory.

## Process input and evidence

The first adapter launches an operator-configured foreground executable. Its path is absolute,
arguments are passed directly, and its environment is cleared then rebuilt from explicit values.
The task arrives on stdin as `agentforge-task-prompt-v1`: fields have byte lengths and lists have
counts followed by length-delimited items. This preserves multiline and Unicode task text.

The adapter captures raw stdout and stderr under one shared byte limit, drains both streams
concurrently, enforces a deadline, and reaps its direct child. It reports raw bytes, exit status,
and whether it exited, timed out, or exceeded the output limit.

The executable must remain in the foreground and must not leave descendants holding adapter pipes.
The adapter does not supervise a process tree.

## Trust boundary

An exit status of zero records process behavior; it does not mean that the task is complete. Raw
stdout is not parsed as an `AgentResult`. A separately supplied result must match the submitted task
ID and contract version, after which the caller still runs gates, reviews the evidence, and changes
task state.

Capability and path declarations are task instructions, not an OS sandbox. The adapter provides no
network or filesystem isolation, credential access, automatic Git operations, or provider behavior.
