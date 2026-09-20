# AgentForge Project State

## Current release

- Workspace version: `0.0.1`
- Release line: `0.0.x` alpha

## Current phase

Phase 2 — operator experience.

## Active milestone

None. P2-M005 is complete; the next increment is intentionally unplanned until real operator use
identifies the next bounded need.

## Completed milestones

- `P0-M001` — Repository bootstrap.
- `P0-M002` — Governance and agent contract.
- `P0-M003` — Plan-first workflow enforcement.
- `P0-M004` — Task graph and durable state.
- `P0-M005` — Worktree isolation manager.
- `P0-M006` — Agent adapter interface.
- `P0-M007` — Gate engine.
- `P0-M008` — CI monitor and failure classifier.
- `P0-M009` — Event and audit log.
- `P1-M001` — Multi-agent scheduling and serialized integration.
- `P1-M002` — Single-task orchestration loop.
- `P1-M003` — Project blueprint and task intake.
- `P2-M001` — Read-only operator HUD.
- `P2-M002` — Interactive operator HUD.
- `P2-M003` — Controlled operator actions.
- `P2-M004` — Release readiness.
- `P2-M005` — Local daemon and dogfooding.

## P2-M004 completion evidence

- Approved plan checkpoint: `819b2001cafca3518f5d9519d418e6f7d0280b14`.
- Plan CI: `35499050051` — all four jobs green.
- Validated implementation head: `ccad549`.
- Exact implementation CI: `35499518052` — all existing and Linux/macOS/Windows matrix jobs green.
- Manual release packaging validation: `35499534918` — all four target archives built successfully;
  publishing was skipped as expected for a manual run.
- Closure commit: `fca7b057bb7422e3bc9ab3e94621bbe797a08aa1`.
- Closure/mainline CI: `35499689853` — all jobs green for that exact closure SHA.

## P2-M005 completion evidence

- Approved plan: `.plans/P2-M005-daemon-dogfooding.plan.md`.
- Implementation commits: `98841d7860a93dbdb10294d9fcf48710b36683d4`,
  `ba713c74433abfaddf1db847a0f25f6f83201af7`, and
  `65409dc5c629d31cf691bd7a3c426793f5943821`.
- Exact implementation CI: `35503881437` and `35504237571` — all jobs green for the exact heads.
- Closure commit: `50b3b8902c3acb68ff76a12598698ff69afbf401`.
- Closure CI: `35504369065` — all jobs green for the exact closure SHA.
- Local full gate: `CARGO_TARGET_DIR=/tmp/agentforge-cargo-target ./scripts/gate.sh full` — passed.
- The bounded loopback `forged` service now supports durable status/run/stop operation, and the
  temporary-repository dogfooding path proves intake through explicit acceptance and safe cleanup.

## P0-M005 completion evidence

- Approved plan checkpoint: `5f109357f3bdcadff3328622a26e7bc69aacd10b`.
- Plan CI: `35478127108` — all four jobs green.
- CI maintenance plan amendment: `bb5791ca59b3006115f3732b02b26b035d243adb`.
- Plan-amendment CI: `35478252595` — all four jobs green.
- CI runtime maintenance: `68e985b96ad9a77195851abe9f88b8b6dd3a496f`.
- CI-maintenance run: `35478294955` — all four jobs green.
- Validated implementation head: `e302cdae8a6610be04ec244bcefa2eab7c768864`.
- Exact implementation CI: `35481590769` — all four jobs green.

- Closure commit: `259f4c06844898c7cd99aeb29b38f23f5688de3a`.
- Closure CI: `35481797256` — all four jobs green.
- Merge commit: `069c058f7d39ab39a3267f5893d20b963d4f5397`.
- Post-merge main CI: `35481855409` — all four jobs green for that exact merge commit.

P0-M005 closure and integration checks are complete.

## P0-M006 completion evidence

- Approved plan checkpoint: `ee61bd7bd6211e00613a233b111dbd960cc1f030`.
- Plan CI: `35482559468` — all four jobs green.
- Validated implementation head: `d277a9191421ae5f4018a4aaacf69f349e2b7974`.
- Exact implementation CI: `35482922234` — all four jobs green.

