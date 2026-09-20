# Project blueprint and guideline intake

P1-M003 adds a small, versioned intake surface before the future TUI. The durable files are the
source of truth; the CLI and a later HUD are only interfaces over them.

## Initialize

```text
forge init /path/to/project
```

This creates `.forge/blueprint.conf` and `.forge/guidelines.md`. Initialization refuses to
overwrite either file.

## Blueprint format

`blueprint.conf` is UTF-8, line-oriented, and bounded to 64 KiB. Blank lines and `#` comments are
allowed. The first required fields are:

```text
# AgentForge Project Blueprint v1
version=1
name=Example project
mission=Build a reviewable local application.
default_milestone=P1-M003
allowed_path=src
forbidden_path=.env
capability=read_repository
capability=write_owned_paths
gate=full
```

Repeated fields are lists. Supported capability and approval names are the stable identifiers from
the policy contract. Unknown keys, duplicate scalar fields, unknown enum names, missing required
fields, unsupported versions, and oversized fields fail validation.

## Guidelines format

`guidelines.md` is UTF-8, bounded to 256 KiB, and begins with:

```text
# AgentForge Guidelines v1
```

The remaining Markdown is project guidance. It is context only: prose cannot grant capabilities,
approvals, paths, or gates.

Validate both documents without changing state:

```text
forge blueprint validate /path/to/project
```

## Create a task

Task authority is explicit. The command requires a task ID, milestone, role, and goal. Repeated
options can override structured defaults from the blueprint:

```text
forge task create /path/to/project P1-M003-T0001 P1-M003 implementer \
  "Add the intake command" \
  --allowed crates/agentforge-cli \
  --capability write_owned_paths \
  --gate full \
  --output "CLI command" \
  --evidence "tests and CI"
```

The task is validated and written through the existing versioned snapshot store. Failed validation,
duplicate IDs, missing dependencies, or a failed write leave the previous snapshot unchanged.
