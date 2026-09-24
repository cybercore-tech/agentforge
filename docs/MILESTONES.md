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
| P2-M013 | complete | PTY-backed foreground agent sessions | Operators can run terminal-native full-screen agents through an explicit PTY mode with bounded evidence and restored terminal state. |
| P2-M014 | complete | Safe review and integration workflow | Operators can inspect bounded task diffs and perform approved, serialized fast-forward-only integration while preserving branches and worktrees. |
| P2-M015 | complete | Real-project orchestration pilot | Operators can launch one ready task through a foreground command that prepares its managed worktree and preserves explicit review, acceptance, integration, and retirement boundaries. |
| P2-M016 | complete | Daemon task-launch parity | Operators can launch one ready detached task through a daemon command that prepares or verifies its managed worktree while preserving explicit review, acceptance, integration, and retirement. |
| P2-M017 | complete | AgentForge GitHub Pages | Visitors can understand the local-first workflow through a responsive, accessible static site deployed from `main` with bounded Pages permissions. |
| P2-M018 | complete | README hardening and in-page reader | README warnings and operator guidance match the current alpha product, the GitHub About link points to Pages, and the public “Read the README” control opens an accessible local dialog with a canonical full-document link. |
| P2-M019 | complete | Registry identity and distribution policy | Existing crates.io collisions are documented, binary distribution remains authoritative, and future package publication is gated behind a separate naming plan. |
| P2-M020 | complete | AgentForge platform package identity | The future end-user Cargo package is named `agentforge-platform` while binaries, internal crates, and GitHub release artifacts remain compatible. |
| P2-M021 | complete | Windows daemon CI reliability | Spawned and foreground daemon lifecycle tests are isolated and the exact cross-platform CI matrix is green. |
| P2-M022 | complete | Cargo publishability preparation | The future platform package has registry-aware metadata and an offline archive preflight while all crates remain private. |
| P2-M023 | complete | Bounded daemon lifecycle tests and CI job timeouts | Daemon lifecycle tests cannot block without bound, a stalled client cannot wedge the daemon, every CI job has a timeout, and the exact seven-job matrix is green. |

## Phase 3 — external systems and Mission Control

| ID | Status | Milestone | Acceptance signal |
| --- | --- | --- | --- |
| P3-M001 | complete | Cybercore Mission Control foundation | A private Cloudflare Worker control plane persists projects, agents, heartbeats, and audit events, exposes a tested observation-only API, and renders a durable/demo dashboard. |
| P3-M002 | complete | Rust local connector | A scoped local connector sends bounded, replay-resistant heartbeats outbound without granting the cloud service local execution authority. |
| P3-M003 | complete | Cybercore connector release hardening | The connector has provenance-aware CLI output, supported-platform CI, inspected packaging, license/support policy, and a tag-gated checksummed release workflow without cloud deployment side effects. |
| P3-M004 | complete | Mission Control production-readiness foundations | Fail-closed environment preflight, disposable migration/API/live-event smoke, negative-path coverage, and an explicitly gated staging dry-run workflow exist without production mutation. |

## Phase 4 — remote workers and distributed execution

| ID | Status | Milestone | Acceptance signal |
| --- | --- | --- | --- |
| P4-M001 | complete | Remote worker contract and lease foundation | A versioned, transport-neutral worker descriptor and deterministic fail-closed task-lease state machine exist without network or remote execution authority. |
| P4-M002 | complete | Durable remote-worker lease state | A separate checksummed lease snapshot restores ownership, generations, expiry, and terminal state across restart, with explicit caller-supplied expiry recovery and no transport authority. |
| P4-M003 | complete | Deterministic remote dispatch planning | Ready tasks can be assigned to supplied workers in canonical order with path, capacity, lease-generation, and all-or-nothing mutation guarantees, without transport or execution authority. |

## Phase 1 — multi-agent scheduling and integration

| ID | Status | Milestone | Acceptance signal |
| --- | --- | --- | --- |
| P1-M001 | complete | Multi-agent scheduling and serialized integration | Runnable batches are deterministic, disjoint work can run concurrently, and integration is serialized. |
| P1-M002 | complete | Single-task orchestration loop | One task runs through durable state, policy, worktree, adapter, gates, audit, and review handoff. |
| P1-M003 | complete | Project blueprint and task intake | Operators can author validated project guidance and explicit task contracts through durable files and CLI commands. |
| P1-M004 | complete | Orchestrated gate evidence | Configured project gates run automatically after a successful agent in every run/launch path, record durable `GateFinished` evidence, and fail the task when any gate fails. |
| P1-M005 | active | CI observation wiring | Operators record exact-SHA CI evidence and classified failed jobs in the audit log through a reviewed provider command, without CI gaining task authority. |
