# MCP task-tool gateway

Since P3-M005 (ADR-0052), an agent working on an AgentForge task can call AgentForge itself through
[MCP](https://modelcontextprotocol.io), the Model Context Protocol that agents such as Claude Code
use to discover and call tools. `forge mcp serve` is a stdio MCP server bound to **one task
contract and one worktree**. It offers that task's agent three tools, lists only the ones the task
may call, checks every call against the task's capabilities, and records each call in the
project's audit log.

```bash
forge mcp serve --contract <task-prompt-file> --worktree <dir> [--root <project>]
```

- `--contract` is the `agentforge-task-prompt-v1` document the agent received on stdin.
- `--worktree` is where `check_changes` and `run_gate` operate.
- `--root` is the AgentForge project. With it, calls are audited there, and `run_gate` loads the
  project's gate profiles. Without it (a remote worker's clone has no project state), calls are
  logged to stderr only and `run_gate` is not offered.

Normally you don't run it yourself: the Claude Code bridge starts it for tasks that hold
`use_mcp_tools` (see [Using it with the Claude Code bridge](#using-it-with-the-claude-code-bridge)).

## Tools and the capabilities they need

Every tool needs `use_mcp_tools` plus its own capability, checked with the same `PolicyEngine`
the orchestrator uses:

| Tool | Needs | What it does |
| --- | --- | --- |
| `task_contract` | `read_repository` | The task's goal, non-goals, allowed and forbidden paths, required gates, capabilities, approvals, and expected outputs, as structured data and text. |
| `check_changes` | `read_repository` | Every changed path in the worktree (tracked and untracked, from `git status`), each judged as a `write_owned_paths` request: allowed, or denied with the policy's reason (`path outside allowed scope`, `forbidden path`, or `missing capability: write_owned_paths`). At most 1,000 paths are listed; the totals always count all of them. |
| `run_gate` | `run_local_commands` | Runs one of the task's `required_gates` from the project's gate profile (`.forge/gates/<gate>.conf`) in the worktree. It returns the outcome, the exit code, and the last 4 KiB of stdout and stderr. Only offered with `--root` and when the task requires gates. |

`tools/list` shows only the tools the task may call. `tools/call` checks again: a tool the task may
not call, or one that doesn't exist, is refused with JSON-RPC error `-32602` and the reason, and the
refusal is recorded. A tool that runs but fails (for example `run_gate` with a gate the task does
not require) returns a result with `isError: true`.

**Advisory only.** No tool grants authority, records an approval, commits, or changes task state.
`run_gate` results are for the agent's own feedback; the orchestrator runs every required gate
again after the agent finishes, and only those results count.

## Audit

With `--root` pointing at a project with task state, every call is appended to `.forge/audit.log`
as a `ToolInvoked` event (code 13):

| Field | Value |
| --- | --- |
| actor | `mcp:<task-id>` |
| task | the task ID |
| `tool` | the tool name as called |
| `decision` | `allowed` or `denied` |
| `channel` | `mcp` |
| `reason` | the policy's or tool's reason, when there is one |
| `outcome` | `ok`, `clean`/`violations` (`check_changes`, with `changed` and `denied` counts), `passed`/`failed`/`timed_out`/`output_limit_exceeded` (`run_gate`, with `gate`), or `error` |

A call that cannot be recorded does not run, and returns JSON-RPC error `-32603`. The gateway
never creates an audit log outside an initialized project.

Tool calls happen while the agent runs, but the orchestrator persists a run's own events
(`AgentStarted`, `AgentFinished`, gates) as one batch when it finishes. So a run's tool calls
usually have *lower* sequence numbers than its `AgentStarted`, while their timestamps show the true
order (P0-M015).

## Protocol

- stdio, newline-delimited JSON-RPC 2.0. stdout carries only protocol messages; diagnostics go to
  stderr.
- `initialize` negotiates the protocol version: `2025-11-25`, `2025-06-18`, `2025-03-26`, or
  `2024-11-05`. A client asking for another version gets the newest.
- Supported methods: `initialize`, `notifications/initialized`, `ping`, `tools/list`, and
  `tools/call`. Everything else is `-32601`. Requests other than `initialize` and `ping` before
  `initialize` are refused (`-32600`).
- Messages are bounded at 1 MiB. An oversized message is refused and the session continues.
  Batches (JSON arrays) are refused.
- There are no resources, prompts, sampling, or elicitation, and `tools/list` does not change during
  a session.

## Using it with the Claude Code bridge

Give the task the capabilities:

```bash
forge task create . <task-id> <milestone> implementer "<goal>" --allowed <path> \
  --capability use_mcp_tools --capability read_repository --capability write_owned_paths \
  --capability run_local_commands --gate <gate>
```

and give the bridge profile the `forge` executable, because profiles run with a cleared
environment:

```text
argument=--forge
argument=/home/you/.cargo/bin/forge
```

For such a task the bridge:
- saves the contract;
- writes an MCP config naming `forge mcp serve` (with `--root` when the worktree's project has task
  state);
- runs Claude Code with `--mcp-config <file> --strict-mcp-config` and the `mcp__agentforge` tools
  allowed;
- tells the agent to call `check_changes` before it finishes.

A task with `use_mcp_tools` and no resolvable `forge` is refused (exit 2). Tasks without the
capability run exactly as before.

Verified with a real Claude Code session (P3-M005). Through `forge run`, the agent called
`task_contract`, `check_changes` (1 changed path, 0 denied), and `run_gate` (passed). It created
the file within scope, and the bridge committed it. The orchestrator's gate passed 1/1, and all
three calls are `ToolInvoked` events in the project's audit log.

## External MCP servers (P3-M006)

The gateway can also front external MCP servers, such as a GitHub, filesystem, or search server,
under the same rules (ADR-0055). Declare each server in the project:

```text
# .forge/mcp/everything.conf        (the server name is the file stem: [a-z0-9-])
version=1
executable=/home/you/.local/share/mise/installs/node/26.10.0/bin/node
argument=/path/to/@modelcontextprotocol/server-everything/dist/index.js
argument=stdio
env.LOG_LEVEL=warn             # literal; the server gets a cleared environment
pass_env=GITHUB_TOKEN          # copied from the gateway's environment (keep secrets out of files)
timeout_ms=20000               # per call; default 30 s, maximum 10 min
tool.echo=read_repository      # expose `echo`, requiring read_repository
tool.get-sum=read_repository
```

- **Unmapped means invisible.** Only tools with a `tool.` line are ever listed or callable. The
  reference server offers 13 tools, including `get-env`, which prints its environment; unless it is
  mapped, an agent never sees it.
- Mapped tools appear as `<server>__<tool>` (for example `everything__get-sum`), with the server's
  own description (prefixed with `[server]`) and input schema. Each needs `use_mcp_tools` plus the
  mapped capability, checked on list and on call.
- Calls are forwarded unchanged. An upstream error, a timeout, or a crash becomes an `isError`
  result, and a server that cannot start is skipped (reason on stderr), while the rest keep working.
- Every call is recorded as `ToolInvoked`, with `server`, `upstream_tool`, and `outcome` (`ok`,
  `error`, or `timeout`) added.
- Servers start when the gateway's session initializes, and only with `--root`.

Verified with the real `@modelcontextprotocol/server-everything` (2026.8.31, protocol 2025-06-18):
- only `everything__echo` and `everything__get-sum` were listed;
- `get-sum` returned "The sum of 19 and 23 is 42";
- `get-env` was refused as unknown;
- a Claude Code session called `everything__get-sum` through the gateway ("The sum of 1234 and
  5678 is 6912").

Every call was audited.

## Not yet

- Remote (HTTP or SSE) MCP servers, and upstream resources, prompts, or sampling.
- No network transport for the gateway itself: it is always a child of the agent's MCP client.
