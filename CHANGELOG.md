# Changelog

All notable AgentForge changes are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and release tags follow
[Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- `scripts/rehearse-two-hosts` rehearses the two-machine remote-worker run with two clean
  containers, real GhostPort, and network chaos (delay, loss, partition) (P4-M012).

- `scripts/worker-bundle` (on the coordinator) and `scripts/worker-host-setup` (on the worker host)
  set up a remote worker in two commands: registration, enrollment, clone, identity, hooks, secret,
  agent profile, GhostPort key and config, env file, and the optional systemd unit. The worker-host
  script is idempotent and ends with `forge worker remote doctor` (P4-M011).

### Fixed

- A remote worker no longer abandons finished work when its result upload fails in transit: it
  retries, and reports a refused retry as possibly imported (P4-M012, finding 19).
- Claiming a lease restarts its window, so a worker that claims late no longer loses the lease
  before its first renewal (P4-M012, finding 20).
- `scripts/worker-host-setup` is idempotent on hosts without `cmp`.
- Remote workers need GhostPort v0.1.2 or later: earlier versions throttled busy authenticated
  peers (finding 18, fixed in GhostPort).

## [0.3.0] - 2026-09-25

The first release with **attested builds**: every archive carries a keyless SLSA build-provenance
attestation from the release workflow, so you can verify where it came from with `gh attestation
verify <archive> -R cybercore-tech/agentforge`. Remote workers can now run unattended. They ride
out coordinator outages, run under a systemd user unit, and show up in `forge hud`. Agents can call
AgentForge's own task tools over MCP, under their capability policy. **Upgrading from `v0.2.0` is
recommended:** in `v0.2.0` a `forged` started with `forge daemon start` stops expiring leases after
the first expiry. Compatibility note: tool-call audit events (code 13) cannot be read by `v0.2.0`.

### Added

- Release archives carry keyless SLSA build-provenance attestations from the release workflow
  (P5-M004, ADR-0053). Verify a download with `gh attestation verify <archive> -R
  cybercore-tech/agentforge`. The workflow verifies the attestations of the assets it uploaded,
  and manual release runs rehearse it.

- `forge mcp serve`, an MCP task-tool gateway (P3-M005, ADR-0052). An agent can call
  `task_contract`, `check_changes`, and `run_gate` for its own task. Each tool is mapped to the
  task's capabilities and checked with the policy engine, and every call is recorded as a
  `ToolInvoked` audit event (new kind, code 13). The Claude Code bridge wires it in for tasks that
  hold `use_mcp_tools` (new `--forge` argument). New dependencies: `serde` and `serde_json`.

- `forge hud` shows remote workers (platform, active leases against capacity, and when each was
  last seen through its own claims, renewals, and releases) and active leases with their time to
  expiry, plus lease totals by state (P4-M010).
- `forge worker remote run` keeps running through a coordinator outage: an unreachable worker API
  is retried with capped backoff (2 s doubling to 60 s), while refusals such as `unauthorized` stop
  it. `contrib/systemd/agentforge-worker@.service` runs one worker per systemd user unit
  (P4-M010).

- Audit events record wall-clock time: they are stamped when created (`AuditEvent::now`), the store
  stamps any stragglers, and the HUD shows `at=` on recent events and `duration=` on agent runs
  (P0-M015). Earlier records carried the placeholder `1` and render without a time.

- `forge worker remote doctor` checks a worker host: clone, Git identity, hooks, agent profile paths
  (including paths copied from another checkout), secret file, and an authenticated, side-effect-free
  `PING` to the coordinator. `forge worker remote run` runs it first and refuses to start on any
  failing check, and now reports lease renewals after each task (P4-M009).

### Fixed

- A `forged` started by `forge daemon start` stopped expiring leases (and dispatching) after its
  first expiry, and answered refused workers with a dropped connection. Logging to its stderr pipe
  panicked once `forge` had exited. `forged` now logs to `.forge/daemon/forged.log`, and logging can
  no longer fail a thread (P4-M010).
- A remote worker could not run the same task again after an attempt on that host (`managed task
  branch already exists`). Attempts are now archived under their lease IDs (P4-M010).

- `scripts/tag-milestone` could tag a milestone whose closure was never committed (it read the
  working-tree milestone table and fell back to the plan's last commit). It now decides only from
  committed history and refuses uncommitted or unclosed milestones; existing tags are unchanged
  (P2-M034).
- The ADR registry was missing ADR-0026 to ADR-0051, and four docs were not linked from the README.
  `xtask validate` now enforces both, in the gate and CI (P2-M034).

- Process-pipe and stdin readers (agent output capture and input forwarding, gate capture, the CI
  provider reader, CLI input) now retry a read interrupted by a signal instead of failing
  (P0-M014). This change was implemented by Claude Code as a remote worker through AgentForge.

- The release workflow's publish job failed on the build staging directories in the artifacts. It
  now uploads only the four archives, their `.sha256` files, and `SHA256SUMS`, and requires exactly
  four archives. A manual (dry-run) release now exercises the whole publish job except the upload
  (P5-M002). `v0.2.0` was published from its tag run's own artifacts with the documented recovery.

- The release upload could only be tested by a real tag, and it failed on both so far. A manually
  dispatched release run now rehearses it: it uploads the release files to a draft release (no
  tag), downloads them back, verifies names, bytes, and `SHA256SUMS`, and deletes the draft. Tag
  runs verify their published assets the same way (`scripts/publish-release`, P5-M003).

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

[Unreleased]: https://github.com/cybercore-tech/agentforge/compare/v0.3.0...HEAD
[0.3.0]: https://github.com/cybercore-tech/agentforge/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/cybercore-tech/agentforge/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/cybercore-tech/agentforge/releases/tag/v0.1.0
