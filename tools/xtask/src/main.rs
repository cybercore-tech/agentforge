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

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
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
    use super::{is_implementation_path, plan_status};

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
