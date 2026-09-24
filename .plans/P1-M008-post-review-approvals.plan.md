# Plan: P1-M008 — Post-review approvals bound to the reviewed commit

Status: Draft
Milestone: P1-M008
Created: 2026-09-24
Owner: AgentForge project
Implementer: operator-authored (see Context)

## Goal

Make an integration approval evidence that a specific commit was reviewed. Approvals that act on
an agent's result (`merge_protected_branch`, `publish_release`, `deploy_production`) are no longer
required to launch the agent. They can be recorded only after the task is accepted, and each one
is bound to the task branch head at that moment. `forge task integrate` merges only the exact
commit the merge approval names.

## Non-goals

- No change to which approvals a task declares, or to pre-execution approvals.
- No new commands for release or deployment; those boundaries are only classified and bound.
- No multi-reviewer or quorum approvals.
- No migration of existing audit records.

## Context

Dogfooding finding 8 (P2-M033). `forge task launch` requires every declared approval before
launch, including `merge_protected_branch`. The operator therefore approves the merge before the
agent has written anything. Review still happened in practice (`accept` and `integrate` are
operator commands), but the audit record of the approval does not prove it. It also does not say
which commit was approved: a task branch could gain commits after review and still integrate.

Remote workers (Phase 4) will return results produced off-host, which makes an approval bound to
the reviewed commit a prerequisite for exact-SHA result acceptance. This milestone therefore comes
before the lease wiring.

This milestone changes approval semantics, so it is operator-authored rather than agent-built:
an agent should not write the gate that governs its own integration. Its CI and review follow the
usual flow.

## Architecture placement

- `agentforge-core`: `ApprovalBoundary::is_post_execution()` is true for `MergeProtectedBranch`,
  `PublishRelease`, and `DeployProduction`. It is a pure classification with a unit test covering
  every variant.
- `agentforge-orchestrator` (`validate_launch_policy`) and `agentforge-adapter` (preflight): only
  pre-execution approvals are required to launch or run. This covers `forge run`, `forge task
  launch`, `launch-batch`, and the daemon's `run` and `launch`, which all go through these two
  checks.
- `agentforge-operator`:
  - `approve_task` for a post-execution boundary requires the task to be `succeeded` (accepted) and
    to have a managed worktree. It records `source_head` (the task branch head) in the
    `ApprovalRecorded` event. Repeating the approval for the same head is a no-op; approving again
    after the head changed appends a new record.
  - Approving a post-execution boundary earlier fails with a message naming the order: review
    with `forge task diff`, `forge task accept`, then approve.
  - `integrate_task` requires a merge approval whose `source_head` equals the current task branch
    head. Otherwise it fails before touching Git. The message says whether no approval exists, the
    approval predates binding (a legacy record with no `source_head`), or the approval names a
    different commit (with both SHAs).
- `agentforge-worktree`: `integrate_expecting(task, target, expected_head)` re-checks the source
  head under the integration lock (the existing `StaleSource` error), so nothing can change between
  the approval check and the fast-forward. `integrate` keeps its signature.
- `forge task approve` prints the bound head (`approved merge_protected_branch for <task> at
  <sha>`). `forge task inspect <task>` lists recorded approvals with their heads.

## Invariants

- The agent never needs a post-execution approval to run.
- A post-execution approval exists only for an accepted task and names one commit.
- Integration fast-forwards exactly the approved commit or nothing.
- Audit records stay append-only; legacy approvals stay readable but cannot authorize integration.

## ADRs

ADR-0045 "Post-review approvals": the boundary classification, head binding, and legacy handling.
It amends the integration rule in ADR-0026.

## Public API / CLI

- `ApprovalBoundary::is_post_execution`, `WorktreeManager::integrate_expecting`.
- Changed behavior of `forge task approve` (ordering and printed head), `forge task inspect`
  (approval heads), and `forge task integrate` (head check).

## Compatibility analysis

