# Plan: P4-M010 — Remote-worker visibility and supervision

Status: Approved
Milestone: P4-M010
Created: 2026-09-25
Owner: AgentForge project

## Goal

The coordinator can see its workers, and a worker host can run unattended.

1. **Visibility.** `forge hud` shows every registered worker (platform, active leases against
   capacity, and the time of its last lease activity) and every active lease (task, worker,
   generation, and time to expiry), plus lease totals by state.
2. **Supervision.** `forge worker remote run` survives a coordinator or tunnel outage: when the
   worker API is unreachable, it reports the outage and retries with capped backoff instead of
   exiting. Refusals (for example `unauthorized`) stay fatal, so a misconfigured host still stops
   loudly. A tracked systemd user-unit template runs one worker per instance and restarts it on
   failure.

## Non-goals

- No new protocol verb or heartbeat. Liveness comes from the lease audit trail that already exists
  (claims, renewals, releases, and results are all `LeaseRecorded` events with wall-clock times
  since P0-M015).
- No coordinator-side worker registry changes; `.forge/workers/<id>.conf` stays the source.
- No launchd or Windows service definitions. The retry behaviour is portable; the unit template is
  Linux-only and documented as such.
- No graceful mid-task shutdown. Stopping a worker during a task abandons that run; the lease
  expires or the coordinator's sweep reclaims it, as today.

## Context

The P0-M014 remote run was observed with `forge lease list` and log tailing on both sides; nothing
showed the coordinator's view of its workers in one place. On the worker side,
`run_remote_worker` returns the first `claim()` error: a coordinator restart or a dropped GhostPort
tunnel ends the worker process, and nothing restarts it. `WorkerClient` reports every failure as a
plain `String`, so a caller cannot tell "unreachable" (retry) from "unauthorized" (stop).

## Architecture placement

- **`agentforge-daemon::worker_api`:**
  - `ClientError { Unreachable(String), Refused(String), Protocol(String) }` with `Display`:
    connect, write, and read failures are `Unreachable`; an `ERR` response is `Refused`; anything
    malformed is `Protocol`. `impl From<ClientError> for String` keeps every other client method's
    signature and message unchanged. `claim` returns `Result<Option<RemoteClaim>, ClientError>`.
  - `run_remote_worker`: an `Unreachable` claim error reports `RemoteReport::Unreachable { error,
    retry_in }`, sleeps, and retries. The delay starts at the poll interval, doubles per consecutive
    failure, is capped at `MAX_UNREACHABLE_BACKOFF` (60 s), and resets after any successful claim
    round trip. `--once` keeps today's behaviour (the error is returned). `Refused` and `Protocol`
    errors are returned, as today.
  - `RemoteWorkerOptions` gains `max_backoff` (so tests can use milliseconds).
- **`agentforge-hud`** (gains a dependency on `agentforge-operator` for the existing read-only
  `list_workers` and `list_leases`):
  - `WorkerSummary { worker_id, platform, max_leases, active_leases, last_activity_ms,
    last_action }`: the last activity is the newest `LeaseRecorded` event naming the worker.
  - `LeaseSummary { lease_id, task_id, worker_id, generation, expires_at_ms }` for active leases
    only, capped at `MAX_LEASES` (16), and `LeaseTotals { active, expired, released }`.
  - `collect_at(root, now_ms)` (and `collect` = `collect_at` with the wall clock), so tests fix the
    observation time.
  - Render: a `workers:` section (`- <id> platform=<p> leases=<active>/<max>
    last-activity=<UTC>(<action>)`, or `never`), and a `leases:` section (`active=N expired=N
    released=N`, then `- <lease> task=<t> worker=<w> gen=<g> expires-in=<s>s`). With no workers
    and no leases, both read `none`. Output stays bounded by the existing render cap.
- **CLI:** `forge worker remote run` prints `coordinator unreachable: <error>; retrying in <s>s`.
- **`contrib/systemd/agentforge-worker@.service`:** a user unit. `EnvironmentFile=%h/.config/
  agentforge/worker-%i.env` supplies `AGENTFORGE_ENDPOINT`, `AGENTFORGE_WORKER`,
  `AGENTFORGE_SECRET_FILE`, `AGENTFORGE_REPO`, and `AGENTFORGE_PROFILE`. `ExecStart` runs `forge
  worker remote run` with them. `Restart=on-failure` and `RestartSec=30` cover exits the retry does
  not (a failed doctor at start, a refusal). `KillMode=mixed` so the agent gets SIGTERM with the
  worker. No sandboxing directives: the worker must run git, ssh, and the agent with the user's
  environment (see `docs/REMOTE_WORKERS.md`).

## Invariants

- The HUD stays read-only: it never creates or modifies project files, including the lease
  snapshot and worker profiles.
- A refusal from the coordinator is never retried silently; only transport failures are.
- The worker never claims while the previous claim is unresolved (unchanged).

## ADRs

None; this extends ADR-0050 and ADR-0051 operationally. `REMOTE_WORKERS.md` and `HUD.md` document
it.

## Public API / CLI

`ClientError`, `RemoteReport::Unreachable`, `RemoteWorkerOptions::max_backoff`,
`MAX_UNREACHABLE_BACKOFF`, the HUD `WorkerSummary`, `LeaseSummary`, `LeaseTotals`, and
`collect_at`, and the `workers:` and `leases:` HUD sections.

## Compatibility analysis

`WorkerClient::claim`'s error type changes (Display gives the same text). HUD output gains two
sections. Every other client method is unchanged.

## Dependency analysis

