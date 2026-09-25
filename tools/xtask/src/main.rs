//! Repository automation for AgentForge.

use std::fs;
use std::path::Path;
use std::process::{Command, ExitCode};

const REQUIRED_PATHS: &[&str] = &[
    "PROJECT_SPEC.md",
    "AGENTS.md",
    "PROJECT_STATE.md",
    "AGENT_HANDOFF.md",
    "CONTRIBUTING.md",
    "docs/MILESTONES.md",
    "docs/adr/README.md",
    "docs/CI.md",
    ".plans/README.md",
    ".plans/TEMPLATE.plan.md",
    ".github/workflows/ci.yml",
    ".githooks/pre-commit",
    "scripts/gate.sh",
    "scripts/check-text-files",
    "scripts/project-status",
];

fn main() -> ExitCode {
    match std::env::args().nth(1).as_deref() {
        Some("validate") => report("repository validation", validate_repository()),
        Some("validate-plan-policy") => report("plan policy", validate_plan_policy()),
        _ => {
            eprintln!("usage: cargo run -p xtask -- <validate|validate-plan-policy>");
            ExitCode::from(2)
        }
    }
}

fn report(label: &str, result: Result<(), Vec<String>>) -> ExitCode {
    match result {
        Ok(()) => {
            println!("AgentForge {label}: ok");
            ExitCode::SUCCESS
        }
        Err(errors) => {
            for error in errors {
                eprintln!("{error}");
            }
            ExitCode::FAILURE
        }
    }
}

fn validate_repository() -> Result<(), Vec<String>> {
    let mut errors = Vec::new();

    validate_required_paths(&mut errors);
    validate_active_plan(&mut errors);
    validate_docs_indexes(&mut errors);
    validate_release_preflight(&mut errors);

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// Every ADR file has a registry row and every row has a file; every top-level doc is linked from
/// the README's documentation map (P2-M034).
fn validate_docs_indexes(errors: &mut Vec<String>) {
    let adr_ids = match markdown_names("docs/adr") {
        Ok(names) => names
            .iter()
            .filter(|name| name.starts_with("ADR-") && name.len() >= 8)
            .map(|name| name[..8].to_owned())
            .collect::<Vec<_>>(),
        Err(error) => {
            errors.push(format!("unable to list docs/adr: {error}"));
            return;
        }
    };
    match fs::read_to_string("docs/adr/README.md") {
        Ok(registry) => errors.extend(adr_registry_errors(&adr_ids, &registry)),
        Err(error) => errors.push(format!("unable to read docs/adr/README.md: {error}")),
    }
    let docs = match markdown_names("docs") {
        Ok(names) => names,
        Err(error) => {
            errors.push(format!("unable to list docs: {error}"));
            return;
        }
    };
    match fs::read_to_string("README.md") {
        Ok(readme) => errors.extend(unlinked_docs(&docs, &readme)),
        Err(error) => errors.push(format!("unable to read README.md: {error}")),
    }
}

/// Every internal (`path`) dependency of the CLI package has a matching `[patch.crates-io]` entry
/// in `.cargo/registry-preflight.toml`, so `scripts/package-preflight` can package it offline
/// (P5-M005; P3-M005 added a crate without one and only the release found it).
fn validate_release_preflight(errors: &mut Vec<String>) {
    match (
        fs::read_to_string("crates/agentforge-cli/Cargo.toml"),
        fs::read_to_string(".cargo/registry-preflight.toml"),
    ) {
        (Ok(manifest), Ok(preflight)) => {
            errors.extend(preflight_patch_errors(&manifest, &preflight));
        }
        (Err(error), _) => errors.push(format!(
            "unable to read crates/agentforge-cli/Cargo.toml: {error}"
        )),
        (_, Err(error)) => errors.push(format!(
            "unable to read .cargo/registry-preflight.toml: {error}"
        )),
    }
}

/// `name = { path = "<path>" ... }` lines of a TOML document, as (name, path).
fn path_entries(document: &str) -> Vec<(String, String)> {
    document
        .lines()
        .filter_map(|line| {
            let (name, rest) = line.split_once('=')?;
            let path = rest.split("path = \"").nth(1)?.split('"').next()?;
            Some((name.trim().to_owned(), path.to_owned()))
        })
        .collect()
}

/// Compares the CLI manifest's path dependencies (relative to `crates/agentforge-cli`) with the
/// preflight patch list (relative to the repository root).
fn preflight_patch_errors(cli_manifest: &str, preflight: &str) -> Vec<String> {
    let patches = path_entries(preflight);
    let mut errors = Vec::new();
    for (name, path) in path_entries(cli_manifest) {
        let expected = match path.strip_prefix("../") {
            Some(sibling) => format!("crates/{sibling}"),
            None => format!("crates/agentforge-cli/{path}"),
        };
        match patches.iter().find(|(patched, _)| *patched == name) {
            None => errors.push(format!(
                ".cargo/registry-preflight.toml: {name} (a path dependency of agentforge-cli) has \
                 no [patch.crates-io] entry; add `{name} = {{ path = \"{expected}\" }}`"
            )),
            Some((_, patched)) if *patched != expected => errors.push(format!(
                ".cargo/registry-preflight.toml: {name} is patched to {patched}, but \
                 agentforge-cli uses {expected}"
            )),
            Some(_) => {}
        }
    }
    errors
}

/// File names ending in `.md` directly inside `directory`, sorted.
fn markdown_names(directory: &str) -> std::io::Result<Vec<String>> {
    let mut names = Vec::new();
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        if !entry.file_type()?.is_file() {
            continue;
        }
        if let Some(name) = entry.file_name().to_str() {
            if name.ends_with(".md") {
                names.push(name.to_owned());
            }
        }
    }
    names.sort();
    Ok(names)
}

/// Compares ADR IDs from files with the `| ADR-NNNN |` rows of the registry.
fn adr_registry_errors(file_ids: &[String], registry: &str) -> Vec<String> {
    let rows = registry
        .lines()
        .filter_map(|line| line.strip_prefix("| "))
        .filter(|rest| rest.starts_with("ADR-") && rest.len() >= 8)
        .map(|rest| rest[..8].to_owned())
        .collect::<Vec<_>>();
    let mut errors = Vec::new();
    for id in file_ids {
        if !rows.contains(id) {
            errors.push(format!(
                "docs/adr/README.md: {id} has a file but no registry row"
            ));
        }
    }
    for (index, id) in rows.iter().enumerate() {
        if !file_ids.contains(id) {
            errors.push(format!(
                "docs/adr/README.md: registry row {id} has no ADR file"
            ));
        }
        if rows[..index].contains(id) {
            errors.push(format!("docs/adr/README.md: {id} is listed more than once"));
        }
    }
    errors
}

/// Top-level docs not linked as `(docs/<name>)` from the README.
fn unlinked_docs(doc_names: &[String], readme: &str) -> Vec<String> {
    doc_names
        .iter()
        .filter(|name| !readme.contains(&format!("(docs/{name})")))
        .map(|name| format!("README.md: docs/{name} is not linked from the documentation map"))
        .collect()
}

fn validate_required_paths(errors: &mut Vec<String>) {
    for path in REQUIRED_PATHS {
        if !Path::new(path).exists() {
            errors.push(format!("missing required path: {path}"));
        }
    }
}

fn validate_active_plan(errors: &mut Vec<String>) {
    let active_path = Path::new(".plans/ACTIVE");

    if !active_path.exists() {
        return;
    }

    let contents = match fs::read_to_string(active_path) {
        Ok(contents) => contents,
        Err(error) => {
            errors.push(format!("unable to read .plans/ACTIVE: {error}"));
            return;
        }
    };

    let plan_path = contents.trim();

    if plan_path.is_empty() {
        errors.push(".plans/ACTIVE is empty".to_owned());
        return;
    }

    if !plan_path.starts_with(".plans/") {
        errors.push(format!(
            ".plans/ACTIVE must point inside .plans/: {plan_path}"
        ));
        return;
    }

    let plan = Path::new(plan_path);

    if !plan.is_file() {
        errors.push(format!(".plans/ACTIVE points to missing plan: {plan_path}"));
        return;
    }

    let plan_contents = match fs::read_to_string(plan) {
        Ok(contents) => contents,
        Err(error) => {
            errors.push(format!("unable to read active plan {plan_path}: {error}"));
            return;
        }
    };

    match plan_status(&plan_contents) {
        Some("Approved") => {}
        Some(status) => errors.push(format!(
            "active plan must have Status: Approved, found Status: {status}"
        )),
        None => errors.push(format!("active plan has no Status field: {plan_path}")),
    }
}

