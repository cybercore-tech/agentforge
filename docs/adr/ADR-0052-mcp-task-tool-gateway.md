# ADR-0052: MCP task-tool gateway

- Status: Accepted
- Date: 2026-09-25
- Milestone: P3-M005

## Context

Agents check their own work with whatever shell access their adapter allows. The Claude Code bridge
allows `Bash(./scripts/gate.sh *)`, which only fits this repository, and an agent learns about a
path-boundary violation only after it has finished. `Capability::UseMcpTools` has existed in the
task contract since P0, but nothing served or enforced it. MCP (the Model Context Protocol) is how
agents such as Claude Code discover and call tools, so a gateway lets any project's agent ask
AgentForge directly, under the same policy the orchestrator enforces.

## Decision

- **Task-bound.** `forge mcp serve --contract <file> --worktree <dir> [--root <project>]` serves
  exactly one task. The task comes from the same `agentforge-task-prompt-v1` document the agent
  receives; the gateway holds no other authority.
- **Tools mapped to capabilities.** Each tool requires `use_mcp_tools` plus its own capability,
  checked with `PolicyEngine::evaluate`:
  - `task_contract` and `check_changes` require `read_repository`. `check_changes` judges each
    changed path as a `write_owned_paths` request, so scope, forbidden paths, and write authority
    all apply.
  - `run_gate` requires `run_local_commands`. It runs only a gate the task requires, from the
    project's reviewed gate profiles, in the task's worktree.

  `tools/list` shows only the tools the task may call, and `tools/call` checks again. A denied or
  unknown tool is a JSON-RPC `-32602` error.
- **Advisory, never authoritative.** No tool grants a capability, records an approval, commits,
  or changes task state. `run_gate` results are for the agent. The orchestrator runs the gates
  again after the agent finishes, and only those results count.
- **Audited, failing closed.** With a project root that has task state, every call is appended as
  `ToolInvoked` (audit code 13): actor `mcp:<task>`, `tool`, `decision`, `channel=mcp`, and the
  outcome and reason. An allowed call that cannot be recorded does not run. Without a project
  root (a remote worker's clone has no project state), calls are logged to the gateway's stderr
  only.
- **Transport.** stdio only, newline-delimited JSON-RPC 2.0 with messages bounded at 1 MiB, and
  stdout reserved for protocol messages. Protocol versions `2025-11-25`, `2025-06-18`,
  `2025-03-26`, and `2024-11-05`; tools only, with no resources, prompts, or sampling.
- **Dependencies.** `serde` and `serde_json` are used by `agentforge-mcp` only, approved by the
  operator on 2026-09-25.
- **Bridge.** The Claude Code bridge adds the gateway (`--mcp-config`, `--strict-mcp-config`, and
  the `mcp__agentforge` tools) only for tasks that hold `use_mcp_tools`. It needs `forge` from
  `--forge` or `PATH`, and refuses the task when neither resolves.

## Consequences

- Agents can check scope and run the right gates before finishing, in any project, without
  project-specific shell permissions.
- A new audit kind means an older `forge` cannot read an audit log that contains tool calls, the
  same trade-off as `LeaseRecorded` (P4-M004).
- A later milestone can put external MCP servers behind the same gateway, reusing the tool policy
  map and the `ToolInvoked` event.

## References

- `docs/MCP_GATEWAY.md`
- `.plans/P3-M005-mcp-task-tool-gateway.plan.md`
- ADR-0015 (capability policy)
