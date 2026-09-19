# AgentForge Project State

## Current release

- Workspace version: `0.0.1`
- Release line: Phase 0 foundation

## Current phase

Phase 0 — reliable local orchestration foundation.

## Active milestone

- `P0-M002` — Governance and agent contract.
- Active plan: `.plans/P0-M002-governance-agent-contract.plan.md`.
- Plan status: Approved.
- Implementation status: not started.
- Base main commit: `1910967c9989d7f705f71be7039e6877493e3aa7`.

## Recently completed milestone

- `P0-M001` — Repository bootstrap.
- Status: Complete.

## Current capability

AgentForge has a compiling Rust workspace, core crate, CLI, daemon entry point, xtask validation,
plan-first project controls, deterministic local gates, and permanent milestone/ADR registries.

P0-M002 is defining the governance contract required before autonomous task execution exists.

## P0-M002 architecture boundary

P0-M002 defines roles, capabilities, human approval boundaries, task/result semantics, and
concurrent ownership rules.

It does not schedule tasks, spawn agents, create worktrees, access secrets, or contact external
model providers.

## Next exact action

Validate this exact Approved plan-only checkpoint with the full local gate.

Do not begin Rust implementation until that exact checkpoint is green.
