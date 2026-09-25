# Plan: P4-M007 — Authenticated remote-worker channel over GhostPort

Status: Draft
Milestone: P4-M007
Created: 2026-09-24
Owner: AgentForge project
Implementer: operator-authored (the first network-facing authority boundary)

## Goal

Let a worker on another machine claim, renew, and release its leases, and receive the exact task
contract and base commit, over an authenticated, encrypted channel. `forged` serves a worker API
on loopback only. GhostPort (Noise KK with pinned keys) carries it between machines. AgentForge
authenticates each request as a specific registered worker with a per-worker secret. This is the
transport half of remote execution. P4-M008 adds remote execution and exact-SHA result
acceptance.

## Non-goals

- No remote execution or result import in this milestone (P4-M008).
- No crypto code or crypto dependencies inside AgentForge. Transport security is GhostPort's job,
  and AgentForge keeps its loopback-only listeners.
- No worker-to-coordinator Git access. The remote worker gets the base commit through its own
  clone of the shared origin.
- No automatic GhostPort configuration. A documented recipe is provided and verified.

## Context

P4-M004 to P4-M006 made leases operable, runnable by a same-host worker, and optionally
auto-dispatched. Everything assumes the worker shares the project directory. GhostPort
(`cybercore-tech/ghostport`) already provides pinned-key Noise KK tunnels, NAT-friendly
client-dials-server connections, and per-peer link restrictions. It needs tokio and snow, so it
runs as a separate process in front of AgentForge instead of being linked into it. That also keeps
AgentForge's "loopback only" rule: GhostPort terminates the network side and forwards to a
loopback port.

Two layers of authentication, deliberately:

1. **GhostPort:** the machine must hold a pinned Noise key and is restricted to its own link.
2. **AgentForge:** each request carries the worker ID and that worker's secret, so a compromised
   or misrouted tunnel still cannot act as another worker.

## Architecture placement

- **Contract wire format (`agentforge-adapter`):** `parse_task_prompt` decodes the existing
  `agentforge-task-prompt-v1` document produced by `render_task_prompt` back into an `AgentTask`.
  It is strict, bounded, and fail-closed (the same rules as the Claude bridge's decoder). A
  render-then-parse round trip is identity for every field.
- **Worker secrets:**
  - `.forge/workers/<id>.secret` holds 64 lowercase hex characters (256 bits).
  - `forge worker enroll <root> <id>` requires the worker profile to exist. It generates the
    secret from the OS CSPRNG (`/dev/urandom` on Unix; on Windows it refuses with instructions to
    supply 32 random bytes as hex). It writes the file with `0600` permissions on Unix, refuses to
    overwrite, and prints the secret once for the worker host.
  - On Unix the coordinator refuses a secret file readable by group or others.
  - Secrets are compared in constant time.
- **Worker API server (`agentforge-daemon`):**
  - Enabled by `.forge/worker-api.conf` with `bind=127.0.0.1:<port>`, which must be loopback.
    There is no default port, and the API is off when the file is missing.
  - `forged serve` then runs a second loopback listener thread for protocol `AFW1`, one request
    per connection (the same bounded framing style as `AFD1`).
  - Each request starts with `AFW1\t<VERB>\t<worker-id>\t<secret>`.
  - A failed authentication gets a generic `ERR unauthorized` after a fixed 200 ms delay. It is
    logged to stderr but not to the audit log (no audit flooding).
  - Verbs:
    - `CLAIM` claims this worker's next claimable lease (the P4-M005 selection rule), recorded as
      `claimed` with actor `worker:<id>` and `channel=remote`. The response gives the lease ID,
      generation, expiry, the coordinator's current `HEAD` commit as the exact base, and the
      contract document (length-prefixed, 64 KiB bound). If there is nothing to claim, it answers
      `NONE`.
    - `RENEW <lease> <ttl-ms>` and `RELEASE <lease>` act only on this worker's own leases, and
      are audited with `channel=remote`.
  - Requests use the lease lock and coordinated audit appends, so they are safe alongside the
    daemon tick and CLI. Remote claims do not take the execution slot: they append small audit
    records only, and P0-M013 makes that safe.
- **Worker API client:** `agentforge_daemon::worker_api::{WorkerClient, RemoteClaim}`. CLI:
  `forge worker remote claim|renew|release --endpoint <host:port> --worker <id> --secret-file
  <path> [...]`. It prints the claim and can write the contract document to a file.
- **GhostPort recipe (`docs/REMOTE_WORKERS.md`):** the coordinator's GhostPort server gets one
  `[[peers]]` entry per worker host, each allowed only its own forward link, targeting the worker
  API's loopback port. The worker host's GhostPort client listens on its own loopback port, and
  `forge worker remote ... --endpoint 127.0.0.1:<local-port>` goes through the tunnel.

## Invariants

- AgentForge still binds only loopback addresses.
- Every worker-API action is authenticated as exactly one registered worker and limited to that
  worker's own leases.
- The task contract a remote worker receives is byte-identical to what a local adapter would give
  the agent.
- Unauthenticated or malformed requests change nothing.

## ADRs

ADR-0050 "Remote-worker channel": GhostPort as an external transport, two-layer authentication,
loopback-only, the secret files, the AFW1 protocol, and the contract document as the wire format.

## Public API / CLI

`parse_task_prompt`, `forge worker enroll`, `.forge/worker-api.conf`, the AFW1 protocol, `forge
worker remote claim|renew|release`, and `worker_api::WorkerClient`.

## Compatibility analysis

Additive and off by default. `channel=remote` is a new optional field on `LeaseRecorded`.

## Dependency analysis

None in AgentForge (std only). GhostPort is an operator-installed external tool, not a build
dependency.

## Expected file boundary

- `.plans/P4-M007-remote-worker-channel.plan.md`, `.plans/ACTIVE`
- `crates/agentforge-adapter/src/lib.rs`, `crates/agentforge-adapter/tests/*.rs`
- `crates/agentforge-operator/src/leases.rs`, `crates/agentforge-operator/src/worker.rs`,
  `crates/agentforge-operator/src/lib.rs`, `crates/agentforge-operator/src/secrets.rs`,
  `crates/agentforge-operator/tests/*.rs`
- `crates/agentforge-daemon/src/lib.rs`, `crates/agentforge-daemon/src/worker_api.rs`,
  `crates/agentforge-daemon/tests/*.rs`
- `crates/agentforge-cli/src/main.rs`, `crates/agentforge-cli/tests/*.rs`
- `Cargo.lock`
- `docs/adr/ADR-0050-remote-worker-channel.md`, `docs/REMOTE_WORKERS.md`, `docs/DAEMON.md`,
  `docs/OPERATIONS.md`
- closure records: `docs/MILESTONES.md`, `CHANGELOG.md`, `README.md`, `PROJECT_STATE.md`,
  `AGENT_HANDOFF.md`

## Test-first matrix

- **`parse_task_prompt`:**
  - a round trip for a fully populated task, including multiline and Unicode fields;
  - rejects a wrong header, a truncated field, a bad length, an unknown or reordered field,
    trailing bytes, an unknown capability or approval, and an oversize document.
- **Secrets:** enroll writes 64 hex characters with `0600`, and refuses an existing secret or an
  unregistered worker. Loading refuses group- or other-readable files (Unix). Comparison is
  correct in both directions.
- **Worker API (in-process server):**
  - an authenticated CLAIM returns the lease and contract, and the parsed contract equals the
    task;
  - NONE when there is nothing to claim;
  - a wrong secret, an unknown worker, and a malformed frame give `unauthorized` or an error with
    no state or audit change;
  - RENEW and RELEASE of another worker's lease are refused;
  - `claimed` and `released` carry `channel=remote`;
  - a non-loopback `bind` is refused at startup;
  - the API is off without the config file.
- **CLI:** `forge worker enroll`, then `forge worker remote claim`, `renew`, and `release` against a
  running `forged` with the worker API enabled.
- **GhostPort dogfood (manual, recorded):** build GhostPort, generate keys, run a server and a
  client on localhost, and run the claim, renew, and release through the encrypted tunnel. A wrong
  GhostPort key cannot connect, and a wrong AgentForge secret is refused.

## Implementation sequence

1. Commit this draft plan.
2. Approve and activate it in a separate checkpoint.
3. `parse_task_prompt`, then secrets and enroll, then the worker API server and client, then the
   CLI, with tests at each step.
4. ADR and docs (including the GhostPort recipe). Full gate plus package preflight. GhostPort
   dogfood.
5. Push and verify CI (push plus two repeats); close; tag.

## Failure modes

- A leaked worker secret: the attacker still needs a GhostPort key for that worker's link.
  Rotation means deleting the `.secret` file and enrolling again.
- A remote worker that claims and then disappears: its lease expires (daemon tick). The task stays
  `pending`, is not auto-re-dispatched (P4-M006), and the operator decides.
- Clock differences between hosts do not matter: expiry is judged on the coordinator's clock only.

## Documentation impact

REMOTE_WORKERS ("Workers on other machines", with the GhostPort recipe), DAEMON (the worker API
listener), OPERATIONS (commands, secret rotation, recovery), and ADR-0050. README, CHANGELOG, and
MILESTONES at closure.

## Quality gates

- `./scripts/gate.sh full` and `./scripts/package-preflight`;
- push-triggered CI plus two dispatched repeats.

## Acceptance criteria

- [ ] A registered, enrolled worker can claim, renew, and release its leases through `forged`'s
      loopback worker API, and receives the exact contract and base commit.
- [ ] Requests are authenticated per worker and limited to that worker's leases; bad requests
      change nothing.
- [ ] Verified through a real GhostPort tunnel.
- [ ] ADR-0050 and docs; CI evidence; closed and tagged.

## Completion record

Implementation commit:
CI run:
CI result:
Completed:
Notes:
