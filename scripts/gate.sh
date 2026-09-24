#!/usr/bin/env bash
set -euo pipefail

mode="${1:-full}"

# A linked worktree builds into its own target directory. Cargo names workspace-member artifacts
# independently of the checkout path, so checkouts sharing one target directory would otherwise
# test each other's binaries as "fresh" (P2-M028).
isolate_worktree_target() {
  local git_dir common_dir base
  git_dir="$(git rev-parse --path-format=absolute --git-dir)"
  common_dir="$(git rev-parse --path-format=absolute --git-common-dir)"
  if [ "$git_dir" != "$common_dir" ]; then
    base="$(cargo metadata --no-deps --offline --format-version 1 \
      | python3 -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')"
    export CARGO_TARGET_DIR="$base/agentforge-worktrees/$(basename "$git_dir")"
    echo "gate: linked worktree; CARGO_TARGET_DIR=$CARGO_TARGET_DIR"
  fi
}

# Git hooks in a linked worktree export absolute GIT_DIR and GIT_INDEX_FILE. Tests that shell out
# to git would then act on this repository instead of their own fixtures (P2-M028), so every
# GIT_* variable is cleared before the cargo steps. Plan policy above still needs them.
clear_git_environment() {
  local name
  for name in $(compgen -e | grep '^GIT_' || true); do
    unset "$name"
  done
}

run_fast() {
  isolate_worktree_target
  ./scripts/check-text-files
  cargo run -p xtask --locked -- validate
  cargo run -p xtask --locked -- validate-plan-policy
  clear_git_environment
  cargo fmt --all --check
  cargo check --workspace --all-targets --locked
  cargo clippy --workspace --all-targets --locked -- -D warnings
  cargo test --workspace --locked
}

case "$mode" in
  precommit|fast|full) run_fast ;;
  *) echo "usage: $0 {precommit|fast|full}" >&2; exit 2 ;;
esac