- Behavior change for the operator workflow: the merge approval moves from before launch to after
  accept. Recording it before launch is now refused, with guidance.
- Tasks already accepted with a legacy (unbound) merge approval must be approved again before
  integration. No such task is pending in this repository.
- The audit format is unchanged (one extra field on `ApprovalRecorded`).

## Dependency analysis

None.

## Expected file boundary

- `.plans/P1-M008-post-review-approvals.plan.md`, `.plans/ACTIVE`
- `crates/agentforge-core/src/agent.rs`
- `crates/agentforge-orchestrator/src/lib.rs`, `crates/agentforge-orchestrator/tests/*.rs`
- `crates/agentforge-adapter/src/lib.rs`, `crates/agentforge-adapter/tests/*.rs`
- `crates/agentforge-operator/src/lib.rs`, `crates/agentforge-operator/tests/operator_actions.rs`
- `crates/agentforge-worktree/src/lib.rs`, `crates/agentforge-worktree/tests/worktree_manager.rs`
- `crates/agentforge-cli/src/main.rs`, `crates/agentforge-cli/tests/*.rs`
- `docs/adr/ADR-0045-post-review-approvals.md`, `docs/adr/README.md`
- `docs/APPROVAL_BOUNDARIES.md`, `docs/ORCHESTRATION.md`, `docs/OPERATIONS.md`,
  `docs/DOGFOODING.md`
- closure records: `docs/MILESTONES.md`, `CHANGELOG.md`, `README.md`, `PROJECT_STATE.md`,
  `AGENT_HANDOFF.md`

## Test-first matrix

- Core: `is_post_execution` classifies all 11 boundaries as expected.
- Launch: a task requiring `merge_protected_branch` plus a recorded pre-execution approval launches
  with no merge approval (orchestrator, and CLI `task launch`). A missing pre-execution approval
  still fails preflight.
- Operator:
  - approving the merge on a pending or running task fails, and nothing is appended;
  - after accept, the approval records `source_head`, and a repeat is a no-op;
  - integration succeeds for the approved head;
  - a commit added to the task branch after approval makes integration fail with both SHAs, and
    `main` is unchanged;
  - approving again binds the new head, and integration then succeeds;
  - a legacy approval without `source_head` is refused for integration.
- Worktree: `integrate_expecting` with a wrong expected head fails with `StaleSource` and leaves the
  target unchanged.
- CLI: `approve` prints the head, and `inspect` lists it.

## Implementation sequence

1. Commit this draft plan.
2. Approve and activate it in a separate checkpoint.
3. Write the tests, then core → worktree → operator → orchestrator/adapter → CLI, plus the ADR and
   docs.
4. Run the full gate, then dogfood on a scratch repo: create → launch without a merge approval →
   diff → accept → approve (head printed) → extra commit → integrate refused → approve again →
   integrate.
5. Push and verify CI (push plus a repeat); close; tag.

## Failure modes

- A launch path missed by the filter keeps requiring the merge approval: covered by the orchestrator
  and CLI launch tests and the scratch-repo dogfood.
- An operator with a pre-launch approval habit gets a refusal: the message gives the order to
  follow.

## Documentation impact

APPROVAL_BOUNDARIES (post-execution boundaries and binding), ORCHESTRATION (integration rule),
OPERATIONS (running-agents order), DOGFOODING (finding 8 resolved), and ADR-0045. README,
CHANGELOG, and MILESTONES at closure.

## Quality gates

- `./scripts/gate.sh full`;
- push-triggered CI green plus a dispatched repeat.

## Acceptance criteria

- [ ] Agents launch without post-execution approvals on every launch path.
- [ ] Post-execution approvals require acceptance and are bound to the reviewed head.
- [ ] Integration merges only the approved commit, checked under the integration lock.
- [ ] ADR-0045, docs, and finding 8 resolved; CI evidence recorded; closed and tagged.

## Completion record

Implementation commit:
CI run:
CI result:
Completed:
Notes:
