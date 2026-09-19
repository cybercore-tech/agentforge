//! Repository automation for AgentForge.

use std::fs;
use std::path::Path;
use std::process::ExitCode;

const REQUIRED_PATHS: &[&str] = &[
    "PROJECT_SPEC.md",
    "AGENTS.md",
    "PROJECT_STATE.md",
    "AGENT_HANDOFF.md",
    "CONTRIBUTING.md",
    "docs/MILESTONES.md",
    "docs/adr/README.md",
    ".plans/README.md",
    ".plans/TEMPLATE.plan.md",
    "scripts/gate.sh",
    "scripts/check-text-files",
    "scripts/project-status",
];

fn main() -> ExitCode {
    match std::env::args().nth(1).as_deref() {
        Some("validate") => validate(),
        _ => {
            eprintln!("usage: cargo run -p xtask -- validate");
            ExitCode::from(2)
        }
    }
}

fn validate() -> ExitCode {
    let mut errors = Vec::new();

    validate_required_paths(&mut errors);
    validate_active_plan(&mut errors);

    if errors.is_empty() {
        println!("AgentForge repository validation: ok");
        ExitCode::SUCCESS
    } else {
        for error in errors {
            eprintln!("{error}");
        }

        ExitCode::FAILURE
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

    // No active plan is a valid repository state between milestones.
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
    }
}
