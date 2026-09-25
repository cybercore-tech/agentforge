# Plan: P4-M009 — Worker-host doctor

Status: Complete
Milestone: P4-M009
Created: 2026-09-24
Owner: AgentForge project

## Goal

A worker host is checked before it runs anything (dogfooding finding 14). `forge worker remote
doctor` reports each setup requirement as `ok`, `warn`, or `fail`, with a fix. `forge worker
remote run` runs the same checks first and refuses to start on any `fail`, so a missing hook can no
longer silently skip the agent's own pre-commit gate.

## Non-goals

- No automatic fixing (for example installing hooks or editing profiles); the doctor reports and
  explains.
- No base-commit check (the base is known only at claim time; the run already fetches it once).
- No change to the coordinator's authority; `PING` changes no state.

## Context

The P0-M014 remote run needed five manual worker-host steps:
- a clone;
- a Git identity;
- `./scripts/install-hooks`;
- a `claude-code` profile rewritten to point at the clone's own bridge (profiles hold absolute
  paths);
- the secret file at mode 600.

`forge worker remote run` checked none of them. With hooks missing, the agent's pre-commit gate
would not run, and only the coordinator's gate would catch problems, later and with less feedback
for the agent.

## Architecture placement

- **The worker protocol:** a new `PING` verb that is authenticated like every verb, has no side
  effects, and answers `OK PONG <worker-id>`. Failed authentication keeps the existing generic
  `unauthorized` with its fixed delay.
- **`worker_api::remote_doctor(options) -> Vec<DoctorCheck>`** with `DoctorCheck { name, level:
  Ok|Warn|Fail, detail, fix }`. The checks:
  1. `repo`: `--repo` is a Git repository with at least one commit (fail otherwise).
  2. `git-identity`: `user.name` and `user.email` resolve in the clone (fail); remote results are
     committed there.
  3. `hooks`: if the clone tracks a `.githooks/pre-commit`, `core.hooksPath` must be `.githooks`
     (fail, with the fix `./scripts/install-hooks`). Otherwise an executable
     `.git/hooks/pre-commit` gives `ok`, and a missing one gives `warn` ("no pre-commit gate;
     only the coordinator's gates will run").
  4. `agent`: the profile (or executable) loads, and its executable exists and is executable
     (fail). Every absolute-path argument must exist (fail); one that exists but is outside
     `--repo` gives `warn` ("profile points outside this clone; copied from another checkout?").
  5. `secret`: `read_secret_file` succeeds (mode, format) (fail).
  6. `endpoint`: loopback, reachable, and `PING` authenticates as the worker (fail, with the
     reason: unreachable, refused, or unauthorized).
  7. `origin`: the clone has at least one remote (warn: "the run cannot fetch a missing base
     commit").
- **CLI:** `forge worker remote doctor --endpoint --worker --secret-file --repo (--profile p |
  --executable e)`. It prints one line per check and exits 1 on any `fail`. `forge worker remote
  run` runs the doctor first, prints it, and refuses on any `fail`.
- **Finding 13 is folded in:** `RemoteReport::Renewals { renewed, failures }` is reported after
  each task (the same as the same-host worker), so the worker host sees its lease health.

## Invariants

- `forge worker remote run` never starts with a failing setup check.
- `PING` never changes state or audit.

## ADRs

None; this extends ADR-0050 and ADR-0051 operationally. `REMOTE_WORKERS.md` documents it.

## Public API / CLI

`PING`, `WorkerClient::ping`, `remote_doctor`, `DoctorCheck`, `forge worker remote doctor`, and
`RemoteReport::Renewals`.

## Compatibility analysis

Additive protocol verb. `worker remote run` is stricter: a host that was silently misconfigured
now refuses to start, with the fix printed.

## Dependency analysis

None.

## Expected file boundary

- `.plans/P4-M009-worker-host-doctor.plan.md`, `.plans/ACTIVE`
- `crates/agentforge-daemon/src/worker_api.rs`, `crates/agentforge-daemon/tests/*.rs`
- `crates/agentforge-cli/src/main.rs`, `crates/agentforge-cli/tests/*.rs`
- `docs/REMOTE_WORKERS.md`, `docs/OPERATIONS.md`, `docs/DOGFOODING.md`
- closure records: `docs/MILESTONES.md`, `CHANGELOG.md`, `README.md`, `PROJECT_STATE.md`,
  `AGENT_HANDOFF.md`

## Test-first matrix

- `PING`: authenticated, gives `PONG` with no audit change; a wrong secret gives `unauthorized`.
- The doctor, on a correctly set-up clone: all `ok`.
- Each failure on its own: no identity; `.githooks` tracked but `core.hooksPath` unset; a profile
  executable missing; an absolute argument missing; a secret at mode 644 (Unix); a wrong secret; an
  unreachable endpoint.
- `warn`: an absolute argument outside the clone, and no remote.
- `worker remote run` refuses on a failing doctor, with nothing claimed.
- Renewals are reported by the remote runner.

## Implementation sequence

1. Commit this draft plan.
2. Approve and activate it in a separate checkpoint.
3. `PING`, then the doctor with tests, then the CLI and the run preflight, then renewal reporting.
4. Docs; gate; commit (exit checked); push; CI plus a repeat.
5. Close; tag after the dry run names the "close ..." commit.

## Failure modes

- A false `fail` blocks a valid host. Each check prints its detail and fix, and the rules are
  deliberately narrow (the hooks check applies only when the clone tracks `.githooks`).

## Documentation impact

REMOTE_WORKERS (worker-host setup checklist and the doctor), OPERATIONS (the command and recovery
rows), and DOGFOODING (findings 13 and 14 resolved).

## Quality gates

- `./scripts/gate.sh full`;
- push CI plus a dispatched repeat.

## Acceptance criteria

- [x] `forge worker remote doctor` checks every requirement from the P0-M014 run, with fixes.
- [x] `forge worker remote run` refuses to start on a failing check.
- [x] Remote renewals are reported (finding 13).
- [x] Docs; CI evidence; closed and tagged correctly.

## Completion record

Implementation commit: `d003175`
CI run: `36104260988` (push) and `36104451697` (dispatched repeat)
CI result: green on all seven jobs in both runs
Completed: 2026-09-24
Notes:
- All seven checks are covered by per-failure tests.
- One design change during implementation: the profile-path warning first used a name-based
  heuristic. It was replaced by a principled rule before commit: warn when an absolute argument
  lives in a *different Git checkout* than the worker's clone. That is exactly the P0-M014 copied
  bridge path.
- The existing remote-execution CLI test passes through the new preflight unchanged.
- Findings 13 and 14 are resolved; finding 12 (audit timestamps) is next, as its own milestone.
