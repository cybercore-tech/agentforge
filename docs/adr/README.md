# AgentForge Architecture Decision Records

ADRs are permanent numbered records of consequential design decisions.

## Registry

| ID | Status | Decision |
| --- | --- | --- |
| ADR-0001 | Accepted | Model-agnostic orchestration |
| ADR-0002 | Accepted | Git worktrees as task isolation boundary |
| ADR-0003 | Accepted | Durable project state outside model memory |
| ADR-0004 | Accepted | Plan-first implementation workflow |
| ADR-0005 | Accepted | Repair-forward failure policy |
| ADR-0006 | Accepted | Role-based agent governance |
| ADR-0007 | Accepted | Capabilities are independent of roles |
| ADR-0008 | Accepted | Versioned structured task and result contracts |
| ADR-0009 | Accepted | Versioned task-state snapshots behind a storage boundary |
| ADR-0010 | Accepted | Deterministic managed worktree lifecycle |
| ADR-0011 | Accepted | Provider-neutral agent adapter execution boundary |
| ADR-0012 | Accepted | Explicit gate definitions and structured process evidence |
| ADR-0013 | Accepted | Read-only CI observation and conservative failure classification |
| ADR-0014 | Accepted | Append-only event audit log with integrity chaining |
| ADR-0015 | Accepted | Explicit least-privilege capability policy |
| ADR-0016 | Accepted | Read-only deterministic doctor and status diagnostics |
| ADR-0017 | Accepted | Explicit single-agent vertical slice |
| ADR-0018 | Accepted | Deterministic multi-agent scheduling and serialized integration |
| ADR-0019 | Accepted | Explicit durable single-task orchestration loop |
| ADR-0020 | Accepted | Durable project blueprint and task intake |
| ADR-0021 | Accepted | Read-only operator HUD |
| ADR-0022 | Accepted | Cooked-mode interactive HUD watch |
| ADR-0023 | Accepted | Controlled operator actions |
| ADR-0024 | Accepted | Cooked-mode interactive foreground agent sessions |
| ADR-0025 | Accepted | PTY-backed foreground agent sessions |
| ADR-0026 | Accepted | Safe review and integration workflow |
| ADR-0027 | Accepted | Foreground real-project orchestration pilot |
| ADR-0028 | Accepted | Daemon task-launch parity |
| ADR-0029 | Accepted | Static GitHub Pages product surface |
| ADR-0030 | Accepted | Use a bounded native dialog for the public README reader |
| ADR-0031 | Accepted | Keep registry identity explicit before crates.io publication |
| ADR-0032 | Accepted | Use `agentforge-platform` for the future end-user Cargo package |
| ADR-0033 | Accepted | Prepare package metadata without publishing the private workspace |
| ADR-0034 | Accepted | Transport-neutral remote-worker lease boundary |
| ADR-0035 | Accepted | Durable remote-worker lease state |
| ADR-0036 | Accepted | Deterministic local remote-dispatch planning |
| ADR-0037 | Accepted | Orchestrated gate evidence |
| ADR-0038 | Accepted | Operator CI observation with recorded classification |
| ADR-0039 | Accepted | Concurrent batch launch with a single state coordinator |
| ADR-0040 | Accepted | Keepalive frames and a single execution slot for daemon executions |
| ADR-0041 | Accepted | cybercore-tech/agentforge is the canonical repository |
| ADR-0042 | Accepted | The site's updates feed is generated from milestone records |
| ADR-0043 | Accepted | Real agents run through a bridge that owns path verification and commits |
| ADR-0044 | Accepted | Milestone tags mark closure commits; release tags stay separate |
| ADR-0045 | Accepted | Post-review approvals are bound to the reviewed commit |
| ADR-0046 | Accepted | Operator lease operations |
| ADR-0047 | Accepted | Coordinated audit appends |
| ADR-0048 | Accepted | Same-host worker process |
| ADR-0049 | Accepted | Opt-in automatic dispatch |
| ADR-0050 | Accepted | Remote-worker channel over GhostPort |
| ADR-0051 | Accepted | Remote execution and exact-SHA import |
| ADR-0052 | Accepted | MCP task-tool gateway |
| ADR-0053 | Accepted | Keyless build provenance for releases |
