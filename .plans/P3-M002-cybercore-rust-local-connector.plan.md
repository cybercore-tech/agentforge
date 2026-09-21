# Plan: P3-M002 — Cybercore Rust local connector

Status: Approved
Milestone: P3-M002
Created: 2026-09-21
Owner: AgentForge project

## Goal

Build the first real local-to-cloud connector for Cybercore Mission Control: a small Rust
component that reads explicit local configuration and sends bounded, replay-resistant agent
heartbeats outbound to the Mission Control Worker without granting the cloud service local
execution authority.

This is the next increment after P3-M001's Worker/D1 control-plane foundation. It should make a
real local machine observable while preserving the project's fail-closed and operator-controlled
security model.

## Context

The standalone `cybercore-mission-control` repository now provides:

- project and agent registration;
- authenticated heartbeat ingestion;
- timestamp, payload-size, and unique-nonce validation;
- durable D1 audit evidence; and
- a read-only dashboard/API surface.

The missing boundary is a supported local client. The connector must be an outbound-only
publisher, not an agent executor, remote shell, tunnel, or daemon with cloud-controlled commands.

## Scope

Implementation occurs in the standalone `cybercore-mission-control` repository, in a new
`connector/` Rust package or workspace member. AgentForge changes in this checkpoint are limited
to this plan and the active-plan pointer.

The implementation may include:

- a Rust 2021 CLI with a one-shot heartbeat command and a bounded periodic mode;
- explicit configuration for Mission Control base URL, project ID, agent ID, credential source,
  display name, and interval;
- a local credential file or environment handoff with permission checks and redaction;
- an HTTPS client that sends only the documented heartbeat contract;
- cryptographically strong or OS-random unique nonces and UTC timestamps;
- bounded exponential retry for transient transport/server failures;
- deterministic request construction and test fixtures;
- an optional local mock endpoint or contract-test harness; and
- operator documentation covering setup, rotation, stop behavior, and limits.

## Security and authority invariants

- The connector opens outbound connections only. It must not listen on a local or remote socket.
- Mission Control receives heartbeat metadata only; no endpoint may execute a remote command,
  upload arbitrary files, or return executable instructions.
- Credentials are read from an explicit local source, never printed, serialized into logs, or
  included in error strings. File-backed credentials must reject unsafe permissions where the
  platform supports that check.
- Heartbeat bodies are bounded to the Worker contract (including the 16 KiB payload limit) and
  must contain a unique nonce and bounded timestamp.
- Retry count, backoff, request timeout, and periodic interval are bounded and configurable only
  within safe limits. A failed request must not spin indefinitely.
- TLS/HTTPS is required for non-loopback endpoints. Insecure transport is permitted only for an
  explicitly documented local test fixture.
- The connector must fail closed on malformed configuration, stale/future timestamps, oversized
  payloads, invalid credentials, non-success responses, or protocol drift.

## Non-goals

- No remote execution, task dispatch, shell access, reverse tunnel, or arbitrary file transfer.
- No raw process output, source tree upload, or unrestricted telemetry by default.
- No automatic modification of an AgentForge project, task state, worktree, or local Git history.
- No production Cloudflare deployment, secret provisioning, or multi-tenant IAM redesign.
- No native GUI/TUI in this increment; the CLI and documented service mode are sufficient.
- No promise of release readiness or crates.io publication.

## Expected files and boundary

Standalone repository boundary:

- `connector/**` (or an equivalently named Rust workspace member);
- connector protocol/configuration documentation;
- connector-specific tests and fixtures;
- root README/CHANGELOG updates needed to explain the local connector; and
- dependency lockfile changes required by the connector package.

AgentForge repository boundary for this plan-only checkpoint:

- `.plans/P3-M002-cybercore-rust-local-connector.plan.md`;
- `.plans/ACTIVE`;
- no AgentForge runtime, CLI, workflow, or dependency changes.

## Test and validation matrix

Required before implementation closure:

- `cargo fmt --check` and `cargo check --locked` for the connector workspace;
- unit tests for configuration bounds, redaction, nonce/timestamp generation, payload limits,
  status classification, and bounded retry behavior;
- a mock HTTP test proving the exact registration/heartbeat request contract;
- tests proving stale and future timestamps, duplicate nonces, oversized payloads, invalid
  credentials, and non-HTTPS production endpoints fail closed;
- tests proving credentials never appear in captured logs or error text;
- a one-shot local smoke run against the Mission Control Worker or protocol fixture;
- a periodic-mode stop test proving cancellation is cooperative and bounded; and
- exact-SHA CI verification for the implementation and closure commits if the standalone project
  adds or changes its workflow.

Validation must not deploy to production or require a real operator credential. Local D1/Worker
fixtures and synthetic credentials are sufficient.

## Acceptance criteria

1. A documented command can send one authenticated heartbeat from a real local machine to a local
   or explicitly configured Mission Control endpoint.
2. The Worker records the heartbeat and audit evidence with the expected agent/project identity.
3. The connector has no inbound listener and no code path that interprets cloud responses as
   commands.
4. Credential handling, request bounds, nonce/timestamp rules, HTTPS policy, and retry limits are
   covered by automated tests.
5. Failure output is actionable but does not disclose credentials or unbounded response data.
6. Documentation clearly labels the connector as experimental/non-release-ready and explains
   revocation, shutdown, and recovery procedures.
7. The implementation is committed separately from this approved plan checkpoint and verified at
   its exact commit SHA.

## Rollback and recovery

- Stop the connector process; no cloud-side command is required.
- Revoke or replace the local credential in Mission Control and delete the local credential file
  if compromise is suspected.
- Disable the connector package or revert its standalone-repository commit without changing
  AgentForge state.
- Preserve failed request metadata without retaining secrets or unbounded response bodies.

## Completion record

Implementation commit: pending
Exact validation evidence: pending
Completed: pending
Notes: Plan-only approval checkpoint. Implementation must occur in the standalone
`cybercore-mission-control` repository after this plan is committed.
