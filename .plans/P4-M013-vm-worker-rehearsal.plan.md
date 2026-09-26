# Plan: P4-M013 — VM worker rehearsal: a real second kernel

Status: Draft
Milestone: P4-M013
Created: 2026-09-25
Owner: AgentForge project

## Goal

Rehearse the two-machine remote-worker run with a **virtual machine** as the worker host and this
machine (`darkbox`) as the real coordinator. `scripts/rehearse-vm-worker` boots a clean Arch Linux
VM under KVM, with its own kernel, filesystem, and network stack, reaching the host only through
QEMU's user-mode network. It sets the VM up with the same `worker-host-setup` the operator's
Omarchy machine will run, and checks the same scenarios as the container rehearsal (P4-M012):
- a clean remote run imported at the exact commit;
- a run under delay and loss with lease renewals;
- an idle partition the worker survives in one process.

This is the closest to the real two-machine run this hardware allows.

## Operator decisions (2026-09-25)

- A VM stand-in was asked for ("on like a vm or something?"). The operator installed `qemu-base`.
- **Scripted agent only:** no Claude Code credentials are copied into the VM. The real-agent remote
  path was already proven on one host (P0-M014).

## Non-goals

- No Omarchy desktop install in the VM: a headless Arch cloud image covers everything a worker host
  needs, and the Omarchy install stays for the real machine.
- No bridged or tap networking, and no host network changes (which would need root): QEMU user-mode
  networking only.
- No CI job (needs KVM and a 560 MB image); it is an operator rehearsal with recorded evidence.

## Context

The container rehearsal (P4-M012) found four real defects, but containers share the host's kernel.
A VM adds what the real second machine brings: a different kernel, a separate network stack, and a
real network hop (virtio NIC and SLIRP) instead of a bridge between namespaces. KVM is usable by the
operator's user (`/dev/kvm` 0666; `query-kvm` enabled), and QEMU 11.1.1 is installed.

## Architecture placement

- **`scripts/rehearse-vm-worker [--keep] [--forge-archive <tar.gz>] [--ghostport <binary>]`**:
  1. **Inputs, verified:** `forge`/`forged` from the published `v0.3.1` (`sha256sum` plus `gh
     attestation verify`), GhostPort `v0.1.2` from its release (`sha256sum`), and the official Arch
     cloud image from `geo.mirror.pkgbuild.com`, checked against its published `.SHA256`. The
     image is cached in `${XDG_CACHE_HOME:-~/.cache}/agentforge-rehearsal/` (560 MB, reused, never
     modified: the VM boots from a throwaway qcow2 overlay).
  2. **VM:** a scratch ed25519 key; a cloud-init seed ISO (built with `xorriso`) creating the user
     `worker` with passwordless sudo and that key, and installing `git` and `iproute2`; QEMU with
     `-enable-kvm`, 2 vCPUs, 2 GiB, virtio disk and NIC, user-mode networking with SSH forwarded on
     a free loopback port, and the serial console logged. The script waits for SSH and the
     cloud-init marker.
  3. **Coordinator (this host, in a scratch directory):** a project with tasks, `forged` with the
     worker API on loopback, `git daemon` on a loopback port as the origin, a GhostPort server on
     loopback ports, and `worker-bundle` with `--server 10.0.2.2` (QEMU's alias for the host's
     loopback, from inside the VM).
  4. **Worker (the VM):** the bundle, `forge`, `ghostport`, the rehearsal agent, and
     `worker-host-setup` are copied in with `scp`. The first setup fails only the endpoint check; the
     printed `[[peers]]` entry is added to the host's GhostPort server; both ends start; the second
     setup passes the doctor.
  5. **Scenarios** 1–3, with `tc netem` on the VM's default-route interface (via sudo in the VM).
  6. Evidence: the VM's and the host's `uname -r` (different kernels), PASS/FAIL per check, a
     transcript, and the VM console log.
  7. Cleanup: every host process (QEMU, GhostPort server, `git daemon`, `forged`) is stopped by its
     recorded PID or command, never by pattern, and the overlay and work directory are removed
     (unless `--keep`). The cached base image stays.
- **Reuse:** `contrib/rehearsal/rehearsal-agent` (P4-M012) is the agent.
- **Docs:** REMOTE_WORKERS (the VM rehearsal), DOGFOODING (the run log), and OPERATIONS.

## Invariants

- No host configuration, keys, or services change. Everything is in a scratch directory, plus the
  documented image cache. Host processes are started by the script and stopped by PID.
- No operator credentials enter the VM.
- The VM's disk is a throwaway overlay; the cached image is verified before every use.

## ADRs

None.

## Public API / CLI

A maintainer script.

## Compatibility analysis

Additive.

## Dependency analysis

Host tools only: `qemu-system-x86_64`, `qemu-img`, `xorriso`, OpenSSH, and `gh`. The script fails
early and names anything missing.

## Expected file boundary

- `.plans/P4-M013-vm-worker-rehearsal.plan.md`, `.plans/ACTIVE`
- `scripts/rehearse-vm-worker`
- `docs/REMOTE_WORKERS.md`, `docs/DOGFOODING.md`, `docs/OPERATIONS.md`
- closure records: `docs/MILESTONES.md`, `CHANGELOG.md`, `README.md`, `PROJECT_STATE.md`,
  `AGENT_HANDOFF.md`

## Test-first matrix

The rehearsal is the test. It must PASS:
- inputs verified (forge checksum and attestation; GhostPort checksum; image checksum);
- the VM boots under KVM, and its kernel differs from the host's;
- the first setup fails only the endpoint check, and the second passes the doctor;
- scenario 1: imported at the worker's exact commit;
- scenario 2: RTT rise ≥ 100 ms measured, imported, and at least one renewal;
- scenario 3: at least two `coordinator unreachable` reports, recovery, a claim and import after it,
  and one worker process throughout;
- cleanup: no QEMU, GhostPort, `git daemon`, or `forged` process from the run remains.

`--break-heal` must fail exactly scenario 3's recovery checks.

## Implementation sequence

1. Commit this draft plan.
2. Approve and activate it in a separate checkpoint.
3. The script, built up and run stage by stage; a full run; the `--break-heal` run; docs.
4. Gate; commit; push; CI plus a repeat.
5. Close; tag after the closure CI is green.

## Failure modes

- A product defect found in the VM: stop, classify, amend, and fix with a test.
- Mirror, download, or boot problems: the script fails early with the reason and cleans up.

## Documentation impact

REMOTE_WORKERS, DOGFOODING, OPERATIONS; CHANGELOG at closure.

## Quality gates

- `./scripts/gate.sh full` and a passing VM rehearsal;
- push CI plus a dispatched repeat.

## Acceptance criteria

- [ ] `scripts/rehearse-vm-worker` passes every check with a KVM VM as the worker host, and
      `--break-heal` fails as designed.
- [ ] The VM is set up only by `worker-host-setup` from a bundle, as the real machine will be.
- [ ] Docs; CI evidence; closed and tagged correctly.
