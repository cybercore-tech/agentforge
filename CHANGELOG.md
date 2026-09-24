# Changelog

All notable AgentForge changes are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and release tags follow
[Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- Release-readiness automation, cross-platform validation, and packaged binary artifacts.
- Project quality gates in `.forge/gates/` that run automatically after a successful agent in every
  run and launch path, with `GateFinished` audit evidence and `forge gate list` (P1-M004).
- `forge ci observe` for recording classified exact-SHA CI evidence through a reviewed provider
  profile, plus the `scripts/ci-provider-github` reference provider (P1-M005).
- `forge task launch-batch` for running disjoint ready tasks concurrently under a single state
  coordinator (P1-M006).
- Remote-worker lease contract, durable lease state, and deterministic dispatch planning as
  transport-neutral foundations (P4-M001 to P4-M003).
- A *What's new* section on the project site with recently shipped milestones, work in progress,
  phase progress, and upcoming release notes, generated from repository records at deploy time
  (P2-M026).
- Annotated `milestone/<ID>` tags on every completed milestone's closure commit, created by
  `scripts/tag-milestone` (P2-M030).
- `docs/OPERATIONS.md`, a single operations reference for workflows, scripts, hooks, remotes, CI
  evidence, agent runs, and recovery (P2-M029).
- `scripts/agents/claude-code-bridge`, which runs Claude Code as an AgentForge agent with strict
  prompt decoding, a post-run path boundary check, and bridge-owned commits (P2-M027).

### Changed

- `forge run`, `forge task launch`, and `forge task launch-batch` print each agent's exit code and
  evidence log paths, show the last 20 lines of output when an agent fails, and exit 1 unless every
  agent exited 0 and all gates passed. Each run's full stdout/stderr is saved under
  `.forge/evidence/<task>/` and referenced by `AgentFinished`. The Claude Code bridge isolates the
  agent's Cargo target directory in worktrees (P2-M029).
- Tasks that declare `required_gates` run exactly those gates, and a missing gate profile fails
  preflight before any side effect. Blueprint default gates are copied into new tasks only when a
  matching profile exists (P1-M007). This milestone was implemented by Claude Code through AgentForge.
- The canonical repository is now `https://github.com/cybercore-tech/agentforge`, with the project
  site at `https://cybercore-tech.github.io/agentforge/` (P2-M025).
- Daemon executions stream keepalive frames and hold a single execution slot, so `daemon run` and
  `daemon launch` work for agents of any length within their profile timeout (P2-M024).
- Every CI job has an explicit timeout (P2-M023).

### Fixed

- An intermittent macOS `forge daemon stop` failure (`Invalid argument (os error 22)`) when a status
  poll hit a connection reset during daemon teardown (P2-M032).
- The recurring six-hour Windows CI hang in the daemon lifecycle tests, a stalled client blocking
  the daemon, a stop/restart lock race, macOS test fixture collisions, and Windows delete-pending
  teardown errors (P2-M023).
- `daemon run`/`daemon launch` failing after 2 seconds with misleading "stale daemon metadata"
  advice while the daemon was still running the task (P2-M024).
- `scripts/tag-milestone` tagging a plan's draft commit when the plan's prose mentioned
  `Status: Complete`. Closure detection now uses the status line and verifies it before tagging
  (P2-M031).
- The repository gate acting on the real repository when run from a Git hook inside a linked
  worktree (setting `core.bare`, injecting `[user]`, committing fixtures), and checkouts sharing a
  Cargo target directory running each other's binaries (P2-M028).

## [0.0.1] - 2026-09-20

### Added

- Plan-first repository governance and exact-head CI validation.
- Durable task contracts, state snapshots, capabilities, policy, and audit evidence.
- Managed Git worktree isolation and provider-neutral local process adapters.
- Deterministic gates, CI observation, scheduling primitives, and single-task orchestration.
- Project blueprint and guideline intake.
- Read-only and watch-mode operator HUD capabilities.
- Explicit task inspection, approval, accept, cancel, and retry actions.

### Notes

- This is an early development release. The `forged` daemon remains a placeholder identity binary,
  and the workspace crates are not published to crates.io.
- Release artifacts are intended for evaluation and controlled local use, not unattended production
  deployment.

[Unreleased]: https://github.com/cybercore-tech/agentforge/compare/v0.0.1...HEAD
[0.0.1]: https://github.com/cybercore-tech/agentforge/releases/tag/v0.0.1
