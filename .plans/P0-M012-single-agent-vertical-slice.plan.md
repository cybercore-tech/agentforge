# Plan: P0-M012 — Single-agent vertical slice

Status: Draft
Milestone: P0-M012
Created: 2026-09-19

## Goal

Connect one approved task through policy validation, managed worktree inspection, provider-neutral
agent execution, gate evidence, review handoff, and audit evidence without introducing scheduling or
multi-agent integration.

## Non-goals

- No scheduler, parallel workers, remote execution, automatic merge/deploy, retries with elevation,
  or new persistence/wire format.

## Boundary

Compose existing core, state, worktree, policy, adapter, gate, and audit crates behind one explicit
orchestration flow. Keep side effects ordered and fail closed; acceptance and task transitions remain
caller-controlled. Add focused end-to-end fixture tests with isolated temporary repositories.

Expected files: this plan, `.plans/ACTIVE`, project state/handoff/milestones, ADR-0017 and registry,
`crates/agentforge-orchestrator/**`, workspace manifests/lockfile, and `docs/VERTICAL_SLICE.md`.

## Test matrix

| Stage | Evidence |
| --- | --- |
| Approval/policy | Unapproved or over-privileged task stops before process launch |
| Worktree | Identity and cleanliness are rechecked before execution |
| Agent | Adapter report binds to task identity and remains bounded |
| Gates/review | Gate evidence and review handoff are explicit and ordered |
| Audit | Started, finished, gate, and handoff events are recorded |
| Failure | Any stage failure stops later side effects and preserves evidence |

## Acceptance

Separate approval, full gate, exact CI, closure, merge, and post-merge verification are required.
