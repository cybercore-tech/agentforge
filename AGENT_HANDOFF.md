# AgentForge Agent Handoff

## Repository state

- Active milestone: none. P2-M033 (HUD view of agent runs) is complete: commit `29484bc` by Claude
  Code, landed through `forge task integrate`; CI `36010816304` and `36011038882` are green. Every
  completed milestone through P2-M033 is tagged. Record every required approval, including
  `merge_protected_branch`, before `forge task launch` (finding 8).
- P2-M032 (macOS daemon stop flake) is complete. Commit `e3c4bbe`, and four CI runs are green
  including macOS. **Closure ends
  with `scripts/tag-milestone <ID>` (check the dry-run subject reads "close ...") and pushing the tag**
  (AGENTS.md rule 14).
- P2-M029 is complete (commits `9e06c07`, `7e5c0c0`; CI `35971863878` and `35971879237` green).
  Agent runs print `agent-exit`, persist output to `.forge/evidence/<task>/`, and exit 1 on agent
  failure.
- P1-M007 (the first agent-built milestone, `6131508`) and P2-M027 (the bridge) are complete.
  Findings are in `docs/DOGFOODING.md`.
- Tasks meant to integrate through AgentForge need `--capability merge_protected_branch --approval
  merge_protected_branch` at creation. Never chain `worktree retire` after `integrate` with `;`.
- P2-M028 implementation commit: `2ffd59d`; CI `35969105365` (push) and `35969113717` are green.
  The gate now clears `GIT_*` before cargo and isolates linked-worktree target dirs.
- Push-triggered workflows now work on `cybercore-tech/agentforge` (first seen on `f5c7a14`).
- The operator's `.forge/` in this checkout is gitignored. It holds the `claude-code` agent profile
  and a `workspace` gate (`./scripts/gate.sh full`).
- P2-M026 — Project site updates feed — is complete.
- P2-M026 implementation commit: `adaacc6`; dispatched CI `35966436681`, `35966450804`, and
  `35966453213` are green; Pages `35966439149` deployed.
- P2-M025 — Canonical repository identity — is complete.
- P2-M025 implementation commit: `e840557`; dispatched CI `35964790564`, `35964797514`, and
  `35964804756` are green, and Pages run `35964788019` deployed. Push-triggered workflows still do not
  fire on the new repository; dispatch `AgentForge CI` (and `AgentForge Pages` for site changes)
  with `gh workflow run ... --ref main`.
- External docs now exist: Wiki page `src/orchestration/agentforge.md` and darknotes
  `Dev/Rust/AgentForge/AgentForge.md`. Keep both current with real work.
- P2-M024 — Daemon long-running requests — is complete.
- P2-M024 implementation commit: `5ef03f1`; dispatched CI `35964099187`, `35964105073`, and
  `35964110929` are green on all seven jobs. Push-triggered workflows did not fire on the new
  repository at closure time; use `gh workflow run 'AgentForge CI' --ref main` until they do.
- P1-M006 — Concurrent batch launch — is complete.
- P1-M006 implementation commit: `b54f47a`; exact CI `35962553757`, `35962575356`, `35962581675`,
  and `35962587949` are green on all seven jobs. `forge task launch-batch` runs disjoint ready tasks
  concurrently under a single state coordinator.
- P1-M005 — CI observation wiring — is complete.
- P1-M005 implementation commit: `f1edf54`; exact CI `35962029748`, `35962047802`, and
  `35962053963` are green on all seven jobs. `forge ci observe` records classified exact-SHA CI
  evidence through the reviewed `.forge/ci/provider.conf`; `scripts/ci-provider-github` is the
  reference provider.
- P1-M004 — Orchestrated gate evidence — is complete.
- P1-M004 implementation commit: `aa30746`; exact CI `35961539333`, `35961557663`, and
  `35961562338` are green on all seven jobs. Gates in `.forge/gates/` now run automatically after
  a successful agent in `forge run`, `forge task launch`, and the daemon paths, and a failing gate
  fails the task.
- P2-M023 — Bounded daemon lifecycle tests and CI job timeouts — is complete.
- Active plan: none (`.plans/ACTIVE` is intentionally absent between milestones).
- P2-M023 implementation commits: `1eb3b0f`, `4f5a45e`, `29bc804`; five exact CI runs on
  `29bc804` (`35961283620` push plus four dispatched repeats) are green on all seven jobs.
  Repaired: the Windows six-hour hang (500 ms readiness poll then an unbounded join), a stalled
  client wedging the daemon, a stop/restart lock race, macOS fixture-root collisions, and Windows
  delete-pending `Access is denied` during teardown. Every CI job now has `timeout-minutes`.
