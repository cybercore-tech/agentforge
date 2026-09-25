# Plan: P4-M011 — Worker-host setup scripts

Status: Complete
Milestone: P4-M011
Created: 2026-09-25
Owner: AgentForge project

## Goal

Setting up a remote worker on another machine becomes one command on each side, so the
two-machine run can start as soon as a second machine is available:

- **coordinator:** `scripts/worker-bundle` registers and enrolls the worker, turns on the worker API,
  and writes a small bundle for the worker host;
- **worker host:** `scripts/worker-host-setup <bundle> --repo <path>` does everything
  REMOTE_WORKERS.md lists as manual (clone, Git identity, hooks, secret, agent profile, GhostPort key
  and client config, env file, and optionally the systemd unit), then runs `forge worker remote
  doctor`.

## Operator decision (2026-09-25)

The second machine is being reinstalled. The operator chose to prepare this now, so the real
two-machine run is one command once it is ready.

## Non-goals

- No SSH or remote execution from the coordinator: the operator copies the bundle and runs the
  worker-host script there (the operator's SSH keys are passphrase-protected, by design).
- No change to the protocol, the worker, or GhostPort. No automatic GhostPort key exchange: the
  worker host's public key is printed, with the exact coordinator config snippet to add.
- No launchd or Windows service setup (the scripts are bash, for Linux and macOS worker hosts; the
  unit option is Linux-only).

## Context

The P0-M014 remote run needed five manual worker-host steps, and P4-M009's doctor now checks them,
but a new host still has to be set up by hand from the docs. A mistake shows up only as a failing
doctor check. With the second machine coming back soon, the remaining friction should be removed
and rehearsed before then.

## Architecture placement

- **`scripts/worker-bundle <root> <worker-id> --origin <git-url> [options]`** (bash, coordinator):
  - writes `.forge/workers/<id>.conf` if missing (`--platform`, repeated `--capability`, and
    `--max-leases`), and refuses a conflicting existing profile;
  - enrolls with `forge worker enroll` if the worker has no secret yet (otherwise it reuses the
    existing enrollment);
  - ensures `.forge/worker-api.conf` binds `127.0.0.1:<--api-port>` (default 47420), and refuses a
    different existing bind;
  - writes the bundle directory (mode 700, `--out`, default `./worker-<id>.bundle`): `worker.env`
    (worker ID, origin, tunnel port, GhostPort server addresses, link ID, and the coordinator's
    GhostPort public key from `--server-key`), `secret` (mode 600), and `README.txt` (the next
    steps);
  - prints the coordinator's GhostPort server `[[links]]` entry and the `[[peers]]` entry to
    complete with the worker's key.
- **`scripts/worker-host-setup <bundle> --repo <path> [options]`** (bash, worker host),
  idempotent:
  1. prerequisites: `git`, `forge` (with its version), `ghostport` (unless `--no-ghostport`),
     and `python3` plus `claude` for the Claude Code profile;
  2. clones the origin into `--repo` if absent, or verifies the existing clone's origin;
  3. sets the clone's Git identity from `--git-name` and `--git-email` when missing (it is
     required then), and runs `scripts/install-hooks` if the project has it;
  4. installs the secret as `~/.config/agentforge/worker-<id>.secret` (mode 600);
  5. writes `.forge/agents/<profile>.conf` in the clone: the Claude Code bridge from *this* clone
     (with `--claude`, `--forge`, `PATH`, and `HOME`), or `--agent-executable <path>` for another
     agent;
  6. GhostPort: generates `~/.config/ghostport/agentforge-<id>.key` if missing, writes the client
     config `agentforge-<id>.toml` (the link listens on `127.0.0.1:<tunnel-port>`), validates it
     with `ghostport check`, and prints the public key with the coordinator `[[peers]]` snippet;
  7. writes `~/.config/agentforge/worker-<id>.env` (mode 600); with `--install-unit`, installs
     the systemd unit template from the clone;
  8. runs `forge worker remote doctor` and exits with its status. `--endpoint` overrides the tunnel
     endpoint (the local rehearsal, `--no-ghostport`);
  - `$HOME`-relative paths only, so a rehearsal can use a scratch `HOME`.
- **Tests:** a CLI integration test (Unix) runs both scripts end to end against the real `forge`: an
  in-process worker API on loopback, `--no-ghostport`, a scratch `HOME`, and the fixture agent. It
  checks the files and modes, that the doctor passes, that a second run is idempotent (no changes,
  still passing), refusals (a missing identity, a conflicting worker-API bind), and finally one
  `forge worker remote run --once` that uses the env file's values and gets its result imported.
