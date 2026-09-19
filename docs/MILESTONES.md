# AgentForge Milestones

Milestone IDs are permanent and are never recycled.

Status values are `planned`, `active`, `complete`, `blocked`, `split`, and `superseded`.

## Phase 0 — reliable orchestration foundation

| ID | Status | Milestone | Acceptance signal |
| --- | --- | --- | --- |
| P0-M001 | active | Repository bootstrap | Workspace, control files, CLI/daemon/core skeletons, and bootstrap gate exist. |
| P0-M002 | planned | Governance and agent contract | Roles, permissions, review rules, and change policy are explicit. |
| P0-M003 | planned | Plan-first workflow enforcement | Active plans, hooks, and checkpoint validation are enforced. |
| P0-M004 | planned | Task graph and durable state | Tasks and dependencies persist locally with deterministic IDs. |
| P0-M005 | planned | Worktree isolation manager | Tasks can create, inspect, and retire isolated Git worktrees safely. |
| P0-M006 | planned | Agent adapter interface | External coding agents can run behind one stable adapter contract. |
| P0-M007 | planned | Gate engine | Project format, lint, test, build, docs, and policy gates run uniformly. |
| P0-M008 | planned | CI monitor and failure classifier | Exact runs are observed and failures are classified before repair. |
| P0-M009 | planned | Event and audit log | Orchestration decisions and task transitions are durably recorded. |
| P0-M010 | planned | Capability and permission policy | Agents receive explicit least-privilege capabilities. |
| P0-M011 | planned | Doctor and status diagnostics | Operators can inspect environment, project, agents, worktrees, and blockers. |
| P0-M012 | planned | Single-agent vertical slice | One approved task flows through worktree, agent, gates, review handoff, and audit evidence. |

## Future phase reservations

- `P1-*` — multi-agent scheduling and integration;
- `P2-*` — TUI/HUD and operator experience;
- `P3-*` — MCP/tool gateway and external systems;
- `P4-*` — remote workers and distributed execution;
- `P5-*` — release/deployment orchestration.
