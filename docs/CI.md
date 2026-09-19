# AgentForge Continuous Integration

P0-M003 bootstraps GitHub Actions as AgentForge's independent remote validation authority.

## Workflow

The repository workflow is `.github/workflows/ci.yml`.

It exposes four independently visible jobs:

1. `Repository policy`
2. `Stable code gate`
3. `MSRV 1.85.0`
4. `CLI smoke`

Independent jobs keep failures classifiable instead of hiding all evidence behind one monolithic
status.

## Triggers

The workflow runs on:

- pull requests;
- pushes to `main`;
- manual workflow dispatch.

Pull-request jobs explicitly check out the pull-request head SHA so exact-head policy checks inspect
the real branch commit rather than only GitHub's synthetic merge commit.

Merge-queue support may be added later if the repository enables a merge queue.

## Repository policy

The repository-policy job validates:

- tracked text policy;
- required repository structure;
- active-plan state;
- plan-first implementation authority.

For staged local changes, implementation-sensitive paths require an Approved plan already in HEAD.

For committed branch heads, an implementation-sensitive commit requires its parent commit to
contain the Approved active plan.

Merge commits are integration boundaries; repository-state validation still runs, while
single-commit sequencing is enforced on the branch commits before merge.

## Permissions

Workflow-level GitHub token permissions are:

```yaml
permissions:
  contents: read
```

A later job may receive broader authority only through an explicit reviewed requirement.

## Dependency and cache policy

All Cargo operations that consume the lockfile use `--locked`.

The bootstrap workflow does not use a dependency/build cache. AgentForge is currently small enough
that cache complexity is not justified.

## Stable code gate

Stable CI installs current stable Rust with `rustfmt` and `clippy`, then runs:

- `cargo fmt --all --check`;
- `cargo check --workspace --all-targets --locked`;
- `cargo clippy --workspace --all-targets --locked -- -D warnings`;
- `cargo test --workspace --locked`.

## MSRV

The MSRV job installs Rust 1.85.0 and runs locked workspace check and tests independently of stable.

## CLI smoke

The smoke job verifies:

- `forge version`;
- `forge doctor`;
- `forged --version`.

## Exact-head evidence

A green run is evidence only for the commit SHA that produced it.

Implementation, closure, and post-merge validation records must identify the exact head they
validated. A successful run from an older commit is not transferable evidence.

## Bootstrap exception

The P0-M003 Approved plan checkpoint predates this workflow. It is validated by the existing local
full gate.

After this workflow is introduced, remote exact-head CI is required for the remaining P0-M003
checkpoints and future milestones.