- Closure commit: `bfad41192a4376e1f36d7f2791ddba1e0dabeb2b`.
- Closure CI: `35483034282` — all four jobs green.
- Merge commit: `023a74969826f1de1bfb80c49e0dda125942336b`.
- Post-merge main CI: `35483102850` — all four jobs green for that exact merge commit.

P0-M006 closure and integration checks are complete.

## Current capability

AgentForge now has:

- provider-neutral governance and durable task-state foundations;
- deterministic task-owned branches and managed worktree paths;
- exact base-commit resolution;
- Git porcelain-based worktree discovery;
- task, path, and branch ownership verification;
- deterministic managed-only listing;
- dirty-state and unresolved-operation detection;
- safe non-forced worktree creation and retirement;
- preserved task branches after retirement;
- direct Git argument passing without shell interpolation;
- no routine forced removal, `git clean`, or `git reset --hard`;
- isolated temporary-repository integration coverage;
- Rust 1.85.0 compatibility without external Rust dependencies.

AgentForge can now invoke an explicitly configured local coding-agent executable in a verified task
worktree through a provider-neutral interface, either directly or through the optional bounded
loopback `forged` service. Execution evidence is bounded and remains separate from task acceptance.
It can also run direct, explicit local gates with cleared child environments,
bounded raw evidence, deadlines, and ordered batch reports. Scheduling, CI failure automation,
audit events, and deployment orchestration remain intentionally unimplemented. Operators can now
read a deterministic, bounded HUD snapshot of intake, task, audit, and managed worktree state.
Operators can also inspect tasks, record explicit required approvals, and apply audited accept,
cancel, and retry transitions through `forge task`. `forge run` consumes only verified, task-linked
approval evidence; HUD and watch mode remain read-only.
The repository now declares its MIT license and `0.0.x` versioning policy, packages tagged/manual
binary archives with checksums, and validates portable workspace behavior on Linux, macOS, and
Windows.

## P2-M003 completion evidence

- Approved plan: `.plans/P2-M003-controlled-operator-actions.plan.md`.
- Implementation head: `381afc6d0342ad12b36ce9574dd6ffb1a1c34a93`.
- Exact implementation CI: `35498108446` — all four jobs green.
- Documentation closure commit: `907913d424148506aa8f1a3704aa89b895ea7658`.
- Closure/mainline CI: `35498226882` — all four jobs green for that exact closure SHA.

## Next planned milestone

None. Future work should be scoped from observed operator usage and approved before implementation.

## Known blockers

Artifact signing and provenance attestations remain future work; they are outside the completed
daemon milestone.

## P2-M002 completion evidence

- Approved plan: `.plans/P2-M002-interactive-hud.plan.md`.
- Implementation head: `11792769304f8042235146d670eb554b0bc0cde3`.
- Exact implementation CI: `35497283972` — all four jobs green.
- The HUD now supports bounded cooked-mode watch operation with refresh/help/quit commands and
  recoverable, read-only source diagnostics.

## P2-M001 completion evidence

- Approved plan: `.plans/P2-M001-operator-hud.plan.md`.
- Implementation head: `ca741baeda2fd040e6df714f9d2c3af18d16ba76`.
- Exact implementation CI: `35496675009` — all four jobs green.
- The HUD now provides a read-only, bounded, deterministic report and fail-closed source diagnostics
  through `forge hud <root>`.

## P1-M003 completion evidence

- Approved plan: `.plans/P1-M003-project-intake.plan.md`.
- Implementation head: `987b971a05ae7ef74b4e9da8cb486b9f0b597260`.
- Exact implementation CI: `35495846139` — all four jobs green.
- The intake path now provides non-overwriting initialization, bounded versioned blueprint and
  guideline validation, explicit task creation, durable snapshot writes, and CLI integration tests.

## P1-M002 completion evidence

- Approved plan: `.plans/P1-M002-orchestration-loop.plan.md`.
- Implementation head: `0dcd50d08b8fcec9c25a9ab13f3567e2f50209cd`.
- Exact implementation CI: `35494859218` — all four jobs green.
- The persisted single-task loop validates task state and policy, executes a bounded local process
  through the adapter, records durable task/audit transitions, and exposes `forge run`.
- Successful executions remain `Running` pending independent acceptance; failed-state persistence is
  a known follow-up gap for the next implementation increment.

## P1-M001 completion evidence

