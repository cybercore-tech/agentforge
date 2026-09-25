# Plan: P4-M012 — Two-host rehearsal in containers, with network chaos

Status: Draft
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
