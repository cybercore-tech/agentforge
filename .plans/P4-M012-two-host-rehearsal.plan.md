# Plan: P4-M012 — Two-host rehearsal in containers, with network chaos

Status: Approved
Milestone: P4-M012
Created: 2026-09-25
Owner: AgentForge project

## Goal

Until the operator's second machine is available (weeks away), rehearse the two-machine
remote-worker run as closely as this host allows. `scripts/rehearse-two-hosts` runs two clean Arch
Linux containers, a **coordinator** and a **worker host**, with separate filesystems, users, and
network namespaces, joined only by a Docker bridge network. They talk through real GhostPort over
that network, not loopback. The worker host is set up only by `scripts/worker-host-setup`, from a
bundle made by `scripts/worker-bundle` and copied across as an operator would. Then it proves, with
`tc netem` on the worker's network interface:

1. a remote task imported at its exact SHA over a clean link;
2. a task completed under a bad network (delay, jitter, and loss), with lease renewals across it;
3. a network partition while idle: the worker retries (`coordinator unreachable`), then recovers
   (`coordinator reachable again`) and claims new work, in the same process.

## Operator decision (2026-09-25)

The second machine is weeks away. The operator chose a stand-in second machine (container plus
network chaos), then an agent-built milestone with the MCP gateway.

## Non-goals

- Not a replacement for the real two-machine run (different hardware and kernel are not tested);
  it is a rehearsal, and is recorded as one.
- No Gateflow integration: Gateflow applies `netem` only to a sandbox's own loopback. Chaos on the
  link between two namespaces is still on its roadmap, and it is a Rust library, not a shell tool.
  Plain `tc netem` inside the worker container does the same job here.
- No CI job (a Docker plus `netem` run of minutes, with image pulls); it is an operator rehearsal,
  and its evidence is recorded.
- No change to the product; findings get their own amendment.

## Context

The one-host rehearsals (P0-M014, P4-M010, P4-M011) used loopback and the operator's own
toolchain. They could not show:
- a truly clean host (missing prerequisites, a fresh user);
- a real network path;
- behaviour under latency and loss, which is where lease renewal and the outage retry matter.

## Architecture placement

- **`contrib/rehearsal/Dockerfile`:** `archlinux` with `git`, `iproute2`, `python`, and
  `openssh`-free defaults; the users `coord` and `worker`; and a rehearsal agent
  (`/usr/local/bin/rehearsal-agent`: sleeps, then writes the task's file).
- **`scripts/rehearse-two-hosts [--keep] [--forge-archive <tar.gz>] [--ghostport <binary>]`**
  (host side, Docker required):
  1. Installs **`forge`/`forged` from the published `v0.3.0` release archive**, downloaded and
     checked on the host with `sha256sum -c` and `gh attestation verify` (or a given archive), so
     the rehearsal also exercises the real install path. `ghostport` is copied from the host's
     build (the worker host would build or install it).
  2. Creates a bridge network and two containers (the worker with `NET_ADMIN` for `tc`).
  3. **Coordinator:** a project with tasks, `forged` running, `git daemon` serving the project as
     the origin, a GhostPort key, and `scripts/worker-bundle`.
  4. The bundle is copied coordinator → host → worker, as an operator would copy it.
  5. **Worker:** `scripts/worker-host-setup`. The first run's endpoint check fails, and the printed
     `[[peers]]` entry is added to the coordinator's server config. Both GhostPort ends start, and
     the second run passes the doctor.
  6. **Scenarios** 1 to 3 above: `tc qdisc` on the worker's `eth0` for chaos (delay 150 ms, jitter
     50 ms, loss 5%) and partition (loss 100%). Each is checked from the coordinator (`forge hud`,
     lease states, the task branch at the reported SHA) and from the worker's own output.
  7. Prints PASS/FAIL per check and a transcript path, removes the containers and network (unless
     `--keep`), and exits non-zero on any failure.
- **Docs:** REMOTE_WORKERS (the rehearsal and its evidence) and DOGFOODING (the run log).

## Invariants

- The host is untouched apart from Docker objects named `agentforge-rehearsal-*` (removed at the end)
  and a scratch directory. No host config, key, or service changes.
- Nothing from the operator's real GhostPort or AgentForge config is used or copied.

## ADRs

None.

## Public API / CLI

A maintainer script and a Dockerfile.

## Compatibility analysis

Additive.

## Dependency analysis

None in the workspace. The rehearsal uses Docker and the `archlinux` image.

## Expected file boundary

- `.plans/P4-M012-two-host-rehearsal.plan.md`, `.plans/ACTIVE`
- `scripts/rehearse-two-hosts`, `contrib/rehearsal/Dockerfile`, `contrib/rehearsal/rehearsal-agent`
- `docs/REMOTE_WORKERS.md`, `docs/DOGFOODING.md`, `docs/OPERATIONS.md`
- Amendment 1: `scripts/worker-host-setup`
- Amendment 2: `crates/agentforge-daemon/src/worker_api.rs`, `crates/agentforge-cli/src/main.rs`,
  `crates/agentforge-cli/tests/*.rs`, `docs/DAEMON.md`
- closure records: `docs/MILESTONES.md`, `CHANGELOG.md`, `README.md`, `PROJECT_STATE.md`,
  `AGENT_HANDOFF.md`

## Test-first matrix

