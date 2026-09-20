//! `forge` command-line entry point.

use agentforge_adapter::{ProcessAdapter, ProcessAdapterConfig};
use agentforge_audit::FileAuditStore;
use agentforge_core::task::TaskId;
use agentforge_orchestrator::execute_process_persisted;
use agentforge_state::FileTaskStore;
use std::path::Path;
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);

    match args.next().as_deref() {
        Some("version" | "--version" | "-V") => {
            println!(
                "{} {}",
                agentforge_core::PRODUCT_NAME,
                agentforge_core::version()
            );
            ExitCode::SUCCESS
        }
        Some("doctor") => {
            print_doctor(Path::new("."));
            ExitCode::SUCCESS
        }
        Some("status") => {
            print_status(Path::new("."));
            ExitCode::SUCCESS
        }
        Some("run") => run_command(args.collect()),
        Some(other) => {
            eprintln!("unknown command: {other}");
            print_usage();
            ExitCode::from(2)
        }
        None => {
            print_usage();
            ExitCode::SUCCESS
        }
    }
}

fn print_usage() {
    println!("usage: forge <version|doctor|status|run <root> <task-id> <absolute-executable>>");
}

fn run_command(arguments: Vec<String>) -> ExitCode {
    if arguments.len() != 3 {
        eprintln!("run requires: <root> <task-id> <absolute-executable>");
        print_usage();
        return ExitCode::from(2);
    }
    let root = std::path::PathBuf::from(&arguments[0]);
    let task_id = match TaskId::parse(arguments[1].clone()) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("invalid task ID: {error}");
            return ExitCode::from(2);
        }
    };
    let task_store = FileTaskStore::for_project_root(&root);
    let audit_dir = root.join(".forge");
    if let Err(error) = std::fs::create_dir_all(&audit_dir) {
        eprintln!("cannot create audit directory: {error}");
        return ExitCode::from(1);
    }
    let mut audit_store = match FileAuditStore::open(audit_dir.join("audit.log")) {
        Ok(store) => store,
        Err(error) => {
            eprintln!("cannot open audit log: {error}");
            return ExitCode::from(1);
        }
    };
    let adapter = match ProcessAdapter::new(ProcessAdapterConfig::new(
        "cli-process",
        arguments[2].clone(),
    )) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("invalid adapter configuration: {error}");
            return ExitCode::from(2);
        }
    };
    match execute_process_persisted(
        &root,
        &task_store,
        &mut audit_store,
        &task_id,
        &adapter,
        &[],
    ) {
        Ok(execution) => {
            println!(
                "task {} launched; termination={:?}",
                execution.report.task_id(),
                execution.report.termination()
            );
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("task run failed: {error}");
            ExitCode::from(1)
        }
    }
}

fn print_doctor(root: &Path) {
    println!("AgentForge doctor");
    println!("version: {}", agentforge_core::version());
    println!("workspace: {}", check(root.join("Cargo.toml")));
    println!("git: {}", check(root.join(".git")));
    println!("plan_pointer: {}", check(root.join(".plans/ACTIVE")));
    println!("environment: externally managed");
    println!("mutations: none");
}

fn print_status(root: &Path) {
    println!("AgentForge status");
    println!("version: {}", agentforge_core::version());
    let active = root.join(".plans/ACTIVE");
    match std::fs::read_to_string(&active) {
        Ok(contents) => println!("active_plan: {}", contents.trim()),
        Err(_) => println!("active_plan: none"),
    }
    println!("project: {}", check(root.join("PROJECT_STATE.md")));
    println!("worktrees: observed through managed boundaries");
    println!("agents: no active process registry");
    println!("blockers: inspect project state and plan evidence");
}

fn check(path: impl AsRef<Path>) -> &'static str {
    if path.as_ref().exists() {
        "ok"
    } else {
        "missing"
    }
}

#[cfg(test)]
mod tests {
    use super::{check, print_status};
    use std::path::Path;

    #[test]
    fn checks_are_read_only_and_deterministic() {
        assert_eq!(check(Path::new("Cargo.toml")), "ok");
        assert_eq!(
            check(Path::new("definitely-missing-agentforge-path")),
            "missing"
        );
    }

    #[test]
    fn status_probe_does_not_require_an_active_plan() {
        print_status(Path::new("definitely-missing-agentforge-path"));
    }
}
