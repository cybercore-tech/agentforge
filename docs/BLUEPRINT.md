# Project blueprint and guideline intake

P1-M003 adds a small, versioned intake surface before the future TUI. The durable files are the
source of truth; the CLI and a later HUD are only interfaces over them.

P2-M010 adds a guided, line-oriented authoring path for operators who do not want to hand-edit
these files. It is an explicit mutation command, separate from the read-only HUD, and it uses the
same parser, task contract, and snapshot boundaries as the lower-level commands.

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

Blueprint default gates (`gate=`) are copied into a task only when the task names no gates and a
matching `.forge/gates/<name>.conf` profile exists in the project. Unconfigured defaults are
dropped silently, so the starter blueprint's `gate=full` adds nothing until a `full` profile is
reviewed in. Explicit gates, from `--gate` or the guided task prompt, are kept as given; a launch
fails preflight if one of them has no profile (see [GATES.md](GATES.md)).

The task is validated and written through the existing versioned snapshot store. Failed validation,
duplicate IDs, missing dependencies, or a failed write leave the previous snapshot unchanged.

## Guided intake

Use the guided command to create or revise both intake documents from a terminal:

```text
forge intake /path/to/project
```

The prompts run in a fixed order: project name, mission, default milestone, allowed paths,
forbidden paths, capabilities, approvals, quality gates, and a multi-line guideline body. Enter a
comma-separated list for repeated fields, press Enter to keep the displayed value, use `-` to
clear a list, and finish guideline editing with a line containing only `.`. The command prints a
complete bounded preview and requires `y`/`yes` confirmation before writing anything.

To collect and persist an explicit task draft in the same preview/confirmation flow, add
`--task`:

```text
forge intake /path/to/project --task
```

Task prompts use the existing role, milestone, goal, dependency, path, capability, approval, gate,
output, and evidence names. Blank authority lists intentionally inherit structured blueprint
defaults (default gates only when their profile exists, as for `forge task create`); guideline
prose never grants authority. Task creation remains distinct from task
approval, acceptance, cancellation, and execution.

Input is ordinary line-oriented UTF-8 stdin, so scripted invocations and redirected input behave
the same on supported platforms. EOF or a declined confirmation reports cancellation and leaves all
documents and task state unchanged. Before commit, AgentForge compares the files shown in the
preview with their original bytes and fails closed if another process edited them. Writes use
same-directory temporary files and rename, and existing explicit commands remain available for
automation that does not need prompts.

For a reviewed, repeatable session, pass a direct UTF-8 input file:

```text
forge intake /path/to/project --input-file /path/to/session.txt
forge intake /path/to/project --task --input-file /path/to/task-session.txt
```

The file is bounded to 512 KiB, must be a regular valid-UTF-8 file, and is never copied into the
project. It feeds the same prompts as stdin, including the final confirmation, so replay cannot
skip the preview or grant additional authority.