The rehearsal is the test. It must report PASS for:
- the release archive verified (checksum and attestation), installed in both containers, and
  reporting `0.3.0`;
- the first setup run failing only the endpoint check, and the second passing the doctor through
  the tunnel;
- scenario 1: imported, with the coordinator's task branch equal to the worker's reported head;
- scenario 2: imported under `netem`, with at least one renewal, and a measured RTT increase
  confirming the chaos was real;
- scenario 3: at least two `coordinator unreachable` reports during the partition,
  `coordinator reachable again` after it, a later claim and import, and a single worker process
  throughout (no restarts);
- cleanup: no `agentforge-rehearsal-*` containers or networks left.

A deliberately broken run (for example the partition never healed) must FAIL, to show the checks
bite.

## Implementation sequence

1. Commit this draft plan.
2. Approve and activate it in a separate checkpoint.
3. The Dockerfile and agent; the script, step by step; a full run; the broken-run check; docs.
4. Gate; commit (exit checked); push; CI plus a repeat (the text and validation checks cover the new
   files).
5. Close; tag after the closure CI is green.

## Failure modes

- A product defect found by the rehearsal: stop, classify, amend, and fix with a test (as in
  P4-M010).
- Mirror or pull failures: the script fails early with the reason; nothing is left running.

## Documentation impact

REMOTE_WORKERS, DOGFOODING, OPERATIONS; CHANGELOG at closure.

## Quality gates

- `./scripts/gate.sh full`, and a passing rehearsal run with its transcript;
- push CI plus a dispatched repeat.

## Acceptance criteria

- [ ] `scripts/rehearse-two-hosts` passes every check, and a broken run fails.
- [ ] The worker host is set up only by the P4-M011 scripts, using the verified `v0.3.0` release.
- [ ] Remote work survives delay, loss, and a partition, with evidence from both sides.
- [ ] Docs; CI evidence; closed and tagged correctly.

## Amendment 1 (2026-09-25)

The first full rehearsal run found a defect in `scripts/worker-host-setup` (P4-M011). On the clean
Arch worker host, the second setup run reported `installed the secret` again instead of
`unchanged`. The script compares the installed secret with `cmp -s`, and `cmp` (from `diffutils`)
is not in the base image. The comparison failed as "different", so the file was rewritten on every
run: harmless, but not idempotent. The script depends on a tool it never checks for.
Classification: semantic, a latent worker-host-setup defect. The P4-M011 test and rehearsal ran on
hosts that have `diffutils`.

Fix: compare the files with the shell alone (no `cmp`), so the script needs only `git`,
coreutils, `grep`, and `sed`. The container rehearsal's idempotency check covers it.

The same run showed a rehearsal-design error, fixed in the rehearsal script (in scope): scenario 2's
lease grant was refused ("path rehearsal.txt overlaps running task P4-M012-T0001"). That is correct
product behaviour: an imported task stays `running` until reviewed. The rehearsal now accepts each
imported task, as an operator would, before the next one.

## Amendment 2 (2026-09-25)

The next rehearsal run failed scenario 2 (delay and loss). The agent finished and the worker
committed `9ea3862`, but its `RESULT` upload and the lease release both failed with `Connection
reset by peer`, so **the worker abandoned finished work**. Two causes, found from both GhostPort
logs and then isolated:

1. **GhostPort (dogfooding finding 18).** The coordinator's GhostPort server logged `data: rejected
   ... (too many recent handshake attempts)`. Its per-IP limiter (10 per 60 s) counted *successful*
   handshakes too, and every tunnelled stream is a new handshake. An AgentForge worker opens one
   stream per request (idle polls every 2 s, renewals, results), so any worker over GhostPort is
   cut off within about 20 seconds, with or without chaos. Reproduced on a clean link: 10
   authenticated `PING`s passed and the 11th and 12th were rejected. The P0-M014 run was too short
   to hit it, and the P4-M010 live run bypassed GhostPort. REMOTE_WORKERS.md wrongly said the
   limiter counts *failed* handshakes. With the operator's approval this was fixed **in GhostPort**
   (`ebd7639`, released as `v0.1.2`): a handshake that authenticates a pinned peer is forgiven, and
   failed attempts still count in full. It has a real-daemon regression test (25 streams), and
   GhostPort CI and its release run are green.
2. **AgentForge (dogfooding finding 19).** A transport failure while sending `RESULT` abandons the
   run. Only `BUSY` is retried, so a single lost connection throws away finished, committed work,
   and the task has to be run again. Classification: semantic, the P4-M008 worker loop.

Fix (AgentForge): `WorkerClient::result` returns `ClientError`. The runner retries an
`Unreachable` result upload with the same capped backoff as claims (up to 10 attempts), with the
lease renewal still running, and reports each retry. When a retry after a lost response is refused,
the report says the first attempt may already have been imported and to check the coordinator.
Refusals stay fatal.

Tests (CLI, the real worker loop through a fault-injecting TCP proxy): a dropped `RESULT` request is
retried and imported; a dropped `RESULT` *response* leads to a refused retry, reported as possibly
imported, and the coordinator does hold the import.

The rehearsal now uses GhostPort `v0.1.2` from its published release (checksum-verified) instead of
the host's older build. Docs: REMOTE_WORKERS (GhostPort ≥ 0.1.2 is required for sustained workers,
and the corrected limiter note), DAEMON, and DOGFOODING (findings 18 and 19).
