# AgentForge Milestones

Milestone IDs are permanent and are never recycled.

Status values are `planned`, `active`, `complete`, `blocked`, `split`, and `superseded`.

## Phase 0 — reliable orchestration foundation

| ID | Status | Milestone | Acceptance signal |
| --- | --- | --- | --- |
| P0-M001 | complete | Repository bootstrap | Workspace, control files, CLI/daemon/core skeletons, and bootstrap gate exist. |
| P0-M002 | complete | Governance and agent contract | Roles, permissions, review rules, and change policy are explicit. |
| P0-M003 | complete | Plan-first workflow enforcement | Active plans, hooks, checkpoint validation, and remote CI are enforced. |
| P0-M004 | complete | Task graph and durable state | Tasks and dependencies persist locally with deterministic IDs. |
| P0-M005 | complete | Worktree isolation manager | Tasks can create, inspect, and retire isolated Git worktrees safely. |
| P0-M006 | complete | Agent adapter interface | External coding agents can run behind one stable adapter contract. |
| P0-M007 | complete | Gate engine | Explicit local checks run with bounded, structured evidence. |
| P0-M008 | complete | CI monitor and failure classifier | Exact runs are observed and failures are classified before repair. |
| P0-M009 | complete | Event and audit log | Orchestration decisions and task transitions are durably recorded. |
| P0-M010 | complete | Capability and permission policy | Agents receive explicit least-privilege capabilities. |
| P0-M011 | complete | Doctor and status diagnostics | Operators can inspect environment, project, agents, worktrees, and blockers. |
| P0-M012 | complete | Single-agent vertical slice | One approved task flows through worktree, agent, gates, review handoff, and audit evidence. |

## Future phase reservations

- `P1-*` — multi-agent scheduling and integration;
- `P2-*` — TUI/HUD and operator experience;
- `P3-*` — MCP/tool gateway and external systems;
- `P4-*` — remote workers and distributed execution;
- `P5-*` — release/deployment orchestration.

## Phase 2 — operator experience

| ID | Status | Milestone | Acceptance signal |
| --- | --- | --- | --- |
| P2-M001 | complete | Read-only operator HUD | Operators can inspect blueprint, task, audit, and worktree state through a deterministic read-only report. |
| P2-M002 | complete | Interactive operator HUD | Operators can run a bounded live HUD, refresh it, inspect recoverable source failures, and exit without mutation. |
| P2-M003 | complete | Controlled operator actions | Operators can inspect tasks, record explicit approvals, and apply audited lifecycle decisions without mutating through the HUD. |
| P2-M004 | complete | Release readiness | The project has explicit licensing/versioning, reproducible tagged artifacts, and cross-platform CI coverage. |
| P2-M005 | complete | Local daemon and dogfooding | Operators can run one approved task through a bounded local daemon with durable failure evidence and a real end-to-end workflow. |
| P2-M006 | complete | Real-agent dogfooding and operator workflow | Operators can prepare managed worktrees, run a configured executable through the daemon, inspect recovery evidence, and retire clean worktrees safely. |
| P2-M007 | complete | Cross-platform agent operations | Windows worktrees, bounded local agent profiles, and cooperative daemon supervision are available with exact CI evidence. |
| P2-M008 | complete | Windows dogfooding parity | Managed worktree and daemon CLI dogfooding suites pass on Linux, macOS, and Windows with direct portable fixtures. |
| P2-M009 | complete | Daemon audit-sequence continuation | Persisted daemon runs append contiguous, integrity-linked audit events after existing project history. |
| P2-M010 | complete | Guided project intake surface | Operators can author validated blueprint/guideline documents and optional task drafts through a bounded preview-confirm workflow. |
| P2-M011 | complete | Guided intake operator hardening | Operators can replay bounded intake sessions and verify intake-to-task/HUD handoff on disposable projects. |
| P2-M012 | complete | Interactive foreground agent sessions | Operators can interact with a direct agent run in the current terminal while preserving AgentForge execution and acceptance boundaries. |

## Phase 1 — multi-agent scheduling and integration

| ID | Status | Milestone | Acceptance signal |
| --- | --- | --- | --- |
| P1-M001 | complete | Multi-agent scheduling and serialized integration | Runnable batches are deterministic, disjoint work can run concurrently, and integration is serialized. |
| P1-M002 | complete | Single-task orchestration loop | One task runs through durable state, policy, worktree, adapter, gates, audit, and review handoff. |
| P1-M003 | complete | Project blueprint and task intake | Operators can author validated project guidance and explicit task contracts through durable files and CLI commands. |
