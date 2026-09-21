# AgentForge Agent Handoff

## Repository state

- Active milestone: none; P2-M015 — real-project orchestration pilot is complete.
- Active plan: none (`.plans/ACTIVE` is intentionally absent after closure).
- P2-M015 implementation commit: `8992da820162e3bd410841dd8eeddd8e5f04a935`; exact CI
  `35549918749` is green across repository policy, stable, MSRV, CLI smoke, Linux, macOS, and
  Windows.
- P2-M015 closure commit: `5ce50c9`; exact closure CI `35550083171` is green across repository
  policy, stable, MSRV, CLI smoke, Linux, macOS, and Windows.
- P2-M015 plan: `.plans/P2-M015-real-project-orchestration-pilot.plan.md` — completed with a
  foreground `forge task launch` path that composes exact-base worktree preparation, readiness and
  approval preflight, bounded process execution, durable worktree observation, and explicit recovery.
- P2-M014 implementation commit: `b0cbdc810a1835c9f27d9c064cd6646334fc4897`; exact CI
  `35548080732` is green across repository policy, stable, MSRV, CLI smoke, Linux, macOS, and
  Windows.
- P2-M014 closure commit: `75031af0105cf5019e8f55751f77e375947bb443`; exact closure CI
  `35548238497` is green across repository policy, stable, MSRV, CLI smoke, Linux, macOS, and
  Windows. The initial Windows teardown failure was classified as infrastructure/flaky and the
  failed job passed on rerun.
- P2-M014 plan: `.plans/P2-M014-safe-review-integration.plan.md` — completed with bounded review
  diffs, explicit merge approval/capability checks, serialized fast-forward-only integration,
  idempotent audit evidence, and preserved branches/worktrees.
- P2-M013 implementation commit: `c9809205cf5f5851b00d957332a35f732392ab66`; exact CI
  `35545010733` is green across repository policy, stable, MSRV, CLI smoke, Linux, macOS, and
  Windows.
- P2-M013 closure commit: `5c086b1c011edc8ebfab8da646d3a6808b239e7e`; exact closure CI
  `35545183120` is green across repository policy, stable, MSRV, CLI smoke, Linux, macOS, and
  Windows.
- P2-M013 plan: `.plans/P2-M013-pty-foreground-agent-sessions.plan.md` — completed with explicit
  PTY-backed foreground sessions, raw terminal restoration, resize forwarding, bounded evidence,
  and fail-closed non-TTY behavior.
- P2-M012 implementation commit: `4fa5c0d2c00887b5de70e6bcf43ed752e8202936`; exact CI
  `35530232141` is green across repository policy, stable, MSRV, CLI smoke, Linux, macOS, and
  Windows.
- P2-M012 closure commit: `3969de7a70b638dce6d4d4eddef0052e49a27f88`; exact closure CI
  `35530354301` is green across repository policy, stable, MSRV, CLI smoke, Linux, macOS, and
  Windows.
- P2-M012 plan: `.plans/P2-M012-interactive-foreground-agent-sessions.plan.md` — completed with
  cooked line-oriented foreground interaction, bounded live evidence, and daemon compatibility.
- P2-M011 implementation commit: `7b40c0a83cf022d66462270d4d4ca6fc66c40975`; exact CI
  `35529284464` is green across repository policy, stable, MSRV, CLI smoke, Linux, macOS, and
  Windows.
- P2-M011 closure commit: `d359d53d3ab8bbcf17091be548e23409ffa971ad`; exact closure CI
  `35529438657` is green across repository policy, stable, MSRV, CLI smoke, Linux, macOS, and
  Windows.
- P2-M011 plan: `.plans/P2-M011-guided-intake-operator-hardening.plan.md` — completed with bounded
  `--input-file` replay, fail-closed input validation, and disposable intake-to-HUD dogfooding.
- P2-M010 implementation commit: `5533454d0d1fb75129b675e61569931a685b934a`; exact CI
  `35528652736` is green across repository policy, stable, MSRV, CLI smoke, Linux, macOS, and
  Windows.
- P2-M010 closure commit: `4ad32620c9a8b3c1e54e7b96a858bd0f746b5bf2`; exact closure CI
  `35528764835` is green across repository policy, stable, MSRV, CLI smoke, Linux, macOS, and
  Windows.
- P2-M010 plan: `.plans/P2-M010-guided-project-intake.plan.md` — completed with implementation and
  closure evidence.
- P2-M008 implementation commits: `f6ee2e4`, `4b2f1f4`; exact CI `35520394000` is green across
  repository policy, stable, MSRV, CLI smoke, Linux, macOS, and Windows.
- P2-M008 plan amendment: `4750814` bound the portable Rust fixture file boundary.
- P2-M008 closure commit: `d6a8e43`; replacement exact closure CI `35523593533` is green across
  repository policy, stable, MSRV, CLI smoke, Linux, macOS, and Windows.
- P2-M009 implementation commit: `d4d17cd`; exact implementation CI `35527442420` is green across
  repository policy, stable, MSRV, CLI smoke, Linux, macOS, and Windows.
- P2-M009 plan: `.plans/P2-M009-daemon-audit-sequence-continuation.plan.md` — completed with closure
  commit `59c5934` and exact closure CI `35527646923` green across all supported jobs.
- The persisted daemon path now seeds attempt audit logs from the verified sequence/digest tail;
  Omniscient dogfooding appended records `#6–#8` after prior history without changing the audit
  format.
- P2-M007 implementation commits: `0153810`, `11c6b32`, `11ea7e7`, `4c33f2f`, `2144aa7`,
  `f963e17`, and `2bd09e7`; exact CI `35518517068` is green across MSRV, stable, policy,
  CLI smoke, Linux, macOS, and Windows.
- P2-M007 closure commit: `3849747`; exact closure CI `35518635838` is green across MSRV,
  stable, policy, CLI smoke, Linux, macOS, and Windows. No implementation work remains.
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
- P2-M005 implementation commits: `98841d7860a93dbdb10294d9fcf48710b36683d4`,
  `ba713c74433abfaddf1db847a0f25f6f83201af7`, and `65409dc5c629d31cf691bd7a3c426793f5943821`;
  CI `35503881437` and `35504237571` green for the exact implementation heads.
- P2-M005 closure: `50b3b8902c3acb68ff76a12598698ff69afbf401`; CI `35504369065` green for the
  exact closure SHA.
- P2-M005 plan: `.plans/P2-M005-daemon-dogfooding.plan.md` — Completed; closure evidence is
  recorded in this final checkpoint.
- P2-M006 implementation commits: `1fd2d705d685e503d96e8f91b78d2edee777d219`,
  `541d317fbd4e6038a3bcdb36fcc507fe037250ca`, and
  `542d80a1cbf654a9b19bb01b8166c4880cf9b5f1`; exact CI `35505389584` is green across all jobs.
- P2-M006 closure: `58c12f6309acaa3510e1aab4a27d52abb8390dcf`; closure CI `35505514028` is green
  for the exact closure SHA.
- P2-M006 plan: `.plans/P2-M006-real-agent-dogfooding.plan.md` — Completed; final closure evidence
  is recorded.
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
7. If work resumes, draft and approve a new bounded plan before implementation; no milestone is
   currently active.
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
