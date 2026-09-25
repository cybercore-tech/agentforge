# Changelog

All notable AgentForge changes are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and release tags follow
[Semantic Versioning](https://semver.org/).

## [Unreleased]

## [0.2.0] - 2026-09-24

Workers on other machines can now run AgentForge tasks. They claim leases over an authenticated
GhostPort tunnel, run the task in their own clone, and return the commit. The coordinator imports
it only at the verified exact SHA and runs the gates itself. This release also fixes audit-log
corruption under concurrent writers, which affects `v0.1.0`, and binds merge approvals to the
reviewed commit. **Upgrading from `v0.1.0` is recommended.**

### Added

- Remote-worker lease operations (P4-M004, ADR-0046):
  - workers are registered as `.forge/workers/<id>.conf` profiles, and `forge worker list` shows
    them;
  - `forge lease grant|list|renew|release|expire` manage leases, each change audited as the new
    `LeaseRecorded` event;
  - a leased task, or one whose paths overlap a lease, cannot run locally on any path, and
    `launch-batch` skips it;
  - a running `forged` expires due leases every 5 seconds, only while no execution is active.

  No worker is contacted yet.
- `forge worker run <root> <worker-id> (<exe> | --profile <p>) [--once] [--poll-ms <ms>]`, a
  same-host worker process (P4-M005, ADR-0048). It claims the leases held by its worker, runs each
  task through the standard launch path, renews the lease while the agent runs, and releases it.
  Only the exact lease holder can run a leased task. Lease events gain the `claimed` action.
- Opt-in automatic dispatch (P4-M006, ADR-0049).
  - A reviewed `.forge/dispatch.conf` (`enabled`, `milestone=` scope, `ttl_ms`, `max_per_tick`) lets
    the daemon's idle tick, or `forge lease dispatch`, grant ready tasks in listed milestones to
    registered workers.
  - Only tasks with every pre-execution approval recorded are granted, once per task, within
    worker capacity.
  - Automatic grants are audited with `dispatch=auto`.
- A remote-worker channel over GhostPort (P4-M007, ADR-0050).
  - `forge worker enroll` creates a per-worker secret.
  - `forged` serves a loopback-only worker API (`AFW1`) from `.forge/worker-api.conf`.
  - `forge worker remote claim|renew|release` is the client. Workers on other machines reach it
    through a GhostPort tunnel, and every request is authenticated as one worker and limited to
    its leases.
  - The task contract travels as the `agentforge-task-prompt-v1` document, with the new
    `parse_task_prompt` decoder.
- Remote execution (P4-M008, ADR-0051).
  - `forge worker remote run --repo <clone>` runs claimed tasks on another machine and returns a
    `git bundle`.
  - The coordinator imports it only if the bundle's commit is exactly the reported SHA, descends
    from the base, and touches only allowed paths. It runs the task's gates locally, and the
    result follows normal review.
  - `CLAIM` requires recorded pre-execution approvals and returns the lease window.

### Changed

- Merge, release, and deployment approvals are post-execution (P1-M008, ADR-0045). Agents launch
  without them. They can be recorded only after `forge task accept`, each is bound to the reviewed
  task branch head (`forge task approve` prints it, and `forge task inspect` lists it), and
  `forge task integrate` fast-forwards exactly that commit, re-checked under the integration lock.
  Merge approvals recorded earlier must be given again.

### Fixed

- The daemon and worker API socket readers gave up on an interrupted system call (`EINTR`) instead
  of retrying, which intermittently failed daemon and remote-worker requests (P5-M002).
- Audit-log corruption when two writers appended concurrently: for example, `forge task approve`
  while a daemon execution was running. That made the next open fail the integrity check. Appends
  now take a short lock, verify other writers' records, and renumber a stale event instead of
  corrupting the chain, and an execution's records are written as one batch. A race that could
  expose a half-written header when creating a new log is also fixed (P0-M013, ADR-0047).
- `forge task approve` (and other operator actions) failing with "audit log is missing" on a fresh
  project. The log is now created on first use (P0-M013).
- The release workflow's publish job failing with `not a git repository` (it has no checkout;
  `gh release create` now gets `--repo`). `SHA256SUMS` now lists bare archive names so
  `sha256sum -c SHA256SUMS` works in a download folder (P5-M001).

## [0.1.0] - 2026-09-24

The first tagged release. AgentForge now builds and integrates its own milestones through a real
coding agent (Claude Code), with hermetic gates, audited agent evidence, and operator review.

### Added

- An `agent_runs:` section in `forge hud` listing the five most recent agent runs with exit code,
  termination, gate results (first failed gate), and evidence log paths (P2-M033). Built by Claude
  Code through AgentForge and landed with `forge task integrate`.
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

## 0.0.1 - 2026-09-20 (never tagged)

### Added

- Plan-first repository governance and exact-head CI validation.
- Durable task contracts, state snapshots, capabilities, policy, and audit evidence.
- Managed Git worktree isolation and provider-neutral local process adapters.
- Deterministic gates, CI observation, scheduling primitives, and single-task orchestration.
- Project blueprint and guideline intake.
- Read-only and watch-mode operator HUD capabilities.
- Explicit task inspection, approval, accept, cancel, and retry actions.

### Notes

- No `v0.0.1` tag or GitHub release was ever created; these changes first shipped in `v0.1.0`.
- This is an early development release. The `forged` daemon remains a placeholder identity binary,
  and the workspace crates are not published to crates.io.
- Release artifacts are intended for evaluation and controlled local use, not unattended production
  deployment.

[Unreleased]: https://github.com/cybercore-tech/agentforge/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/cybercore-tech/agentforge/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/cybercore-tech/agentforge/releases/tag/v0.1.0