fn validate_plan_policy() -> Result<(), Vec<String>> {
    let staged_paths = match git_lines(&["diff", "--cached", "--name-only", "--diff-filter=ACMRTD"])
    {
        Ok(paths) => paths,
        Err(error) => return Err(vec![error]),
    };

    if !staged_paths.is_empty() {
        return validate_implementation_authority(&staged_paths, "HEAD", "staged changes");
    }

    validate_head_commit_policy()
}

fn validate_head_commit_policy() -> Result<(), Vec<String>> {
    let parents = match git_lines(&["rev-list", "--parents", "-n", "1", "HEAD"]) {
        Ok(lines) => lines,
        Err(error) => return Err(vec![error]),
    };

    let Some(parent_line) = parents.first() else {
        return Err(vec!["unable to resolve HEAD parents".to_owned()]);
    };

    let parts: Vec<&str> = parent_line.split_whitespace().collect();

    if parts.len() > 2 {
        println!("plan policy: merge commit integration boundary");
        return Ok(());
    }

    let changed_paths = match git_lines(&[
        "diff-tree",
        "--no-commit-id",
        "--name-only",
        "-r",
        "--root",
        "HEAD",
    ]) {
        Ok(paths) => paths,
        Err(error) => return Err(vec![error]),
    };

    if parts.len() == 1 {
        if changed_paths
            .iter()
            .any(|path| is_implementation_path(path))
        {
            return Err(vec![
                "root implementation commit has no prior Approved plan checkpoint".to_owned(),
            ]);
        }

        return Ok(());
    }

    validate_implementation_authority(&changed_paths, parts[1], "HEAD commit")
}

fn validate_implementation_authority(
    changed_paths: &[String],
    authority_ref: &str,
    context: &str,
) -> Result<(), Vec<String>> {
    if !changed_paths
        .iter()
        .any(|path| is_implementation_path(path))
    {
        return Ok(());
    }

    let active_plan = match git_show_optional(authority_ref, ".plans/ACTIVE") {
        Ok(Some(contents)) => contents.trim().to_owned(),
        Ok(None) => {
            return Err(vec![format!(
                "{context} contains implementation but {authority_ref} has no .plans/ACTIVE"
            )]);
        }
        Err(error) => return Err(vec![error]),
    };

    if active_plan.is_empty() {
        return Err(vec![format!(
            "{context} contains implementation but {authority_ref}:.plans/ACTIVE is empty"
        )]);
    }

    if !active_plan.starts_with(".plans/") {
        return Err(vec![format!(
            "{context} contains implementation but active plan is outside .plans/: {active_plan}"
        )]);
    }

    let plan_contents = match git_show_optional(authority_ref, &active_plan) {
        Ok(Some(contents)) => contents,
        Ok(None) => {
            return Err(vec![format!(
                "{context} contains implementation but {authority_ref}:{active_plan} is missing"
            )]);
        }
        Err(error) => return Err(vec![error]),
    };

    match plan_status(&plan_contents) {
        Some("Approved") => Ok(()),
        Some(status) => Err(vec![format!(
            "{context} contains implementation but prior plan status is {status}, not Approved"
        )]),
        None => Err(vec![format!(
            "{context} contains implementation but prior plan has no Status field"
        )]),
    }
}

fn is_implementation_path(path: &str) -> bool {
    path != ".plans/ACTIVE" && !path.ends_with(".md")
}

fn plan_status(contents: &str) -> Option<&str> {
    contents
        .lines()
        .find_map(|line| line.strip_prefix("Status: ").map(str::trim))
}

fn git_lines(args: &[&str]) -> Result<Vec<String>, String> {
    let output = git_output(args)?;

    Ok(output
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(ToOwned::to_owned)
        .collect())
}

fn git_show_optional(reference: &str, path: &str) -> Result<Option<String>, String> {
    let spec = format!("{reference}:{path}");
    let output = Command::new("git")
        .args(["show", &spec])
        .output()
        .map_err(|error| format!("unable to execute git show {spec}: {error}"))?;

    if output.status.success() {
        return String::from_utf8(output.stdout)
            .map(Some)
            .map_err(|error| format!("git show {spec} returned non-UTF-8 data: {error}"));
    }

    Ok(None)
}

fn git_output(args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .args(args)
        .output()
        .map_err(|error| format!("unable to execute git {}: {error}", args.join(" ")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("git {} failed: {}", args.join(" "), stderr.trim()));
    }

    String::from_utf8(output.stdout)
        .map_err(|error| format!("git {} returned non-UTF-8 data: {error}", args.join(" ")))
}

