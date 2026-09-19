# AgentForge Project Specification

## Mission

AgentForge is a local-first, model-agnostic development environment for building applications with
specialized AI agents under explicit engineering controls.

The system must make agents interchangeable while keeping workflow, evidence, permissions, and
project state durable.

## Product shape

AgentForge consists of:

- `forge` — operator CLI and future TUI;
- `forged` — local orchestration daemon;
- `agentforge-core` — domain models and orchestration primitives;
- project-local `.forge/` state and policy;
- plans, agent contracts, gates, and audit records;
- adapters for external coding agents and tools;
- isolated Git worktrees for implementation tasks.

## Foundational principles

1. Human approval at consequential boundaries.
2. Plans precede implementation.
3. Every agent task has an explicit file and capability boundary.
4. Agents do not share dirty working trees.
5. Git history is evidence.
6. CI failures are classified before repair.
7. Exact-head validation matters more than partial green runs.
8. Repair forward; do not erase failure history.
9. Models are replaceable workers, not the source of project truth.
10. Durable state lives in files/databases, not chat memory.
11. Least privilege is the default.
12. Local operation must remain useful without a cloud dependency.

## Phase 0 success condition

Phase 0 is complete when AgentForge can take one approved task, assign it to one configured agent
inside an isolated worktree, run deterministic gates, classify a failing CI result, record an audit
trail, and produce a review-ready handoff without bypassing policy.
