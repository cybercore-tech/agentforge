# Plan: P2-M008 — Windows dogfooding parity

Status: Completed
Milestone: P2-M008
Created: 2026-09-20

## Goal

Complete the remaining cross-platform first-use path by running managed worktree lifecycle and
daemon-backed CLI dogfooding tests on Windows CI. The milestone removes test-only platform gaps
without weakening ownership checks, path normalization, approval boundaries, or conservative
cleanup.

## Why now

P2-M007 added Windows-native Git path handling and cross-platform daemon supervision, but the
portable Windows test command still excludes `agentforge-worktree`, and the daemon CLI integration
fixture is gated to Unix because it uses a shell executable. Real Windows support needs evidence
from the same worktree-backed workflow operators use, not only compilation and CLI version smoke.

## Scope

1. **Windows worktree lifecycle evidence** — make the managed worktree integration fixture safe
   under Windows temporary paths and run it in the Windows platform matrix.
2. **Cross-platform daemon CLI fixture** — replace the Unix-only shell fixture with a deterministic
   executable fixture that can write the expected bounded output on Windows without shell
   interpolation or ambient environment assumptions.
3. **CI parity and documentation** — remove only the test exclusions justified by the repaired
   fixtures, document the platform coverage, and preserve exact-head validation evidence.

## Non-goals

- No new runtime dependencies, provider SDKs, shell execution, or network services.
- No changes to task acceptance, capabilities, approvals, audit formats, or daemon authority.
- No forced worktree removal, `git clean`, `git reset --hard`, branch deletion, or automatic
  cleanup of unrelated worktrees.
- No broad Windows path redesign beyond the fixture and narrowly required cross-platform behavior.
- No release signing, provenance attestations, or remote worker support.

## Architecture placement

- `agentforge-worktree` remains authoritative for Git discovery, path equivalence, ownership, dirty
  state, and non-forced retirement.
- Integration fixtures invoke Git and child processes with direct argument vectors and explicit
  paths; they must not depend on shell syntax or ambient `GIT_*` state.
- The existing daemon and adapter contracts remain unchanged unless a compatibility-preserving
  test-only abstraction is required to express one portable fixture.
- CI remains an exact-head evidence boundary; exclusions are removed only after the corresponding
  tests pass on Windows.

## Expected operator/evidence surface

- `cargo test -p agentforge-worktree --test worktree_manager` runs on Windows CI.
- The daemon CLI integration test exercises init, task creation, worktree creation, daemon run,
  explicit acceptance, status/stop, and safe retirement on Windows.
- Platform documentation identifies which tests are portable and which remain intentionally Unix-
  specific, with the reason stated explicitly.

## Expected file boundary

- `.plans/P2-M008-windows-dogfooding-parity.plan.md`, `.plans/ACTIVE`
- `PROJECT_STATE.md`, `AGENT_HANDOFF.md`, `docs/MILESTONES.md`, `README.md`
- `.github/workflows/ci.yml`, `docs/CI.md`, `docs/WORKTREE_ISOLATION.md`, `docs/DAEMON.md`
- `crates/agentforge-worktree/tests/worktree_manager.rs`
- `crates/agentforge-cli/tests/daemon_commands.rs`
- `crates/agentforge-cli/src/bin/agentforge-cli-fixture.rs`
- `crates/agentforge-worktree/src/lib.rs` only if a narrowly evidenced production path repair is
  required; no unrelated implementation changes
- `Cargo.lock` only if an existing workspace change requires regeneration; no new dependencies

## Test-first matrix

| Area | Required evidence |
| --- | --- |
| Worktree lifecycle | create, inspect, list, dirty refusal, retire, and branch preservation on Windows |
| Path handling | native Git arguments and canonical/verbatim path equivalence on Windows |
| Daemon CLI dogfooding | portable fixture executes, persists evidence, accepts explicitly, stops, and retires safely |
| Safety regressions | unrelated worktree isolation, no shell interpolation, no forced cleanup |
| Compatibility | Linux and macOS worktree/daemon tests remain green |
| Quality | formatting, Clippy, full gate, and exact-head CI matrix |

## Implementation sequence

1. Commit this draft as a plan-only checkpoint; approval and activation remain separate.
2. Reproduce the current Windows exclusions and identify the smallest fixture portability changes.
3. Repair the worktree and daemon CLI fixtures without weakening assertions.
4. Remove only the corresponding CI exclusions and update platform documentation.
5. Run the full local gate and exact-head CI on Linux, macOS, and Windows.
6. Close the milestone with separate documentation and exact-SHA post-closure evidence.

## Quality gates

- `CARGO_TARGET_DIR=/tmp/agentforge-cargo-target ./scripts/gate.sh full`
- Exact-head GitHub CI for implementation and closure commits.
- Windows, macOS, and Linux portable tests must remain green.
- No hooks or validation may be bypassed.

## Acceptance criteria

- [x] Windows CI runs and passes the managed worktree lifecycle integration suite.
- [x] Windows CI runs and passes daemon-backed CLI dogfooding with a non-shell fixture.
- [x] Existing ownership, approval, audit, path, dirty-state, and non-forced-retirement guarantees
      remain asserted and green on all supported platforms.
- [x] CI and documentation no longer claim parity while silently excluding these workflows.
- [x] Full local gate and exact-head CI pass for implementation and closure checkpoints.

## Completion record

Implementation commits: `f6ee2e4`, `4b2f1f4`.
Implementation CI: `35520394000` — all jobs green for exact head `4b2f1f4`.
Closure commit: pending.
Closure CI: pending.
