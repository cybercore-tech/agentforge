#!/usr/bin/env bash
set -euo pipefail

mode="${1:-full}"

run_fast() {
  ./scripts/check-text-files
  cargo fmt --all --check
  cargo check --workspace --all-targets --locked
  cargo clippy --workspace --all-targets --locked -- -D warnings
  cargo test --workspace --locked
  cargo run -p xtask --locked -- validate
}

case "$mode" in
  precommit|fast|full) run_fast ;;
  *) echo "usage: $0 {precommit|fast|full}" >&2; exit 2 ;;
esac