- P4-M003 implementation commit: `ba06cc0c77c187c4ca63f8735f3e507603e4d6af`; local
  `./scripts/gate.sh full` passed for implementation and closure checkpoints.
- P4-M003 adds `agentforge-scheduler::plan_remote_dispatch` with canonical task/worker ordering,
  readiness/path checks, explicit expiry, lease-generation evidence, worker-capacity enforcement,
  and all-or-nothing lease-book mutation. Eight focused scheduler tests pass. No network,
  process, daemon, CLI, authentication, persistence, cloud mutation, or remote execution authority
  was added; no remote CI run was requested.
- P4-M002 implementation commit: `82626529ff5cdb6ae0d63aeafa43c2fa0490a143`; local
  `./scripts/gate.sh full` passed for implementation and closure checkpoints.
- P4-M002 adds validated lease restoration and deterministic lease iteration in core plus a separate
  bounded, checksummed, atomically replaced `.forge/state/remote-leases.snapshot` through
  `agentforge-state::FileLeaseStore`. Recovery is explicitly `load -> expire_due(observed_at_ms) ->
  continue`; no clock is read during load, and task state remains independent.
- P4-M002 corruption, bounds, semantic-conflict, and restart-recovery tests pass. No network,
  daemon, CLI, authentication, cloud mutation, or remote execution authority was added; no remote
  CI run was requested.
- P4-M001 implementation commit: `2027608`; local `./scripts/gate.sh full` passed for the
  implementation and closure checkpoints, including workspace Clippy, tests, and documentation
  tests. The focused `agentforge-core` suite passed 29 tests.
- P4-M001 adds validated worker descriptors, deterministic capabilities, and an in-memory lease
  state machine. It intentionally adds no network, persistence, cloud mutation, remote command
  execution, or authentication authority.
- P3-M001 implementation commit: `07eafdf5851b32b92a392d7ab9f2465f71f5e860` in the standalone
  `cybercore-mission-control` repository; remote `main` resolves to the exact SHA.
- P3-M001 validation passed with Cloudflare Workers Types `5.20260921.1`, Wrangler `4.135.0`
  dry-run, local D1 migration, runtime/API smoke, durable audit evidence, and duplicate-nonce
  rejection. No production Cloudflare deployment or credential was used.
- P3-M002 implementation commit: `5da61c73e729d245edc356047353c2bd82842fcc` in the standalone
  `cybercore-mission-control` repository; exact GitHub Actions run `35599020205` is green for
  Worker TypeScript and Rust connector jobs.
- P3-M002 passed locked Rust formatting, check, seven connector tests, and Clippy gates. The
  connector is outbound-only, observation-only, credential-redacting, and has no command channel;
  no production deployment or real operator credential was used.
- P3-M003 implementation commit: `781590ce3769aacda21d95db267ca74d91022dfe` in the standalone
  `cybercore-mission-control` repository; exact CI `35601172176` is green across Ubuntu, macOS,
  Windows, and Worker TypeScript.
- P3-M003 adds version/build provenance, cooperative Ctrl-C shutdown, MIT licensing, inspected
  package metadata, platform CI, and a tag-gated checksummed release workflow. GitHub accepted the
  workflow definition; no release tag, production deployment, crate publication, or real credential
  was used.
- P3-M004 implementation sequence: `48863e8`, `d2029f1`, `c5d92b4`, `9533c51`, `e9b76c3`, and
  `4a435c1c6ac90f78986aa39c04235dae6552d8aa` in the standalone `cybercore-mission-control`
  repository; exact CI runs `35608217415`, `35611397659`, `35612605311`, and `35614113529` are
  green for their corresponding heads, with the final run green across Worker TypeScript and the
  Ubuntu/macOS/Windows Rust connector matrix.
- P3-M004 covers fail-closed environment preflight, disposable local migration/API/live-event smoke,
  protocol and negative-path tests, explicit staging/production boundaries, a staging-only manual
  dry-run workflow, and deployment/recovery/security documentation. No Cloudflare deployment,
  production credential, real account ID, or remote command authority was used.
- AgentForge closure commit `571c8d26724b479ca2e9e0019ca4a31b240d8744` has exact CI
  `35621709316` and Pages deployment `35621709313` green.
- P2-M022 prepared the `agentforge-platform` package without changing the private registry
  boundary. Metadata, license inclusion, explicit registry version requirements, and the opt-in
  offline archive preflight are now in place.
- P2-M022 plan-boundary amendment: `00e7b5c`.
- P2-M022 implementation commit: `5027f3b7d289d4efb07bbd6d768062ea82632147`; exact CI
  `35569143760` is green across repository policy, stable, MSRV, CLI smoke, Linux, macOS, and
  Windows. The package preflight produced and inspected a 16-file archive; normal offline registry
  resolution still fails closed because internal crates remain unpublished.
- P2-M021 isolated the concurrent Windows daemon lifecycle test race without weakening lifecycle
  assertions. The foreground daemon tests now serialize only their listener ownership inside the
  test binary; unrelated tests remain parallel.
- P2-M021 implementation commit: `97401eb712923acf08c88b0c09a3cc830daad928`; exact CI
  `35567104109` is green across repository policy, stable, MSRV, CLI smoke, Linux, macOS, and
  Windows. Twenty repeated local parallel daemon-test runs and the full local gate passed.
- P2-M020 selected `agentforge-platform` as the future end-user Cargo package identity while
  preserving the `forge`/`forged` binaries, internal crates, and GitHub release artifacts.
- P2-M020 implementation commit: `02cf453ab778c4c2aa9e44d0c83d9378b8ac142e`; exact CI
  `35562749505` is green across all seven jobs after the initial macOS temporary-root collision.
- P2-M020 closure commit: `6ff27045a32c22d8910f48cd07ef70e086f50f94`; closure CI `35565106069`
  is green on six jobs. Windows reproduced the known daemon teardown `PermissionDenied (Access is
  denied.)` infrastructure failure; no package-identity regression was observed.
- P2-M019 implementation commit: `caa37debdd89ea6035723749b52db7a02fba081d`; exact CI
  `35559168897` is green across repository policy, stable, MSRV, CLI smoke, Ubuntu, macOS, and
  Windows.
- P2-M019 closure commit: `b4d7f7c90b72154b81306f289b872e2d8f1cf248`; exact CI
  `35559471908` attempt 2 is green across all seven jobs after the initial Windows daemon teardown
  infrastructure failure passed on the failed-job rerun.
- P2-M019 keeps all workspace crates private, documents the occupied crates.io names, and preserves
  GitHub binary distribution; no package publication, rename, yank, or owner-transfer request was
  made.
- P2-M018 implementation commit: `d6856e2`; exact CI `35555122531` is green across repository
  policy, stable, MSRV, CLI smoke, Linux, macOS, and Windows after the failed-job rerun.
- P2-M018 Pages deployment: exact workflow `35555122547` is green for the implementation SHA.
- P2-M018 closure commit: `cc2a6b2`; exact closure CI `35555830628` and Pages deployment
  `35555830619` are green for that exact closure SHA.
- The README now carries current alpha status, explicit safety/authority limitations, a safe
  first-run checklist, and detailed failure inspection guidance. The public Pages CTA opens a
  bounded local README dialog and the GitHub About homepage points to the live site.
- P2-M016 implementation commits: `b8eaff7`, `e92d9e7` (portable Windows protocol-fixture repair);
  exact CI `35551842104` is green across repository policy, stable, MSRV, CLI smoke, Linux, macOS,
  and Windows after the macOS temporary-root collision passed on the failed-job rerun.
- P2-M016 closure commit: `63513a6`; exact closure CI `35552143595` is green across repository
  policy, stable, MSRV, CLI smoke, Linux, macOS, and Windows.
- P2-M016 plan: `.plans/P2-M016-daemon-task-launch-parity.plan.md` — completed with
  `forge daemon launch` for direct executables and named profiles. It reuses the foreground launch
  preparation seam, keeps `daemon run` compatibility, and preserves explicit acceptance, review,
  integration, and retirement boundaries.
- P2-M017 implementation commit: `cae1563`; exact CI `35553651412` is green across repository
  policy, stable, MSRV, CLI smoke, Linux, macOS, and Windows after the Windows daemon teardown
  test passed on the failed-job rerun.
- P2-M017 Pages deployment: exact workflow `35553651453` is green for the same implementation SHA;
  the public site is live at `https://darkstardevx.github.io/agentforge/`.
- P2-M017 closure commit: `96e02c3`; exact closure CI `35553972640` and Pages deployment
  `35553972643` are green for that exact closure SHA.
- P2-M017 Pages workflow repair: `7d2b45b`; exact CI `35554310599` and Pages deployment
  `35554310590` are green, and the deployment environment URL is now a valid GitHub expression.
- P2-M017 Pages workflow repair closure: `24d1571`; exact CI `35554451233` and Pages deployment
  `35554451229` are green for the exact closure SHA.
- P2-M017 plan: `.plans/P2-M017-agentforge-github-pages.plan.md` — completed with the static
  forge-rail landing page, bounded Pages workflow, ADR, README discovery link, and preserved mockup.
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
