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
| P0-M013 | complete | Coordinated audit appends and first-use audit logs | Concurrent writers (threads, processes, daemon executions) cannot corrupt the audit chain, attempt logs are appended atomically, and a fresh project can record its first operator action. |
| P0-M014 | complete | EINTR-safe pipe readers (remote real-agent dogfood) | All pipe and stdin read loops retry on `Interrupted`; implemented by Claude Code as a remote worker through GhostPort, imported at its exact SHA, gated on the coordinator, and integrated. |

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
| P2-M024 | complete | Daemon long-running requests | Daemon run/launch executions longer than the control timeout succeed, status stays available during them, overlapping executions and stop are refused, and no client error suggests removing live metadata. |
| P2-M025 | complete | Canonical repository identity | Repository metadata, README, CHANGELOG, registry docs, and the Pages site point at `cybercore-tech/agentforge`, and Pages deploys from the new repository. |
| P2-M026 | complete | Project site updates feed | The project site shows recently shipped milestones, work in progress, phase progress, and unreleased features, generated from repository records at deploy time and validated in CI. |
| P2-M027 | complete | Real-agent bridge and dogfooding | A real coding agent completes an AgentForge milestone through `forge task launch`, with path boundaries enforced before commit and repository gates on the agent's commit. |
| P2-M028 | complete | Hermetic repository gate under hooks and in worktrees | The gate clears hook Git variables before cargo steps and builds linked worktrees into their own target directory, proven by reproducing the incident before and after the fix. |
| P2-M029 | complete | Agent run visibility and build isolation | Every agent run shows its exit code, keeps its full output as audited evidence, and fails the CLI when the agent fails; agent builds in worktrees are isolated; an operations reference exists. |
| P2-M030 | complete | Milestone tags | Every completed milestone has an annotated `milestone/<ID>` tag on its closure commit, tagging is part of closure, and release tags stay separate. |
| P2-M031 | complete | Tagger closure detection fix | Milestone tags are created only on commits whose plan status line reads `Status: Complete`, verified before tagging; all earlier tags unchanged. |
| P2-M032 | complete | macOS daemon stop flake | `forge daemon stop` treats macOS EINVAL on a reset socket as transport loss, with repeated green CI on macOS. |
| P2-M033 | complete | HUD view of agent runs | `forge hud` shows recent agent runs with exit status, gate results, and evidence paths; built by Claude Code and landed through `forge task integrate`. |
| P2-M034 | complete | Closure and documentation-index integrity | The tagger decides only from committed history, refuses uncommitted or unclosed milestones, and keeps every existing tag; `xtask validate` enforces a complete ADR registry and README documentation map in CI. |

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
| P4-M004 | complete | Remote-worker leases in the CLI and daemon | Operators register workers and grant, renew, release, and expire audited leases from `forge`; leased tasks and tasks overlapping them cannot run locally; `forged` expires due leases without colliding with executions. |
| P4-M005 | complete | Same-host worker process | `forge worker run` claims its leases, runs each task through the standard launch path with renewals, and releases it; only the exact lease holder can run a leased task. |
| P4-M006 | complete | Opt-in automatic dispatch | With an enabled, milestone-scoped `.forge/dispatch.conf`, `forged` and `forge lease dispatch` grant ready, approved tasks to registered workers once per task, bounded by capacity and a per-tick limit, audited as `dispatch=auto`. |
| P4-M007 | complete | Authenticated remote-worker channel over GhostPort | Enrolled workers on other machines claim, renew, and release their own leases and receive the exact contract and base commit through `forged`'s loopback worker API, carried by a GhostPort Noise KK tunnel; verified end to end with real GhostPort processes. |
| P4-M008 | complete | Remote execution and exact-SHA import | A worker on another machine runs its claimed task and returns a git bundle; the coordinator imports it only at the verified exact SHA with in-bounds paths, runs gates locally, and hands it to normal review through `integrate`. |
| P4-M009 | complete | Worker-host doctor | `forge worker remote doctor` checks clone, identity, hooks, profile paths, secret, and an authenticated `PING`; `worker remote run` refuses on any failing check; remote renewals are reported. |

## Phase 1 — multi-agent scheduling and integration

| ID | Status | Milestone | Acceptance signal |
| --- | --- | --- | --- |
| P1-M001 | complete | Multi-agent scheduling and serialized integration | Runnable batches are deterministic, disjoint work can run concurrently, and integration is serialized. |
| P1-M002 | complete | Single-task orchestration loop | One task runs through durable state, policy, worktree, adapter, gates, audit, and review handoff. |
| P1-M003 | complete | Project blueprint and task intake | Operators can author validated project guidance and explicit task contracts through durable files and CLI commands. |
| P1-M004 | complete | Orchestrated gate evidence | Configured project gates run automatically after a successful agent in every run/launch path, record durable `GateFinished` evidence, and fail the task when any gate fails. |
| P1-M005 | complete | CI observation wiring | Operators record exact-SHA CI evidence and classified failed jobs in the audit log through a reviewed provider command, without CI gaining task authority. |
| P1-M006 | complete | Concurrent batch launch | Disjoint ready tasks launch concurrently from one command with a single state coordinator; overlapping and over-limit tasks are deferred deterministically. |
| P1-M007 | complete | Task-declared required gates | Tasks run exactly their declared gates, missing gate profiles fail preflight before any side effect, and blueprint defaults need a configured profile; implemented by a real agent through AgentForge. |
| P1-M008 | complete | Post-review approvals bound to the reviewed commit | Agents launch without merge/release/deploy approvals; those approvals are recorded only after accept, bound to the reviewed head, and `integrate` merges exactly that commit. |

## Phase 5 — release and deployment orchestration

| ID | Status | Milestone | Acceptance signal |
| --- | --- | --- | --- |
| P5-M001 | complete | First release v0.1.0 | `v0.1.0` is published from green CI with four checksummed archives, and version metadata, CHANGELOG, and docs agree on `0.1.0`. |
| P5-M002 | complete | Release v0.2.0 | `v0.2.0` (remote workers, the audit-corruption fix) is published with four checksummed archives and a directly verifiable `SHA256SUMS`; the publish job uploads only release files, and dispatched dry runs exercise it. |