- Approved plan checkpoint: `f6d0beec1bd40d14496cd97629b21d42bf1d5667`.
- Plan CI: `35493471046` — all four jobs green.
- Validated implementation head: `f71e4b15af3e5eb09c172ea11d3464d946415847`.
- Exact implementation CI: `35493600837` — all four jobs green.
- Closure commit: `4e2a0b39745fec576eb3579d5f9895fe380c1019`.
- Closure CI: `35493875738` — all four jobs green.
- Merge commit: `3805f0007618395011f3733e38c0514a9fb5ac6e`.
- Post-merge main CI: `35493925460` — all four jobs green for that exact merge commit.

P1-M001 closure and integration checks are complete.

## P0-M012 completion evidence

- Approved plan checkpoint: `59212c5610849a68ed61aad8df8fca33540ade43`.
- Plan CI: `35493056319` — all four jobs green.
- Validated implementation head: `fcf3809c9f5c099833cb07862e626ca3f1764cff`.
- Exact implementation CI: `35493160857` — all four jobs green.

## P0-M011 completion evidence

- Approved plan checkpoint: `8bf2ad634fd834219bc32bb09eb6eff58f10d5f0`.
- Plan CI: `35492671297` — all four jobs green.
- Validated implementation head: `bd1aaf24800dde73f1dc707519f5d1ed710a6b0e`.
- Exact implementation CI: `35492790507` — all four jobs green.
- Closure commit: `62911373015ed8e94d7e8883b955c078a175cf48`.
- Closure CI: `35492882542` — all four jobs green.
- Merge commit: `f35aebea3f017c3dd015dc4bb24a70ab04f1c59a`.
- Post-merge main CI: `35492920743` — all four jobs green for that exact merge commit.

P0-M011 closure and integration checks are complete.

## P0-M010 completion evidence

- Approved plan checkpoint: `e7cf33adccfc46eceb0b5c54d09a2682eeccc1bd`.
- Plan CI: `35490065662` — all four jobs green.
- Validated implementation head: `1ece087f25ccb6ce8fda96867e9f2502b91e4c86`.
- Exact implementation CI: `35490199374` — all four jobs green.

## P0-M009 completion evidence

- Approved plan checkpoint: `67a0b2abe8794e348277b1ea41a362fc578d8a2a`.
- Plan CI: `35485474803` — all four jobs green.
- Validated implementation head: `30b557369f3298dceb706935993fcb358ce62c0c`.
- Exact implementation CI: `35489676966` — all four jobs green.
- Closure commit: `931c16fede4b9a12051bf83e64a19527a688fd39`.
- Closure CI: `35489732903` — all four jobs green.
- Merge commit: `76f5cedefffabf70a699171c75e8a49d721290c4`.
- Post-merge main CI: `35489772080` — all four jobs green for that exact merge commit.

P0-M009 closure and integration checks are complete.

## P0-M008 completion evidence

- Approved plan checkpoint: `b203df0403ba6e4272770336e77f138a9c4731ee`.
- Plan CI: `35484652048` — all four jobs green.
- Validated implementation head: `6c2a057750e38fe1ec8c19c3618773283900b367`.
- Exact implementation CI: `35485116791` — all four jobs green.
- Closure commit: `5b548bb53263c8e0dc10a5c3e9ad5edbffdad068`.
- Closure CI: `35485184645` — all four jobs green.
- Merge commit: `407f42214fb9099bc947dad3727b693a046cc04d`.
- Post-merge main CI: `35485226922` — all four jobs green for that exact merge commit.

P0-M008 closure and integration checks are complete.

## P0-M007 completion evidence

- Approved plan checkpoint: `216f5269e6186274f22bfa974f54cead4326ecb5`.
- Plan CI: `35483669377` — all four jobs green.
- Validated implementation head: `bc115e2c8f12d3459287661e554f02f7adb7b3e4`.
- Exact implementation CI: `35483980498` — all four jobs green.
- Closure commit: `02242b2f3fe7eb82239e2dca43b1bd85aa239a86`.
- Closure CI: `35484071505` — all four jobs green.
- Merge commit: `741cf58caa6fca38c4693c4359816f4721a2b126`.
- Post-merge main CI: `35484138853` — all four jobs green for that exact merge commit.

P0-M007 closure and integration checks are complete.
