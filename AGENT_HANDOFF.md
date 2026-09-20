# AgentForge Agent Handoff

## Repository state

- Active milestone: P2-M005 — Local daemon and dogfooding.
- Active plan: `.plans/P2-M005-daemon-dogfooding.plan.md` — Approved.
- P0-M001 status: Complete.
- P0-M002 status: Complete.
- P0-M003 status: Complete.
- P0-M004 status: Complete.
- P0-M005 status: Complete.
- P0-M005 validated implementation head: `e302cdae8a6610be04ec244bcefa2eab7c768864`.
- P0-M005 implementation CI: `35481590769` — all four jobs green.
- P0-M005 closure: `259f4c06844898c7cd99aeb29b38f23f5688de3a`; CI `35481797256` green.
- P0-M005 main merge: `069c058f7d39ab39a3267f5893d20b963d4f5397`; CI `35481855409` green.
- P0-M006 implementation head: `d277a9191421ae5f4018a4aaacf69f349e2b7974`.
- P0-M006 implementation CI: `35482922234` — all four jobs green.
- P0-M006 closure: `bfad41192a4376e1f36d7f2791ddba1e0dabeb2b`; CI `35483034282` green.
- P0-M006 main merge: `023a74969826f1de1bfb80c49e0dda125942336b`; CI `35483102850` green.
- P0-M007 implementation head: `bc115e2c8f12d3459287661e554f02f7adb7b3e4`.
- P0-M007 implementation CI: `35483980498` — all four jobs green.
- P0-M007 closure: `02242b2f3fe7eb82239e2dca43b1bd85aa239a86`; CI `35484071505` green.
- P0-M007 main merge: `741cf58caa6fca38c4693c4359816f4721a2b126`; CI `35484138853` green.
- P0-M008 implementation head: `6c2a057750e38fe1ec8c19c3618773283900b367`.
- P0-M008 implementation CI: `35485116791` — all four jobs green.
- P0-M008 closure: `5b548bb53263c8e0dc10a5c3e9ad5edbffdad068`; CI `35485184645` green.
- P0-M008 main merge: `407f42214fb9099bc947dad3727b693a046cc04d`; CI `35485226922` green.
- P0-M009 closure: `931c16fede4b9a12051bf83e64a19527a688fd39`; CI `35489732903` green.
- P0-M009 main merge: `76f5cedefffabf70a699171c75e8a49d721290c4`; CI `35489772080` green.
- Current planning branch: `feat/p0-m010-capability-policy`.
- P0-M009 implementation head: `30b557369f3298dceb706935993fcb358ce62c0c`.
- P0-M009 implementation CI: `35489676966` — all four jobs green.
- Current closure branch: `feat/p0-m009-event-audit-log`.
- P0-M009 plan: `.plans/P0-M009-event-audit-log.plan.md` — Completed.
- P0-M010 plan: `.plans/P0-M010-capability-policy.plan.md` — Completed; merged and post-merge CI green.
- P0-M011 plan: `.plans/P0-M011-doctor-status.plan.md` — Completed; merged and post-merge CI green.
- P0-M012 plan: `.plans/P0-M012-single-agent-vertical-slice.plan.md` — Completed; merged and post-merge CI green.
- P1-M001 plan: `.plans/P1-M001-scheduling-integration.plan.md` — Completed; merged and post-merge CI green.
- P1-M002 plan: `.plans/P1-M002-orchestration-loop.plan.md` — Completed; implementation and exact-head CI are green.
- P1-M003 plan: `.plans/P1-M003-project-intake.plan.md` — Completed; implementation and exact-head CI are green.
- P2-M001 plan: `.plans/P2-M001-operator-hud.plan.md` — Completed; implementation and exact-head CI are green.
- P2-M002 plan: `.plans/P2-M002-interactive-hud.plan.md` — Completed; implementation and exact-head CI are green.
- P2-M003 plan: `.plans/P2-M003-controlled-operator-actions.plan.md` — Completed; implementation and exact-head CI are green.
- P1-M001 implementation: `f71e4b15af3e5eb09c172ea11d3464d946415847`; CI `35493600837` green.
- P1-M002 implementation: `0dcd50d08b8fcec9c25a9ab13f3567e2f50209cd`; CI `35494859218` green.
- P1-M003 implementation: `987b971a05ae7ef74b4e9da8cb486b9f0b597260`; CI `35495846139` green.
- P2-M001 implementation: `ca741baeda2fd040e6df714f9d2c3af18d16ba76`; CI `35496675009` green.
- P2-M002 implementation: `11792769304f8042235146d670eb554b0bc0cde3`; CI `35497283972` green.
- P2-M003 implementation: `381afc6d0342ad12b36ce9574dd6ffb1a1c34a93`; CI `35498108446` green.
- P2-M003 closure: `907913d424148506aa8f1a3704aa89b895ea7658`; CI `35498226882` green.
- P2-M004 plan: `.plans/P2-M004-release-readiness.plan.md` — Completed; implementation and exact-head
  CI are green.