`agentforge-hud` gains an internal dependency on `agentforge-operator`; no new external crates.

## Expected file boundary

- `.plans/P4-M010-remote-worker-visibility-and-supervision.plan.md`, `.plans/ACTIVE`
- `crates/agentforge-daemon/src/worker_api.rs`, `crates/agentforge-daemon/tests/*.rs`
- `crates/agentforge-hud/Cargo.toml`, `crates/agentforge-hud/src/lib.rs`,
  `crates/agentforge-hud/tests/*.rs`, `Cargo.lock`
- `crates/agentforge-cli/src/main.rs`, `crates/agentforge-cli/tests/*.rs`
- `contrib/systemd/agentforge-worker@.service`, `contrib/systemd/worker.env.example`
- `docs/REMOTE_WORKERS.md`, `docs/HUD.md`, `docs/OPERATIONS.md`
- Amendment 1: `crates/agentforge-daemon/src/lib.rs`, `crates/agentforge-cli/tests/*.rs`,
  `docs/DAEMON.md`, `docs/DOGFOODING.md`
- closure records: `docs/MILESTONES.md`, `CHANGELOG.md`, `README.md`, `PROJECT_STATE.md`,
  `AGENT_HANDOFF.md`

## Test-first matrix

- **Client:** a closed port gives `Unreachable`; a wrong secret gives `Refused("unauthorized")`.
- **Runner:** with no coordinator listening, the runner reports `Unreachable` with growing delays
  (capped) and keeps running; once a coordinator starts on that port, it claims and completes a
  task. A `Refused` claim ends the runner with the error. `--once` still returns the error.
- **HUD:** fixed-time snapshots with two workers (one with a renewal, one never seen), an active,
  an expired-unswept, and a released lease: the totals, the active-lease line with `expires-in`,
  the last-activity time and action, `never`, and `none` for an empty project. `collect` does not
  create `.forge/state` files on a project without leases.
- **CLI:** `forge hud` on a project with a granted lease shows the worker and lease lines.
- **Unit:** `systemd-analyze --user verify` accepts the template (locally; not in CI).
- **Live:** on this host, a coordinator (`forged` with the worker API) and a worker started from the
  unit template complete a leased task. Stopping the coordinator mid-idle shows `unreachable ...
  retrying`, and restarting it resumes claims without restarting the worker.

## Implementation sequence

1. Commit this draft plan.
2. Approve and activate it in a separate checkpoint.
3. `ClientError` and the runner retry with tests; then the HUD sections with tests; then the CLI
   message; then the unit template and the live check.
4. Docs; gate; commit (exit checked); push; CI plus a repeat.
5. Close; tag after the dry run names the "close ..." commit.

## Failure modes

- A permanently unreachable coordinator makes the worker retry forever at the capped interval. That
  is the intended supervised behaviour; each attempt is reported, so the journal shows it.
- Misclassifying a refusal as unreachable would hide a misconfiguration. Only socket-level failures
  are `Unreachable`, and a test pins `unauthorized` as `Refused`.

## Documentation impact

REMOTE_WORKERS (unattended workers: the unit, the env file, outage behaviour), HUD (the new
sections), and OPERATIONS (commands and recovery rows). README and CHANGELOG at closure.

## Quality gates

- `./scripts/gate.sh full`;
- push CI plus a dispatched repeat.

## Acceptance criteria

- [ ] `forge hud` shows workers (capacity, last activity) and active leases (expiry), read-only.
- [ ] A remote worker keeps running through a coordinator outage and resumes; refusals still stop
      it.
- [ ] A tracked systemd user-unit template runs a worker unattended, verified live on this host.
- [ ] Docs; CI evidence; closed and tagged correctly.

## Amendment 1 (2026-09-25)

The live check found a latent defect outside this plan's boundary (dogfooding finding 16).
Classification: semantic, in the daemon's process setup; not caused by this milestone's changes.

`forge daemon start` spawns `forged` with stderr piped back to itself, to report startup failures.
When `forge` exits, the pipe's read end closes. Every later `eprintln!` in `forged` then fails with
`EPIPE`, and `eprintln!` panics on a write failure, killing the thread that logged:

- **The lease sweep** logs `forged: expired N lease(s)` after recording an expiry. Reproduced live:
  the first expiry was recorded (audit #9), and the sweep thread was gone afterwards. A second lease
  granted with a 1 s TTL was never expired (audit #10 is its grant, with no expiry 14 s later), and
  `forged` had two threads instead of three. Automatic dispatch runs in the same sweep, so it stops
  too.
- **A worker API connection** logs a refused request before answering, so a refused worker saw
  `response failed: connection closed` instead of `unauthorized`. The P4-M010 runner classified it,
  correctly, as a transport failure and retried, so a rotated secret was retried forever instead of
  stopping the worker.

The tests missed it because they start `forged` in-process or keep its stderr open. Fix:

1. `forged` logs through a helper that ignores write failures (`let _ = writeln!(stderr, ...)`), so
   logging can never kill a thread. Every runtime `eprintln!` in `agentforge-daemon/src/lib.rs` and
   `worker_api.rs` uses it.
2. `forge daemon start` sends `forged`'s stderr to `.forge/daemon/forged.log` (appended) instead of
   a pipe that dies with `forge`, so the log survives. A startup failure is reported from the
   bytes the log gained during startup.
3. A regression test starts `forged` through the real `forge daemon start`, lets `forge` exit, and
   requires two consecutive lease expiries to be recorded and logged, plus a bad-secret worker
   request to be refused with `unauthorized`.

Docs: DAEMON (the log file) and DOGFOODING (finding 16).
