# Plan: P3-M006 — External MCP servers behind the gateway

Status: Complete
Milestone: P3-M006
Created: 2026-09-25
Owner: AgentForge project

## Goal

The MCP task-tool gateway (P3-M005, ADR-0052) can front **external MCP servers**, such as a
GitHub or filesystem server, under the same rules as its own tools. A project declares each server
and the tools it exposes, with the capability each one requires. The gateway starts the servers,
lists only the mapped tools the task's capabilities allow, forwards calls, and records every call
(allowed or denied) as `ToolInvoked`. Unmapped tools are never listed or callable.

## Operator decision (2026-09-25)

The operator asked for "external MCP servers behind the gateway (P3)", the follow-up that P3-M005
reserved.

## Non-goals

- Only stdio servers (the transport the gateway itself speaks); no HTTP or SSE upstreams.
- Tools only: no resources, prompts, sampling, or elicitation from upstreams, and upstream
  notifications are not forwarded.
- No automatic discovery: every server and every exposed tool is declared by the operator.
- No secret store: secrets come from the gateway's own environment, and only those named in the
  config are passed through.

## Context

`Capability::UseMcpTools` plus a per-tool capability is already the gateway's authority model.
Agents often need external tools (issue trackers, docs search). Wiring such servers straight into
the agent's MCP client bypasses AgentForge's policy and audit; behind the gateway, they get both.
A real reference server (`@modelcontextprotocol/server-everything` 2.0.0, stdio, protocol
2025-06-18) runs locally with 13 tools, including `echo`, `get-sum`, and `get-env` (which returns
the server's environment: exactly the kind of tool that must stay unexposed unless mapped).

## Architecture placement

- **Config: `.forge/mcp/<server>.conf`**, in the bounded `key=value` format of the other profiles:

  ```text
  version=1
  executable=/absolute/path            # the server program
  argument=...                         # repeatable, literal
  env.NAME=value                       # literal environment (the child env is cleared)
  pass_env=NAME                        # copy NAME from the gateway's environment (for secrets)
  timeout_ms=30000                     # per call; the default is 30 s, the maximum 10 min
  tool.<upstream-tool>=<capability>    # expose this tool, requiring this capability
  ```

  The server name is the file stem (`[a-z0-9-]{1,32}`). Tool names follow MCP's
  `[A-Za-z0-9_-]{1,64}`. The config is validated before any process starts.
- **`agentforge-mcp::upstream`:**
  - `UpstreamServer` spawns the server, runs the client side of `initialize` and
    `notifications/initialized`, and sends requests with increasing ids, reading newline-delimited
    responses until the matching id (notifications are skipped);
  - each request is bounded by the timeout (a reader thread and a channel), and messages by the
    gateway's 1 MiB limit;
  - the server is stopped when the gateway exits (kill plus wait).
- **Gateway:**
  - with `--root`, it loads `.forge/mcp/*.conf` and starts each server at `initialize`. A server
    that fails to start or answer is logged to stderr and skipped, and the others continue;
  - it lists `<server>__<tool>` for each mapped tool that the upstream really offers (the
    upstream's description and input schema are kept, under the prefixed name) and that passes the
    policy engine: `use_mcp_tools` plus the mapped capability;
  - `tools/call` on such a name checks the policy again, forwards `tools/call` with the upstream
    name and arguments, and returns the upstream result (`content`, `isError`, and
    `structuredContent`) unchanged. An upstream JSON-RPC error or a timeout becomes `isError:
    true` with the reason;
  - unmapped or unavailable names are refused with `-32602`, as for native tools;
  - audit: `ToolInvoked` with `tool=<server>__<tool>`, plus `server` and `upstream_tool` fields, and
    `outcome` (`ok`, `error`, or `timeout`).
- **CLI:** unchanged (`forge mcp serve ... --root`). **Bridge:** unchanged (it already wires
  `mcp__agentforge` for `use_mcp_tools` tasks).
- **ADR-0055:** external servers are declared, mapped, policy-checked, and audited, and unmapped
  means invisible.

## Invariants

- An upstream tool is reachable only if the operator mapped it, and only for tasks holding
  `use_mcp_tools` plus the mapped capability.
- Every call through the gateway is audited, with the same fail-closed recording as native tools.
- A misbehaving upstream (crash, garbage, hang) cannot hang or crash the gateway.

## ADRs

ADR-0055 (new) and its registry row.

## Public API / CLI

The `.forge/mcp/<server>.conf` format; the `server` and `upstream_tool` fields on `ToolInvoked`.

## Compatibility analysis

Additive. Without `.forge/mcp/`, the gateway behaves exactly as before.

## Dependency analysis

None (std threads and processes; `serde_json`, already approved for this crate).

## Expected file boundary

- `.plans/P3-M006-external-mcp-servers.plan.md`, `.plans/ACTIVE`
- `crates/agentforge-mcp/**` (including a fixture MCP server binary for tests)
- `crates/agentforge-cli/tests/*.rs`
- `docs/MCP_GATEWAY.md`, `docs/adr/ADR-0055-external-mcp-servers.md`, `docs/adr/README.md`,
  `docs/OPERATIONS.md`
- closure records: `docs/MILESTONES.md`, `CHANGELOG.md`, `README.md`, `PROJECT_STATE.md`,
  `AGENT_HANDOFF.md`

## Test-first matrix

- **Config:** valid, missing executable, relative path, unknown key, bad tool name, unknown
  capability, and repeated `tool.` lines refused, each with the line.
- **Fixture server** (a test binary speaking MCP over stdio, with tools `echo`, `leak`, `slow`,
  `fail`, and `crash`):
  - only mapped tools are listed, prefixed, with their upstream schema;
  - an unmapped upstream tool (`leak`) is never listed, and calling it is `-32602` and audited as
    denied;
  - the capability rule: mapped to `read_github` and the task lacks it, so the tool is not listed
    and a call is denied;
  - a call is forwarded and the result returned unchanged (arguments round-trip);
  - `fail` (upstream `isError`) passes through; an upstream JSON-RPC error becomes `isError`;
  - `slow` beyond `timeout_ms` gives `isError` with `outcome=timeout`, and the gateway stays
    responsive;
  - `crash` kills the server: the call gives `isError`, and native tools still work;
  - a server that fails to start is skipped, and the others are still listed;
  - `pass_env` passes only the named variable, and `env.` values are literal.
- **CLI:** `forge mcp serve` over real pipes with a fixture server configured.
- **Real (local):** `forge mcp serve` fronting `@modelcontextprotocol/server-everything` with
  `echo` and `get-sum` mapped: only those two are listed (never `get-env`), `get-sum` returns the
  right sum, and the calls are audited. Plus a Claude Code session calling one of them through the
  gateway.
- The full gate, and push CI plus a repeat.

## Implementation sequence

1. Commit this draft plan.
2. Approve and activate it in a separate checkpoint.
3. Config and tests; the fixture server; `upstream`; gateway integration; CLI test; real server
   check; ADR and docs.
4. Gate; commit; push; CI plus a repeat.
5. Close; tag after the closure CI is green.

## Failure modes

- An upstream floods output: messages are bounded, and over-limit output ends that server's session.
- An upstream ignores termination: kill plus wait when the gateway exits.

## Documentation impact

MCP_GATEWAY (external servers, the config format, the audit fields), ADR-0055, OPERATIONS; CHANGELOG
at closure.

## Quality gates

- `./scripts/gate.sh full`;
- push CI plus a dispatched repeat.

## Acceptance criteria

- [x] Declared external MCP servers are fronted by the gateway, with only mapped, permitted tools
      visible and callable, and every call audited.
- [x] Misbehaving upstreams (error, timeout, crash, failure to start) are contained.
- [x] A real third-party MCP server works through the gateway, and a Claude Code session uses it.
- [x] ADR, docs; CI evidence; closed and tagged correctly.

## Completion record

Implementation commit: `3939aff`; `de0f0f7` (portable config test paths)
CI run: `36212771840` (push on `3939aff`: the Windows job failed on test paths, see Notes);
`36213033086` (push on `de0f0f7`: one stable-gate flake in an unrelated audit test, see Notes) and
`36213252213` (dispatched repeat on `de0f0f7`)
CI result: green on all seven jobs in `36213252213`; Windows (including the upstream tests) green
from `de0f0f7` on
Completed: 2026-09-25
Notes:
- **Real server:** `forge mcp serve` fronted `@modelcontextprotocol/server-everything` (2026.8.31,
  protocol 2025-06-18) with `echo` and `get-sum` mapped. Only those two were listed. `get-sum`
  returned "The sum of 19 and 23 is 42". The unmapped `get-env` was refused as unknown. All the
  calls were audited. A Claude Code session then called `everything__get-sum` through the gateway
  ("The sum of 1234 and 5678 is 6912", `ToolInvoked` #7).
- **Tests bite:** the first fixture neither required its `ping` to be answered nor refused requests
  before `initialize`, so a mutant without server-request handling passed all 7 upstream tests. With
  the fixture corrected, that mutant fails 6 of 7.
- **Deviation:** the plan's CLI-level fixture test could not reach the fixture binary, because Cargo
  exposes a crate's binaries only to that crate's own tests. The CLI path was proven instead by the
  real-server run above (local, not in CI).
- **CI 1:** the config unit tests used `/bin/x`, which is not absolute on Windows. Classified
  semantic/test and fixed in `de0f0f7`; the product rule is unchanged.
- **CI 2:** `concurrent_writers_keep_one_verified_chain` (P0-M013) failed once on the Linux stable
  gate: one of four writers gave up after the 5 s append-lock wait. It is unrelated to this change,
  passed on the repeat and 5/5 locally, and is recorded as **dogfooding finding 23 (open)**.
