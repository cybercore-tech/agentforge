# Plan: P3-M004 — Mission Control production-readiness foundations

Status: Completed
Milestone: P3-M004
Created: 2026-09-21
Owner: AgentForge project

## Goal

Move Cybercore Mission Control from a released alpha observation plane toward a controlled,
staging-first deployment that can be operated safely without weakening AgentForge's local authority
boundary. This milestone establishes the production-readiness foundations and evidence needed before
any real Cloudflare deployment or operator credential is used.

The outcome is a repeatable local/staging preflight, explicit environment separation, hardened
authentication and data paths, end-to-end integration evidence, and an operator runbook. It does not
grant Mission Control authority to execute commands or mutate local AgentForge state.

## Context

P3-M001 established the Worker, D1, Durable Object event channel, dashboard, and API boundaries.
P3-M002 added the outbound-only Rust connector. P3-M003 shipped the first checksummed `v0.1.0`
release. The remaining high-value gap is operational: deployment configuration, migration and secret
preflight, staging verification, and documented recovery are not yet proven as one workflow.

## Scope

Implementation occurs in the standalone `cybercore-mission-control` repository. AgentForge changes
are limited to this plan, `.plans/ACTIVE`, and closure evidence.

The implementation may include:

- explicit local, staging, and production configuration boundaries with safe placeholders and no
  committed account IDs, tokens, or credentials;
- a deployment/preflight command or script that validates bindings, migration state, required secret
  names, dashboard access assumptions, and release provenance before any deploy action;
- schema and API hardening for bounded inputs, authentication failure behavior, request correlation,
  and durable audit integrity, with focused tests for duplicate/replay and unauthorized paths;
- a local end-to-end smoke harness covering migration, registration, authenticated heartbeat,
  audit persistence, event delivery, and dashboard health using disposable state;
- a staging-oriented GitHub workflow or documented manual gate that runs validation and requires
  explicit environment approval before deployment, without using real credentials in CI;
- an operator runbook for first deployment, secret rotation, migration rollback, incident response,
  connector upgrade/rollback, and disabling public access; and
- security/support documentation that clearly separates observation, control, and future command
  authority.

## Non-goals

- No production Cloudflare deployment, real account ID, API token, admin token, or private dashboard
  exposure during implementation.
- No remote command execution, shell transport, file transfer, reverse tunnel, or cloud-to-local
  mutation capability.
- No crates.io publication, multi-tenant billing, report upload, or broad telemetry redesign.
- No replacement of AgentForge's local task, worktree, approval, or audit authority.

## Invariants

- Every deployment path fails closed when required environment data, migration state, or secrets are
  missing or malformed.
- Credentials never appear in source, artifacts, logs, manifests, dashboard payloads, or test output.
- Agent credentials remain scoped, hashed at rest, single-use on registration, and rejected on replay,
  stale timestamps, malformed payloads, or unauthorized project scope.
- D1 migrations are deterministic and forward-only; rollback instructions never imply destructive
  mutation without an explicit backup/recovery step.
- Live events remain a convenience view; durable D1 audit/state is authoritative.
- The connector remains outbound-only and observation-only.

## Expected standalone repository boundary

- `wrangler.jsonc` and environment-safe configuration templates;
- `scripts/**` or equivalent deployment/preflight and local smoke tooling;
- `worker/**`, `migrations/**`, and tests for bounded/authenticated API behavior;
- `.github/workflows/**` for validation and explicitly gated staging deployment shape;
- `README.md`, `docs/ARCHITECTURE.md`, `docs/SECURITY.md`, `docs/RELEASE.md`, and a deployment/
  operations runbook;
- `CHANGELOG.md` if user-visible operational behavior changes; and
- `.gitignore` updates needed to exclude local environment files and generated reports.

AgentForge files in this milestone:

- `.plans/P3-M004-mission-control-production-readiness.plan.md`;
- `.plans/ACTIVE`; and
- closure evidence in `PROJECT_STATE.md`, `AGENT_HANDOFF.md`, and `docs/MILESTONES.md`.

## Validation matrix

Required before closure:

- existing Rust connector format/check/test/Clippy and package gates;
- Worker type-check, lint, migration validation, and local D1/DO smoke tests;
- negative-path tests for missing bindings, missing secrets, unauthorized requests, replayed
  nonces, malformed payloads, and oversized bodies;
- preflight output proving no credential or account identifier is emitted;
- exact-SHA GitHub CI for the full supported matrix;
- workflow syntax and policy validation for any deployment workflow; and
- a documented dry-run deployment with no real Cloudflare mutation.

Infrastructure failures must be classified separately from application semantics and rerun only at
the same exact SHA. No hook, test, lint, security, or deployment gate may be bypassed.

## Acceptance criteria

1. A fresh checkout can run a documented preflight that validates local/staging configuration and
   fails clearly before a deployment when required inputs are absent.
2. Local end-to-end smoke proves migration, registration, heartbeat, durable audit, live event, and
   dashboard health behavior using disposable state.
3. Authentication, replay, bounds, and project-scope negative paths are covered by automated tests.
4. A staging deployment path exists behind explicit environment approval and cannot use production
   credentials accidentally.
5. Runbooks document secret rotation, migration recovery, connector rollback, access disablement,
   and incident evidence collection.
6. Exact-SHA CI is green for the implementation and closure commits, with no production mutation.

## Implementation sequence

1. Add the approved plan checkpoint and validate repository policy.
2. Inventory current Worker bindings, migrations, secrets, and smoke coverage; record gaps.
3. Implement environment-safe preflight/configuration and negative-path tests.
4. Strengthen local end-to-end smoke and durable audit/event assertions.
5. Add the explicitly gated staging workflow shape and operator runbook.
6. Run the full gate, exact-SHA CI, and dry-run deployment evidence.
7. Close the milestone only after recording exact commits, runs, and the no-production-mutation
   boundary.

## Rollback and recovery

- Revert the standalone implementation commit without changing AgentForge state.
- Disable the staging workflow or remove its environment approval gate if validation is incomplete.
- Revoke and replace any test/staging credential, restore the previous verified connector artifact,
  and preserve audit evidence.

## Completion record

Implementation commits: `48863e8`, `d2029f1`, `c5d92b4`, `9533c51`, `e9b76c3`, and `4a435c1` in the standalone `cybercore-mission-control` repository; remote `main` resolves to `4a435c1c6ac90f78986aa39c04235dae6552d8aa`.
CI runs: `35608217415` (preflight foundation), `35611397659` (deterministic platform gate), `35612605311` (negative-path and staging workflow increment), and `35614113529` (live-event verification); each is green for its exact implementation SHA.
CI result: Worker TypeScript/preflight/Wrangler staging binding validation and the Ubuntu, macOS, and Windows Rust connector matrix are green for the final implementation SHA.
Completed: 2026-09-21
Notes: Local disposable validation passed migration, registration, authenticated heartbeat, durable audit, authenticated live event delivery, malformed/stale/future/oversized input rejection, credential/project-scope failures, nonce replay rejection, protocol tests, and Rust package gates. A manually confirmed staging-only dry-run workflow is present. No Cloudflare deployment, production credential, real account ID, or remote command authority was used. Automatic CI intentionally excludes the long-lived local Wrangler smoke process because repeated CI runs exposed a platform process-lifecycle hang; the same smoke passed in a clean local checkout and remains operator-runnable via `npm run smoke:local`.
AgentForge closure evidence: commit `571c8d2`; exact AgentForge CI `35621709316` and Pages deployment `35621709313` are green for that closure SHA.