- P2-M004 implementation: `ccad549`; CI `35499518052` green, including Linux/macOS/Windows matrix.
- P2-M004 release packaging validation: `35499534918` green for all four target archives; manual
  publishing correctly skipped.
- P2-M004 closure: `fca7b057bb7422e3bc9ab3e94621bbe797a08aa1`; CI `35499689853` green for the
  exact closure SHA.
- P2-M005 plan: `.plans/P2-M005-daemon-dogfooding.plan.md` — Approved; implementation begins after
  the activation checkpoint CI is green.
- P0-M012 implementation: `fcf3809c9f5c099833cb07862e626ca3f1764cff`; CI `35493160857` green.
- P0-M011 implementation: `bd1aaf24800dde73f1dc707519f5d1ed710a6b0e`; CI `35492790507` green.
- P0-M011 closure: `62911373015ed8e94d7e8883b955c078a175cf48`; CI `35492882542` green.
- P0-M011 main merge: `f35aebea3f017c3dd015dc4bb24a70ab04f1c59a`; CI `35492920743` green.
- P0-M010 implementation: `1ece087f25ccb6ce8fda96867e9f2502b91e4c86`; CI `35490199374` green.

## Resume checklist

1. Read `PROJECT_SPEC.md`.
2. Read `PROJECT_STATE.md`.
3. Read `AGENTS.md`.
4. Read `docs/WORKTREE_ISOLATION.md`.
5. Read ADR-0002 and ADR-0010.
6. Run `./scripts/project-status`.
7. Implement P2-M005 only within the approved local daemon and dogfooding plan boundary after its
   activation checkpoint CI is green.
8. Never bypass repository hooks or gates.
9. Classify failures before repair.

## Completed P0-M005 work

P0-M005 established the task-owned Git worktree lifecycle:

- deterministic `agentforge/task/<task-id>` branches;
- deterministic `.forge/worktrees/<task-id>` paths;
- exact base commit resolution;
- authoritative `git worktree list --porcelain -z` inspection;
- task, path, and branch ownership checks;
- managed-only deterministic listing;
- tracked and untracked dirty-state detection;
- unresolved merge, rebase, cherry-pick, and revert detection;
- conservative non-forced retirement;
- task-branch preservation;
- no shell interpolation or routine destructive cleanup;
- isolated concurrent integration fixtures;
- ambient `GIT_INDEX_FILE` isolation for Git subprocesses.

Exact implementation CI run `35481590769` passed all four jobs for
`e302cdae8a6610be04ec244bcefa2eab7c768864`.

## P0-M007 work

P0-M007 added a standard-library-only gate engine with direct explicit executable/argument
configuration, cleared child environments, bounded concurrent raw output capture, timeout and
output-limit termination, ordered batch reports, and duplicate-name rejection. Its implementation,
closure, and post-merge CI are green. P0-M008 will add exact-SHA CI observation and conservative
failure classification before later audit and policy milestones.

P0-M008 added a standard-library-only exact-SHA CI observer with bounded direct-command execution,
strict versioned protocol decoding, stale/ambiguous evidence rejection, and conservative taxonomy
classification. Its implementation CI is green; closure, merge, and post-merge evidence remain.
