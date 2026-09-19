# Plan: P0-M002 — Governance and agent contract

Status: Approved
Milestone: P0-M002
Created: 2026-09-19

## Goal

Define AgentForge's permanent agent-governance model before orchestration code is allowed to make
autonomous decisions.

P0-M002 establishes:

- canonical agent roles and responsibilities;
- role-independent capability grants;
- human approval boundaries;
- structured task and result contracts;
- path ownership and concurrent-agent rules;
- reviewer independence;
- escalation and refusal behavior;
- core Rust types that represent these concepts without depending on any model provider.

## Non-goals

- No task scheduler.
- No SQLite or durable task database.
- No Git worktree manager.
- No agent process spawning.
- No MCP gateway.
- No external model/provider adapter.
- No CI API integration.
- No secrets backend.
- No deployment automation.
- No TUI/HUD.
- No serialization dependency.
- No provider-specific prompt format.

## Context

AgentForge treats models as replaceable workers. That requires the system to define authority and
responsibility independently of Codex, Claude, Gemini, local models, or future providers.

An agent must know:

- what role it is performing;
- what task it owns;
- what files it may modify;
- what tools/capabilities it may use;
- what evidence it must produce;
- what actions require human approval;
- what it must do when its scope is insufficient.

P0-M002 freezes those semantics before P0-M004 task state, P0-M005 worktree isolation, or P0-M006
agent adapters depend on them.

## Architecture placement

```text
operator / human approver
        |
        v
governance policy
        |
        +--> agent role
        +--> capability grants
        +--> approval boundaries
        +--> path ownership
        |
        v
structured AgentTask
        |
        v
future agent adapter / worker
        |
        v
structured AgentResult
        |
        v
review / gate / integration
```

P0-M002 defines the contract. Later milestones execute it.

## Canonical roles

The initial role registry is:

- Planner
- Architect
- Researcher
- Implementer
- Tester
- Reviewer
- SecurityReviewer
- Integrator
- ReleaseManager

Roles describe responsibility, not model identity.

One model may perform different roles on different tasks, but one task execution has one primary
role.

## Role responsibilities

### Planner

May:

- inspect project state;
- decompose an approved milestone;
- define dependencies, risks, tests, and expected file boundaries.

Must not:

- implement production code in the same plan-approval change.

### Architect

May:

- define architecture boundaries;
- author or update ADRs when approved;
- identify interfaces and ownership.

Must escalate:

- irreversible architecture changes;
- scope that crosses an explicit human approval boundary.

### Researcher

May:

- gather external evidence;
- summarize specifications, APIs, compatibility, and prior art.

Must:

- distinguish evidence from inference;
- preserve source provenance where the project requires it.

### Implementer

May:

- modify only task-owned paths;
- add tests inside the approved task boundary;
- run local gates.

Must not:

- merge its own work;
- broaden scope silently;
- weaken tests or policy to obtain green status.

### Tester

May:

- add or improve tests inside the approved test boundary;
- reproduce failures;
- create regression coverage.

Must not:

- change production behavior merely to make a test pass unless explicitly assigned implementation
  responsibility.

### Reviewer

Default posture is independent review.

May:

- inspect code, tests, plans, diffs, and evidence;
- produce findings.

Must not:

- silently fix the implementation it is reviewing unless a separate repair task explicitly grants
  write capability.

### SecurityReviewer

May:

- inspect threat boundaries, secrets handling, dependencies, permissions, network behavior, and
  unsafe operations.

Security findings that imply privilege expansion or production risk require human review.

### Integrator

May:

- combine already-approved task outputs;
- resolve integration-only conflicts inside an explicitly granted boundary;
- run integration gates.

Must not:

- invent new feature behavior during conflict resolution.

### ReleaseManager

May:

- prepare versions, changelogs, packages, release candidates, and staging releases when granted.

Production release remains a human approval boundary by default.

## Capability model

Capabilities are granted explicitly and independently of role.

Initial capability vocabulary:

- ReadRepository
- WriteOwnedPaths
- RunLocalCommands
- UseNetwork
- ReadGitHub
- WriteGitHub
- ManageWorktrees
- ReadSecrets
- UseMcpTools
- CreatePullRequest
- MergeProtectedBranch
- DeployStaging
- DeployProduction

A role does not automatically imply every capability commonly associated with that role.

Least privilege is the default.

## Human approval boundaries

Human approval is required by default for:

- activating an implementation plan;
- expanding a task beyond its approved file/scope boundary;
- adding or materially changing third-party dependencies;
- privilege or capability elevation;
- secret access;
- destructive data migration or irreversible state change;
- merging into a protected branch;
- production deployment;
- publishing a public release;
- changing governance rules that define these approval boundaries.

Projects may add stricter boundaries.

Projects must not silently remove a repository-wide human boundary during an unrelated task.

## Task contract

P0-M002 defines a logical `AgentTask` contract with:

- contract version;
- task ID;
- milestone ID;
- primary role;
- goal;
- non-goals;
- dependency task IDs;
- allowed paths;
- forbidden paths;
- capability grants;
- required approvals;
- required gates;
- expected outputs;
- evidence requirements.

Serialization is intentionally deferred. P0-M002 defines semantics and Rust domain types only.

## Result contract

P0-M002 defines a logical `AgentResult` contract with:

- contract version;
- task ID;
- outcome;
- summary;
- changed paths;
- commit identity when applicable;
- gates run;
- tests added or changed;
- evidence produced;
- unresolved risks;
- requested escalation;
- handoff notes.

A result never grants itself new authority.

## Concurrency and ownership rules

- Two write-capable tasks must not own overlapping paths concurrently by default.
- Read-only agents may inspect the same paths concurrently.
- A task may depend on another task's accepted result.
- Integration conflict resolution is an Integrator responsibility, not an excuse for implementers
  to edit outside their ownership.
- Shared generated files require explicit ownership or serialized integration.
- Dirty working trees are never shared between agents.

## Invariants

- Models are replaceable workers.
- Roles describe responsibility, not identity.
- Capabilities are explicit and least-privilege.
- Human approval boundaries cannot be silently bypassed.
- Task scope is explicit before execution.
- Results report evidence but do not self-authorize.
- Concurrent write ownership does not overlap by default.
- Review is independent by default.
- Failures are classified before repair.
- Repair proceeds forward without destructive history rewriting.

## ADRs

- ADR-0001 — model-agnostic orchestration.
- ADR-0002 — Git worktree isolation.
- ADR-0003 — durable state outside model memory.
- ADR-0004 — plan-first workflow.
- ADR-0005 — repair-forward failure policy.
- ADR-0006 — role-based agent governance.
- ADR-0007 — capability grants are independent of roles.
- ADR-0008 — versioned structured task and result contracts.

## Public API / CLI

P0-M002 may add core Rust types under `agentforge-core`:

- `AgentRole`;
- `Capability`;
- `TaskOutcome`;
- `AgentTask`;
- `AgentResult`;
- related validation helpers.

No new user-facing `forge` subcommand is required.

No serialization format is stabilized in this milestone.

## Compatibility analysis

The governance model must remain provider-neutral.

No field or enum may require one model vendor, one cloud service, or one external agent protocol.

New enum variants may be added before 1.0.

## Dependency analysis

No external dependency is required.

P0-M002 domain types use the Rust standard library.

Serialization belongs to a later milestone.

## Expected file boundary

Plan checkpoint:

- `.plans/ACTIVE`
- `.plans/P0-M002-governance-agent-contract.plan.md`
- `docs/MILESTONES.md`
- `docs/GOVERNANCE.md`
- `docs/AGENT_ROLES.md`
- `docs/TASK_CONTRACT.md`
- `docs/APPROVAL_BOUNDARIES.md`
- `docs/adr/ADR-0006-role-based-agent-governance.md`
- `docs/adr/ADR-0007-capabilities-independent-of-roles.md`
- `docs/adr/ADR-0008-versioned-agent-contracts.md`
- `docs/adr/README.md`
- `PROJECT_STATE.md`
- `AGENT_HANDOFF.md`