- **Docs:** REMOTE_WORKERS gains "Setting up a worker host in two commands"; OPERATIONS gains the
  commands.

## Invariants

- The scripts never print or log the secret; it is written only to mode-600 files.
- The coordinator script never changes an existing, different worker profile or worker-API bind.
- The worker-host script never overwrites a user's GhostPort key or existing clone.

## ADRs

None; this is operational tooling around ADR-0050 and ADR-0051.

## Public API / CLI

Two maintainer scripts. No product changes.

## Compatibility analysis

Additive.

## Dependency analysis

None (bash, git, and the existing `forge` and `ghostport` binaries).

## Expected file boundary

- `.plans/P4-M011-worker-host-setup.plan.md`, `.plans/ACTIVE`
- `scripts/worker-bundle`, `scripts/worker-host-setup`
- `crates/agentforge-cli/tests/*.rs`
- `docs/REMOTE_WORKERS.md`, `docs/OPERATIONS.md`
- closure records: `docs/MILESTONES.md`, `CHANGELOG.md`, `README.md`, `PROJECT_STATE.md`,
  `AGENT_HANDOFF.md`

## Test-first matrix

- **CLI integration test:** as above (bundle, setup, doctor passes, idempotent re-run, refusals, and
  a remote run through the env file's values, imported).
- **Live rehearsal on this host, with real GhostPort:**
  - a coordinator with a GhostPort server (scratch keys and config);
  - the worker host as a separate clone under a scratch `HOME`, set up by `worker-host-setup`
    *with* GhostPort, using the printed snippets;
  - the doctor passes through the tunnel;
  - the systemd unit is not installed in the rehearsal, to leave the user's manager untouched.
    `--install-unit` is checked with `--dry-run`.
- The full gate, and push CI plus a dispatched repeat.

## Implementation sequence

1. Commit this draft plan.
2. Approve and activate it in a separate checkpoint.
3. The coordinator script, the worker-host script, and the integration test; the live rehearsal;
   docs.
4. Gate; commit (exit checked); push; CI plus a repeat.
5. Close; tag after the closure CI is green.

## Failure modes

- A worker host lacks a prerequisite: the script stops at step 1 with what to install.
- The GhostPort tunnel is not connected yet: the doctor's endpoint check fails. The script says so
  and prints what to finish on the coordinator.

## Documentation impact

REMOTE_WORKERS, OPERATIONS; CHANGELOG at closure.

## Quality gates

- `./scripts/gate.sh full`;
- push CI plus a dispatched repeat.

## Acceptance criteria

- [x] The two scripts set up a worker host from a bundle, idempotently, with the doctor passing.
- [x] The CLI test runs a remote task through a host set up only by the scripts.
- [x] A live rehearsal with real GhostPort passes on this host.
- [x] Docs; CI evidence; closed and tagged correctly.

## Completion record

Implementation commit: `3358a30`
CI run: `36187573886` (push) and `36187851798` (dispatched repeat)
CI result: green on all seven jobs in both runs; the new integration test ran and passed on Linux
and macOS
Completed: 2026-09-25
Notes:
- **Live rehearsal with real GhostPort** on this host, with scratch keys and configs. The first
  `worker-host-setup` run failed only the doctor's endpoint check, and printed the `[[peers]]`
  entry. After adding it to the server config and starting both GhostPort ends, the second run
  changed nothing ("unchanged" for every file) and the doctor passed. A remote task through the
  tunnel was imported at the exact SHA (`3ccaf22`).
- **Rehearsal mistake:** the first live run overrode `HOME` but not `XDG_CONFIG_HOME`, which this
  session sets to the real `~/.config`. Five new files were written there: the GhostPort key pair
  and client config, and the worker secret and env file. They came from that run only, nothing was
  overwritten, and all five (and the empty `~/.config/agentforge`) were removed. The script is
  right to honour `XDG_CONFIG_HOME`. The CI test clears it, and REMOTE_WORKERS now warns about it
  for rehearsals.
- The rehearsal also found the dry run hiding its keygen step, which was fixed before commit.
- A mutant that skips installing the secret fails the integration test.
- The systemd unit option was checked with `--dry-run` only, to leave the user's manager alone. The
  unit itself was verified live in P4-M010.