#[cfg(test)]
mod tests {
    use super::{
        adr_registry_errors, is_implementation_path, plan_status, preflight_patch_errors,
        unlinked_docs,
    };

    #[test]
    fn every_cli_path_dependency_needs_a_preflight_patch() {
        let manifest = "[dependencies]\n\
                        agentforge-core = { path = \"../agentforge-core\", version = \"0.3.0\" }\n\
                        agentforge-mcp = { path = \"../agentforge-mcp\", version = \"0.3.0\" }\n\
                        serde = \"1\"\n";
        let complete = "[patch.crates-io]\n\
                        agentforge-core = { path = \"crates/agentforge-core\" }\n\
                        agentforge-mcp = { path = \"crates/agentforge-mcp\" }\n";
        assert!(preflight_patch_errors(manifest, complete).is_empty());
        let missing =
            "[patch.crates-io]\nagentforge-core = { path = \"crates/agentforge-core\" }\n";
        assert_eq!(
            preflight_patch_errors(manifest, missing),
            [
                ".cargo/registry-preflight.toml: agentforge-mcp (a path dependency of agentforge-cli) \
              has no [patch.crates-io] entry; add `agentforge-mcp = { path = \"crates/agentforge-mcp\" }`"
            ]
        );
        let wrong = complete.replace("crates/agentforge-mcp", "crates/elsewhere");
        assert_eq!(
            preflight_patch_errors(manifest, &wrong),
            [
                ".cargo/registry-preflight.toml: agentforge-mcp is patched to crates/elsewhere, but \
              agentforge-cli uses crates/agentforge-mcp"
            ]
        );
    }

    fn ids(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_owned()).collect()
    }

    #[test]
    fn adr_registry_matches_files_exactly() {
        let registry = "| ID | Status | Decision |\n| --- | --- | --- |\n\
                        | ADR-0001 | Accepted | One |\n| ADR-0002 | Accepted | Two |\n";
        assert!(adr_registry_errors(&ids(&["ADR-0001", "ADR-0002"]), registry).is_empty());
        let missing = adr_registry_errors(&ids(&["ADR-0001", "ADR-0002", "ADR-0003"]), registry);
        assert_eq!(
            missing,
            ["docs/adr/README.md: ADR-0003 has a file but no registry row"]
        );
        let dangling = adr_registry_errors(&ids(&["ADR-0001"]), registry);
        assert_eq!(
            dangling,
            ["docs/adr/README.md: registry row ADR-0002 has no ADR file"]
        );
        let doubled = format!("{registry}| ADR-0002 | Accepted | Two again |\n");
        assert_eq!(
            adr_registry_errors(&ids(&["ADR-0001", "ADR-0002"]), &doubled),
            ["docs/adr/README.md: ADR-0002 is listed more than once"]
        );
    }

    #[test]
    fn every_doc_must_be_linked_from_the_readme() {
        let readme = "## Documentation map\n\n- [Gates](docs/GATES.md)\n";
        assert!(unlinked_docs(&ids(&["GATES.md"]), readme).is_empty());
        assert_eq!(
            unlinked_docs(&ids(&["GATES.md", "HIDDEN.md"]), readme),
            ["README.md: docs/HIDDEN.md is not linked from the documentation map"]
        );
    }

    #[test]
    fn approved_plan_status_is_detected() {
        assert_eq!(
            plan_status("# Plan\n\nStatus: Approved\n"),
            Some("Approved")
        );
    }

    #[test]
    fn non_approved_plan_status_is_detected() {
        assert_eq!(plan_status("Status: Draft\n"), Some("Draft"));
        assert_eq!(plan_status("Status: Complete\n"), Some("Complete"));
    }

    #[test]
    fn markdown_and_active_pointer_are_control_changes() {
        assert!(!is_implementation_path("docs/CI.md"));
        assert!(!is_implementation_path(".plans/P0-M003.plan.md"));
        assert!(!is_implementation_path(".plans/ACTIVE"));
    }

    #[test]
    fn source_workflow_and_script_changes_are_implementation() {
        assert!(is_implementation_path("src/main.rs"));
        assert!(is_implementation_path("Cargo.toml"));
        assert!(is_implementation_path(".github/workflows/ci.yml"));
        assert!(is_implementation_path("scripts/gate.sh"));
    }
}