Implementation:

- `crates/agentforge-core/src/lib.rs`
- `crates/agentforge-core/src/agent.rs`
- project governance documents if implementation reveals a precise clarification;
- `PROJECT_STATE.md`
- `AGENT_HANDOFF.md`.

No Cargo dependency change is expected.

## Test-first matrix

| Behavior | Expected |
| --- | --- |
| Every canonical role has stable identity | pass |
| Every initial capability has stable identity | pass |
| Task contract has explicit contract version | pass |
| Task has exactly one primary role | pass |
| Task distinguishes allowed and forbidden paths | pass |
| Task declares capabilities explicitly | pass |
| Result references the originating task | pass |
| Result cannot mutate task authority | pass by type/API boundary |
| Concurrent write ownership rule is documented | pass |
| Human approval boundaries are documented | pass |
| Reviewer defaults to independent/read-oriented behavior | pass |
| Core types require no external dependency | pass |
| Existing P0-M001 gates remain green | pass |

## Implementation sequence

1. Commit this Approved plan and governance documentation without Rust implementation.
2. Validate the exact plan-only checkpoint.
3. Classify and repair any failure before implementation.
4. Add failing/expected core tests for role and capability identity.
5. Add provider-neutral role and capability enums.
6. Add versioned `AgentTask` and `AgentResult` domain types.
7. Add lightweight constructors/validation where useful without serialization.
8. Run the full local gate.
9. Inspect the exact implementation diff.
10. Record implementation evidence.
11. Close P0-M002 only after exact implementation validation.
12. Merge with full history preserved.
13. Validate exact `main` state.

## Failure modes

- Encoding provider-specific assumptions into core governance types.
- Giving roles implicit unrestricted capabilities.
- Letting an agent grant itself capabilities.
- Allowing implementers to merge their own work by default.
- Allowing reviewers to silently rewrite reviewed code.
- Treating human approval as advisory rather than authoritative.
- Overlapping concurrent write ownership without explicit integration.
- Stabilizing a serialization format before durable-state requirements are known.
- Adding dependencies for convenience despite a standard-library solution.
- Mixing Approved plan and Rust implementation in one commit.

## Documentation impact

P0-M002 creates the normative governance documents used by later orchestration milestones.

Later milestones may extend them but must preserve the approval and least-privilege principles
unless a dedicated governance change explicitly supersedes them.

## Quality gates

- `./scripts/check-text-files`
- `cargo fmt --all --check`
- `cargo check --workspace --all-targets --locked`
- `cargo clippy --workspace --all-targets --locked -- -D warnings`
- `cargo test --workspace --locked`
- `cargo run -p xtask --locked -- validate`
- `./scripts/gate.sh full`

## Acceptance criteria

- [ ] Canonical agent roles are documented.
- [ ] Role responsibilities and prohibitions are explicit.
- [ ] Capability vocabulary is documented.
- [ ] Capabilities are independent from roles.
- [ ] Least privilege is normative.
- [ ] Human approval boundaries are explicit.
- [ ] AgentTask semantic contract is documented.
- [ ] AgentResult semantic contract is documented.
- [ ] Concurrent write ownership rule is explicit.
- [ ] Reviewer independence is explicit.
- [ ] Provider neutrality is preserved.
- [ ] Core Rust role/capability/task/result types exist.
- [ ] Core types have regression tests.
- [ ] No external dependency is added.
- [ ] Exact implementation head passes full validation.
- [ ] Closure removes `.plans/ACTIVE`.
- [ ] Final main state is clean and green.

## Completion record

Implementation commit:
CI run:
CI result:
Completed:
Notes:
