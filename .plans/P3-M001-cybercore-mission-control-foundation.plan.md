# Plan: P3-M001 — Cybercore Mission Control foundation

Status: Approved
Milestone: P3-M001
Created: 2026-09-21
Owner: AgentForge project

## Goal

Create the first usable Cybercore Mission Control foundation: a private Cloudflare full-stack
control plane that persists projects, machines, agents, heartbeats, runs, and audit events while
providing a dashboard shell and a real-time event boundary for future Cybercore connectors.

## Context

AgentForge, Cyberdeck, Cyberplug, and Cyberterm currently operate as useful local tools with
separate static showcase pages. Mission Control is the next ecosystem layer: it should make local
state visible through one durable, operator-owned control plane without turning the existing
local-first tools into cloud-required products.

The first increment must establish a narrow, auditable ingress and observation boundary. A local
connector may send signed heartbeats and explicitly selected reports outbound to the service. The
service must not open inbound ports on an operator's machine, execute arbitrary remote commands, or
silently collect raw system data.

## Scope

- Create a new standalone repository named \`cybercore-mission-control\`; do not add the Worker
  application to the AgentForge runtime workspace.
- Scaffold a Cloudflare Workers full-stack application with static dashboard assets and a typed
  Worker API boundary.
- Add Wrangler configuration for local development and a separately named private deployment
  environment.
- Add D1 migrations and typed data access for:
  - projects;
  - machines;
  - agents;
  - scoped agent credentials;
  - heartbeats;
  - runs;
  - audit events.
- Implement bounded API routes for:
  - project creation;
  - agent registration;
  - heartbeat ingestion;
  - project and agent listing;
  - audit timeline retrieval;
  - project event subscription.
- Define a Durable Object event-channel interface for per-project live updates; the first
  implementation may publish heartbeat and audit notifications only.
- Add a responsive Mission Control dashboard shell showing project state, agent online/offline
  status, latest heartbeat, run/audit timeline, and an explicit empty state.
- Add deterministic seed/demo data for local development without requiring real credentials.
- Define the local connector contract for future Rust clients, including payload versioning,
  timestamp/nonce handling, scoped authentication, redaction defaults, and replay rejection.
- Add deployment, local development, privacy, threat-boundary, and recovery documentation.
- Record implementation and validation evidence in the new repository's changelog/release notes and
  retain this plan as the governing AgentForge milestone record.

## Non-goals

- No remote shell, process launch, file mutation, or agent command execution.
- No requirement that AgentForge, Cyberdeck, Cyberplug, or Cyberterm depend on the cloud service.
- No raw system telemetry by default; sensitive fields require explicit connector opt-in.
- No public multi-tenant billing, public sign-up, team roles, or marketplace features.
- No production credentials, API tokens, account IDs, or private reports in the repository.
- No use of a Direct Upload-only Cloudflare Pages project; Git-tracked Worker deployment remains the
  source of truth.
- No Rust connector implementation in this increment; only the versioned protocol contract and a
  deterministic simulator are required.
- No migration of existing GitHub Pages showcase sites.

## Architecture placement

The new repository is a separate Cloudflare application boundary:

- Workers serves the API and static dashboard assets.
- D1 stores durable relational metadata and audit records.
- Durable Objects coordinate per-project live event streams.
- R2 is reserved for a later artifact/report increment and is not required for M001 behavior.
- A future Rust connector makes outbound HTTPS requests using scoped credentials.
- Cloudflare Access protects the initial private operator dashboard; application-level public
  authentication is deferred until a separate plan.

AgentForge remains the local source of orchestration truth. Mission Control records observations and
operator-visible events; it does not become an authority to execute local work.

## Invariants

- Every mutating API operation records an audit event with actor/source, timestamp, request ID, and
  affected resource.
- Heartbeat ingestion is authenticated, bounded in size, versioned, and replay-resistant.
- A missing or invalid credential fails closed; no anonymous write endpoint exists.
- A heartbeat can mark an agent observed/online but cannot authorize a task or command.
- Event delivery is best-effort for live views; D1 remains the durable source of truth.
- The dashboard remains useful with zero agents, an empty project, or a disconnected live channel.
- Local development can run with seeded fixtures and no Cloudflare production account.
- Cloud credentials and secrets are supplied only through environment bindings or local secret
  stores and never committed.

## Expected implementation boundary

New repository \`cybercore-mission-control\` only:

- \`package.json\`
- \`wrangler.toml\` or \`wrangler.jsonc\`
- \`src/worker.ts\`
- \`src/api/**\`
- \`src/db/**\`
- \`src/durable/**\`
- \`src/dashboard/**\`
- \`migrations/**\`
- \`scripts/seed-demo.*\`
- \`tests/**\`
- \`docs/ARCHITECTURE.md\`
- \`docs/SECURITY.md\`
- \`docs/CONNECTOR_PROTOCOL.md\`
- \`README.md\`
- \`CHANGELOG.md\`

AgentForge repository changes in this plan-only checkpoint are limited to:

- \`.plans/P3-M001-cybercore-mission-control-foundation.plan.md\`
- \`.plans/ACTIVE\`

## Test-first matrix

- Type-check and lint the Worker/API/dashboard source.
- Apply D1 migrations to a disposable local database and verify the schema.
- Exercise project creation, agent registration, heartbeat authentication, replay rejection,
  bounded payload rejection, listing, and audit retrieval.
- Verify invalid credentials, missing fields, stale timestamps, duplicate nonces, and unknown
  project/agent IDs fail closed.
- Verify a valid heartbeat produces one durable audit event and one live event notification.
- Verify seeded demo mode renders with zero production bindings.
- Verify the dashboard handles empty, offline, and populated states.
- Run the full repository test/build gate and a production-mode Worker build before implementation
  closure.

## Implementation sequence

1. Create the standalone repository and capture the Cloudflare/Wrangler baseline.
2. Define the versioned protocol, D1 schema, credential scope, redaction defaults, and audit event
   model before writing route handlers.
3. Scaffold the Worker, local bindings, migration runner, and deterministic demo seed path.
4. Implement authenticated project/agent/heartbeat/audit routes with bounded validation and direct
   tests.
5. Add the Durable Object live-event interface and verify durable-first/live-second ordering.
6. Build the dashboard shell against typed API fixtures, then connect it to the local Worker.
7. Add security, recovery, local development, and deployment documentation.
8. Run all local gates, inspect generated artifacts, and record exact commit evidence.
9. Perform a separate closure checkpoint before starting the Rust connector milestone.

## Failure modes and classification

- Invalid schema, type, or route behavior is a semantic/compile failure and must be repaired in the
  new repository's bounded source/test files.
- Cloudflare account, binding, or deployment failures are infrastructure/configuration failures and
  must not be hidden by weakening local tests.
- Authentication, replay, or redaction defects are security failures; the affected write path must
  remain fail-closed until covered by a regression test.
- Live event loss is not treated as durable-state success; D1 audit persistence remains the
  acceptance authority.
- Toolchain or dependency drift is recorded separately from application behavior and must not cause
  an unrelated redesign.

## Quality gates

- Repository policy and formatting/lint checks.
- Type-check and unit/integration tests.
- Disposable local D1 migration and seed validation.
- Production Worker/static-asset build.
- Local API security matrix, including replay and payload limits.
- Exact deployment verification for the implementation commit in the selected Cloudflare environment.
- No production deploy or credential use occurs without explicit operator approval.

## Acceptance criteria

- [ ] A standalone \`cybercore-mission-control\` repository builds locally with no production secrets.
- [ ] D1 migrations create the project, machine, agent, heartbeat, run, and audit tables.
- [ ] Project creation, agent registration, authenticated heartbeat, listing, and audit retrieval
      work against disposable local bindings.
- [ ] Invalid credentials, replayed heartbeats, oversized payloads, and unknown resources fail
      closed with tested responses.
- [ ] Each accepted heartbeat creates durable audit evidence and a live event notification.
- [ ] The dashboard renders empty, offline, demo, and populated states.
- [ ] Connector protocol and privacy/threat boundaries are documented.
- [ ] Full local validation and exact implementation deployment evidence are recorded before closure.
- [ ] No existing AgentForge, Cyberdeck, Cyberplug, or Cyberterm behavior is changed.

## Completion record

Implementation commit: pending
Exact deployment/validation evidence: pending
Completed: pending
Notes: Plan-only approval checkpoint. Implementation must occur in the separate
\`cybercore-mission-control\` repository after this plan is committed.
