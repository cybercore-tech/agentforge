# Plan: P3-M005 — MCP task-tool gateway

Status: Complete
Milestone: P3-M005
Created: 2026-09-25
Owner: AgentForge project

## Goal

An agent working on an AgentForge task can call AgentForge itself through MCP (the Model Context
Protocol), under the task's capability policy. `forge mcp serve` is a stdio MCP server bound to
one task contract and one worktree. It offers three task tools:

- `task_contract`: the task's goal, scope, gates, capabilities, and approvals;
- `check_changes`: every changed path in the worktree, each judged by the policy engine;
- `run_gate`: one of the task's required gates, run in the worktree.

It lists only the tools the task's capabilities allow, re-checks every call, and records every call
(allowed or denied) in the project's audit log when it has one. The Claude Code bridge wires it in
for tasks that hold `use_mcp_tools`.

## Operator decisions (2026-09-25)

- Scope: native task tools first; proxying external MCP servers is a later milestone.
- Dependencies: `serde` and `serde_json` are approved for JSON-RPC (AGENTS.md agent boundaries).

## Non-goals

- No proxying of external MCP servers (a later P3 milestone, reusing this milestone's policy
  mapping and audit event).
- No network transport: stdio only, spawned by the agent's MCP client.
- No new authority: a tool never grants a capability, records an approval, commits, or changes task
  state. `run_gate` results are advisory; the orchestrator's gates after the run stay
  authoritative.
- No MCP resources, prompts, sampling, or elicitation; tools only.

## Context

`Capability::UseMcpTools` has existed in the task model since P0 but nothing honours it. Agents
check their own work with project-specific shell access. The bridge allows
`Bash(./scripts/gate.sh *)`, which only fits this repository, and an agent learns about a boundary
violation only after it finishes. A gateway lets any project's agent ask AgentForge directly: "what
am I allowed to change, is my change inside that, and do my gates pass", with each answer decided
by the same policy engine the orchestrator uses, and every call on the record.

## Architecture placement

- **New crate `agentforge-mcp`** (library; deps: core, policy, adapter, gate, audit, operator,
  serde, serde_json):
  - `Gateway::new(task: AgentTask, worktree, root: Option<PathBuf>)`, where the task comes from
    `parse_task_prompt` of the same `agentforge-task-prompt-v1` document the agent receives;
  - `Gateway::serve(reader, writer)`: newline-delimited JSON-RPC 2.0 over any `BufRead`/`Write`,
    so tests drive it in memory. Messages are bounded at 1 MiB; stdout carries only protocol
    messages, and diagnostics go to stderr;
  - protocol: `initialize` (version negotiation over `2025-11-25`, `2025-06-18`, `2025-03-26`,
    and `2024-11-05`; capabilities `{tools: {listChanged: false}}`; server info),
    `notifications/initialized`, `ping`, `tools/list`, and `tools/call`. Standard errors: parse
    error `-32700`, invalid request `-32600` (including batches), method not found `-32601`, and
    invalid params `-32602` (including an unknown or unlisted tool);
  - **tool policy map**, each checked with `PolicyEngine::evaluate`:

    | Tool | Requires | Notes |
    | --- | --- | --- |
    | `task_contract` | `use_mcp_tools`, `read_repository` | structured and text content |
    | `check_changes` | `use_mcp_tools`, `read_repository` | `git status` of the worktree; each path judged as a `write_owned_paths` request (scope, forbidden paths) |
    | `run_gate` | `use_mcp_tools`, `run_local_commands` | `gate` must be one of the task's `required_gates` and have a project gate profile; runs in the worktree; bounded output tail |

    `tools/list` lists a tool only when its policy checks pass (and, for `run_gate`, when a root
    with gate profiles is known). `tools/call` evaluates again, so a denied call is refused with
    `isError: true` and the policy's reason.
  - **audit:** with `root` given and the project's task snapshot present, each call appends
    `ToolInvoked` (new kind, code 13) through `open_project_audit`: actor `mcp:<task>`, the task
    ID, `tool`, `decision` (`allowed` or `denied`), `reason`, `channel=mcp`, and for `run_gate`
    the `gate` and `outcome`. Without a root (a remote worker's clone has no project state),
    calls are logged to stderr only; this is documented.
- **`agentforge-audit`:** `AuditEventKind::ToolInvoked = 13`.
- **CLI:** `forge mcp serve --contract <file> --worktree <dir> [--root <project>]`.
- **Bridge (`scripts/agents/claude-code-bridge`):** for a task with `use_mcp_tools`, it writes the
  contract and an MCP config to its scratch directory and runs `claude` with `--mcp-config <file>
  --strict-mcp-config` and `mcp__agentforge` in the allowed tools. `forge` comes from a new
  `--forge <path>` argument or `PATH`; if neither resolves, the bridge refuses (exit 2), because
  profiles run with a cleared environment. The project root is the worktree's
  `git rev-parse --git-common-dir` parent, passed only when it has a task snapshot. Tasks without
  `use_mcp_tools` run exactly as today.
- **ADR-0052** records the gateway's authority model: task-bound, policy-mapped, advisory,
  audited, and stdio-only.

## Invariants

- A tool call never exceeds the task's authority. Listing and calling both go through the policy
  engine, and a denial is returned and recorded, never ignored.
- The gateway never changes task state, approvals, or Git history. `run_gate` runs only a gate the
  task requires, and only in the task's worktree.
- stdout carries nothing but MCP messages.

## ADRs

ADR-0052 (new): MCP task-tool gateway. Registry row and README docs-map link (enforced by `xtask
validate`).

## Public API / CLI

The `agentforge-mcp` crate (`Gateway`, the tool map), `AuditEventKind::ToolInvoked`, `forge mcp
serve`, and the bridge's `--forge` argument.

## Compatibility analysis

`ToolInvoked` is a new audit kind (code 13). An older `forge` cannot read an audit log containing
it, the same as when `LeaseRecorded` was introduced. It appears only in projects whose agents use
the gateway. Bridge behaviour is unchanged for tasks without `use_mcp_tools`.

## Dependency analysis

`serde` (with `derive`) and `serde_json`, approved by the operator on 2026-09-25, used only by
`agentforge-mcp`. Both are MIT/Apache-2.0 and compatible with MSRV 1.85; the lockfile gains them
and their small transitive set (`serde_derive`, `itoa`, `ryu`, `memchr`).

## Expected file boundary

- `.plans/P3-M005-mcp-task-tool-gateway.plan.md`, `.plans/ACTIVE`
- `Cargo.toml`, `Cargo.lock`
- `crates/agentforge-mcp/**`
- `crates/agentforge-audit/src/lib.rs`, `crates/agentforge-audit/tests/*.rs`
- `crates/agentforge-cli/Cargo.toml`, `crates/agentforge-cli/src/main.rs`,
  `crates/agentforge-cli/tests/*.rs`
- `scripts/agents/claude-code-bridge`
- `docs/MCP_GATEWAY.md`, `docs/adr/ADR-0052-mcp-task-tool-gateway.md`, `docs/adr/README.md`,
  `docs/AUDIT.md`, `docs/AGENT_PROFILES.md`, `docs/OPERATIONS.md`
- closure records: `docs/MILESTONES.md`, `CHANGELOG.md`, `README.md`, `PROJECT_STATE.md`,
  `AGENT_HANDOFF.md`

## Test-first matrix

- **Protocol (in memory):** initialize with each supported version and with an unknown one (answers
  the latest); `notifications/initialized` has no response; ping; unknown method; parse error;
  batch refused; oversized line refused; requests before `initialize` refused.
- **Policy map:** a task with all capabilities lists all three tools; without `use_mcp_tools`,
  none; without `run_local_commands`, no `run_gate`; calling an unlisted tool is `-32602` and is
  audited as denied.
- **Tools:** `task_contract` round-trips the contract; `check_changes` reports an in-scope change
  as allowed and an out-of-scope or forbidden one as denied with the policy's reason;
  `run_gate` runs a passing and a failing fixture gate and refuses a gate the task does not
  require.
- **Audit:** allowed and denied calls append `ToolInvoked` with the fields above; without a root,
  nothing is written; the audit kind round-trips and verifies.
- **CLI:** `forge mcp serve` over real pipes: initialize, `tools/list`, and a `tools/call`, with
  stdout parsed line by line as JSON.
- **Bridge:** its `--self-test` covers the command with and without `use_mcp_tools`, and a missing
  `forge`.
- **Real client:** Claude Code (headless) connects to `forge mcp serve` through the bridge's config
  and calls `task_contract` and `check_changes`; the audit log shows the calls.

## Implementation sequence

1. Commit this draft plan.
2. Approve and activate it in a separate checkpoint.
3. Dependencies and crate skeleton; protocol with tests; the tool map and tools with tests; audit
   kind; CLI; bridge; ADR and docs; the real-client check.
4. Gate; commit (exit checked); push; CI plus a repeat.
5. Close; tag after the closure CI is green and the dry run names the "close ..." commit.

## Failure modes

- A client speaks a newer protocol version: the server answers with its latest supported version,
  per the specification, and the client decides.
- An agent floods calls: each call is bounded (message size, gate timeout from its profile) and
  audited. Rate limiting is out of scope.
- The audit log is busy: appends are coordinated (P0-M013), and a failure to record fails the call
  (fail closed) instead of running it unrecorded.

## Documentation impact

MCP_GATEWAY (new: tools, policy map, setup, audit), ADR-0052, AUDIT (the new kind), AGENT_PROFILES
(the bridge's MCP wiring and `--forge`), and OPERATIONS (command and recovery rows). README and
CHANGELOG at closure.

## Quality gates

- `./scripts/gate.sh full`, and the bridge `--self-test`;
- push CI plus a dispatched repeat.

## Acceptance criteria

- [x] `forge mcp serve` speaks MCP over stdio and serves the three task tools under the policy map.
- [x] Every call is policy-checked and audited when the project is known.
- [x] The bridge wires the gateway for `use_mcp_tools` tasks; a real Claude Code session uses it.
- [x] ADR, docs; CI evidence; closed and tagged correctly.

## Completion record

Implementation commit: `9e4cf6f`
CI run: `36143255345` (push) and `36143670594` (dispatched repeat)
CI result: green on all seven jobs in both runs
Completed: 2026-09-25
Notes:
- **Real client.** A Claude Code session, launched through `forge run` and the bridge on a
  throwaway project, called `task_contract`, `check_changes` (1 changed, 0 denied), and `run_gate`
  (passed). It created the file within scope, and the bridge committed it. The orchestrator's
  gate passed 1/1, and the calls are `ToolInvoked` #1 to #3 in that project's audit log. The run
  took 18 s.
- The tool calls have lower sequence numbers than the run's `AgentStarted`, because the
  orchestrator persists a run's events in one batch when it finishes. The timestamps show the true
  order. This is documented in MCP_GATEWAY.md.
- **Dependencies.** The lockfile gained `serde`, `serde_core`, `serde_derive`, `serde_json`,
  `itoa`, `memchr`, and `zmij`. The plan listed `ryu`; current `serde_json` uses `zmij` instead.
  All have MSRV 1.71 or lower and permissive licenses.
- **Tests.** Mutation checks: listing without the policy check, calling without it, and allowed
  calls going unrecorded each fail the suite.
- A dry-run helper briefly staged a `__pycache__` file from importing the bridge as a module. It
  was removed before commit.
