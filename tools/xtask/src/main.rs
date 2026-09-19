//! Repository automation for AgentForge.

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
    ".plans/ACTIVE",
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
    let mut missing = Vec::new();

    for path in REQUIRED_PATHS {
        if !Path::new(path).exists() {
            missing.push(*path);
        }
    }

    if missing.is_empty() {
        println!("AgentForge repository validation: ok");
        ExitCode::SUCCESS
    } else {
        for path in missing {
            eprintln!("missing required path: {path}");
        }
        ExitCode::FAILURE
    }
}
