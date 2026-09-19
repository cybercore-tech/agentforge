# AgentForge Continuous Integration

P0-M003 bootstraps GitHub Actions as AgentForge's independent remote validation authority.

## Required job model

The initial workflow has four independently visible jobs:

1. Repository policy
2. Stable code gate
3. MSRV 1.85.0
4. CLI smoke

Independent jobs make failures classifiable instead of hiding all evidence behind one monolithic
status.

## Triggers

The workflow runs on:

- pull requests;
- pushes to `main`;
- manual workflow dispatch.

Merge-queue support may be added later if the repository enables a merge queue.

## Permissions

Workflow-level GitHub token permissions default to:

```yaml
permissions:
  contents: read
```

A later job may receive broader authority only through an explicit, reviewed requirement.

## Dependency and cache policy

All Cargo operations that consume the lockfile use `--locked`.

The bootstrap workflow does not use a dependency/build cache. AgentForge is currently small enough
that cache complexity is not justified.

## Exact-head evidence

A green run is evidence only for the commit SHA that produced it.

Implementation, closure, and post-merge validation records must identify the exact head they
validated. A successful run from an older commit is not transferable evidence.

## Bootstrap exception

The P0-M003 Approved plan checkpoint predates the first workflow. It is validated by the existing
local full gate.

After the workflow is introduced, remote exact-head CI is required for the remaining P0-M003
checkpoints and future milestones.
