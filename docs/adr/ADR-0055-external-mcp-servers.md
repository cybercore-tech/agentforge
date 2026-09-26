# ADR-0055: External MCP servers behind the gateway

- Status: Accepted
- Date: 2026-09-25
- Milestone: P3-M006

## Context

Agents often need tools that AgentForge does not provide, such as issue trackers, documentation
search, or a filesystem or database server, and most are available as MCP servers. Wiring those
straight into the agent's MCP client bypasses AgentForge: there is no capability check and no audit,
and every tool the server offers is reachable, including ones like "print the environment". The
task-tool gateway (ADR-0052) already has the authority model; this decision extends it outward.

## Decision

- **Declared, not discovered.** A project declares each server in `.forge/mcp/<server>.conf`: an
  absolute executable, literal arguments, a literal environment (the child environment is cleared),
  `pass_env` for named secrets from the gateway's environment, and a per-call timeout.
- **Unmapped means invisible.** Each exposed tool has a `tool.<name>=<capability>` line. A tool the
  server offers but the declaration does not map is never listed and never callable.
- **The same policy and audit.** An upstream tool is listed as `<server>__<tool>` and callable only
  when the task holds `use_mcp_tools` plus the mapped capability, checked with the policy engine on
  list and on call. Every call, allowed or denied, is a `ToolInvoked` audit event, with `server`,
  `upstream_tool`, and `outcome` (`ok`, `error`, or `timeout`), and the same fail-closed recording.
- **Forwarded unchanged, contained.** Results pass through as the server produced them. An upstream
  error, a timeout, a crash, malformed output, or a failure to start becomes an `isError` result or
  a skipped server; it never hangs or crashes the gateway. Server-initiated requests are not
  answers: the gateway answers their `ping` and refuses the rest.
- **stdio only**, started when the gateway session initializes and stopped when it ends.

## Consequences

- A project can give agents third-party tools with least privilege and an audit trail, per tool.
- Operators maintain the tool map. A new upstream tool stays invisible until it is mapped.
- Remote (HTTP) MCP servers, resources, and prompts are out of scope for now.

## References

- `docs/MCP_GATEWAY.md`
- `.plans/P3-M006-external-mcp-servers.plan.md`
- ADR-0052 (MCP task-tool gateway), ADR-0015 (capability policy)
