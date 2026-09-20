//! `forge` command-line entry point.

use agentforge_adapter::{ProcessAdapter, ProcessAdapterConfig};
use agentforge_audit::FileAuditStore;
use agentforge_core::task::{TaskGraph, TaskId, TaskRecord};
use agentforge_intake::{
    IntakeError, TaskDraft, approval_from_name, build_task, capability_from_name, initialize, load,
    role_from_name,
};
use agentforge_orchestrator::execute_process_persisted;
use agentforge_state::{FileTaskStore, TaskStore};
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
        Some("init") => init_command(args.collect()),
        Some("blueprint") => blueprint_command(args.collect()),
        Some("task") => task_command(args.collect()),
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
    println!(
        "usage: forge <version|doctor|status|init <root>|blueprint validate <root>|task create <root> <task-id> <milestone> <role> <goal> [options]|run <root> <task-id> <absolute-executable>>"
    );
}

fn init_command(arguments: Vec<String>) -> ExitCode {
    if arguments.len() != 1 {
        eprintln!("init requires: <root>");
        print_usage();
        return ExitCode::from(2);
    }
    match initialize(&arguments[0]) {
        Ok((blueprint, guidelines)) => {
            println!("created {}", blueprint.display());
            println!("created {}", guidelines.display());
            ExitCode::SUCCESS
        }
        Err(error) => {
            print_intake_error(&error);
            ExitCode::from(1)
        }
    }
}

fn blueprint_command(arguments: Vec<String>) -> ExitCode {
    if arguments.len() != 2 || arguments[0] != "validate" {
        eprintln!("blueprint requires: validate <root>");
        print_usage();
        return ExitCode::from(2);
    }
    match load(&arguments[1]) {
        Ok(bundle) => {
            println!(
                "valid blueprint: {} (guidelines v{})",
                bundle.blueprint.name, bundle.guidelines.version
            );
            ExitCode::SUCCESS
        }
        Err(error) => {
            print_intake_error(&error);
            ExitCode::from(1)
        }
    }
}

fn task_command(arguments: Vec<String>) -> ExitCode {
    if arguments.first().map(String::as_str) != Some("create") {
        eprintln!("task requires: create <root> <task-id> <milestone> <role> <goal> [options]");
        print_usage();
        return ExitCode::from(2);
    }
    task_create_command(arguments[1..].to_vec())
}

fn task_create_command(arguments: Vec<String>) -> ExitCode {
    if arguments.len() < 5 {
        eprintln!("task create requires: <root> <task-id> <milestone> <role> <goal> [options]");
        print_usage();
        return ExitCode::from(2);
    }
    let root = std::path::PathBuf::from(&arguments[0]);
    let role = match role_from_name(&arguments[3]) {
        Some(role) => role,
        None => {
            eprintln!("unknown role: {}", arguments[3]);
            return ExitCode::from(2);
        }
    };
    let mut draft = TaskDraft::new(arguments[1].clone(), role, arguments[4].clone());
    if !arguments[2].is_empty() {
        draft.milestone_id = Some(arguments[2].clone());
    }
    let mut index = 5;
    while index < arguments.len() {
        let option = &arguments[index];
        index += 1;
        let value = match next_option_value(&arguments, &mut index, option) {
            Ok(value) => value,
            Err(error) => {
                eprintln!("{error}");
                return ExitCode::from(2);
            }
        };
        match option.as_str() {
            "--milestone" => draft.milestone_id = Some(value),
            "--non-goal" => draft.non_goals.push(value),
            "--depends-on" => draft.dependency_task_ids.push(value),
            "--allowed" => draft.allowed_paths.push(value),
            "--forbidden" => draft.forbidden_paths.push(value),
            "--gate" => draft.required_gates.push(value),
            "--output" => draft.expected_outputs.push(value),
            "--evidence" => draft.evidence_requirements.push(value),
            "--capability" => match capability_from_name(&value) {
                Some(capability) => draft.capabilities.push(capability),
                None => {
                    eprintln!("unknown capability: {value}");
                    return ExitCode::from(2);
                }
            },
            "--approval" => match approval_from_name(&value) {
                Some(approval) => draft.required_approvals.push(approval),
                None => {
                    eprintln!("unknown approval boundary: {value}");
                    return ExitCode::from(2);
                }
            },
            _ => {
                eprintln!("unknown task option: {option}");
                return ExitCode::from(2);
            }
        }
    }

    let bundle = match load(&root) {
        Ok(bundle) => bundle,
        Err(error) => {
            print_intake_error(&error);
            return ExitCode::from(1);
        }
    };
    let task = match build_task(&draft, &bundle.blueprint) {
        Ok(task) => task,
        Err(error) => {
            print_intake_error(&error);
            return ExitCode::from(1);
        }
    };
    let store = FileTaskStore::for_project_root(&root);
    let graph = match store.load() {
        Ok(Some(graph)) => graph,
        Ok(None) => TaskGraph::new(),
        Err(error) => {
            eprintln!("cannot load task state: {error}");
            return ExitCode::from(1);
        }
    };
    let task_id = match TaskId::parse(task.task_id.clone()) {
        Ok(task_id) => task_id,
        Err(error) => {
            eprintln!("invalid task ID: {error}");
            return ExitCode::from(2);
        }
    };
    if graph.get(&task_id).is_some() {
        eprintln!("task already exists: {task_id}");
        return ExitCode::from(1);
    }
    let record = match TaskRecord::new(task) {
        Ok(record) => record,
        Err(error) => {
            eprintln!("cannot create task record: {error}");
            return ExitCode::from(1);
        }
    };
    let mut records = graph.records().cloned().collect::<Vec<_>>();
    records.push(record);
    let next_graph = match TaskGraph::from_records(records) {
        Ok(graph) => graph,
        Err(error) => {
            eprintln!("cannot create task graph: {error}");
            return ExitCode::from(1);
        }
    };
    if let Err(error) = store.save(&next_graph) {
        eprintln!("cannot save task state: {error}");
        return ExitCode::from(1);
    }
    println!("created task {task_id}");
    ExitCode::SUCCESS
}

fn next_option_value(
    arguments: &[String],
    index: &mut usize,
    option: &str,
) -> Result<String, String> {
    if !matches!(
        option,
        "--milestone"
            | "--non-goal"
            | "--depends-on"
            | "--allowed"
            | "--forbidden"
            | "--capability"
            | "--approval"
            | "--gate"
            | "--output"
            | "--evidence"
    ) {
        return Ok(String::new());
    }
    let Some(value) = arguments.get(*index) else {
        return Err(format!("{option} requires a value"));
    };
    *index += 1;
    if value.is_empty() || value.starts_with('-') {
        return Err(format!("{option} requires a non-empty value"));
    }
    Ok(value.clone())
}

fn print_intake_error(error: &IntakeError) {
    eprintln!("{error}");
    if let IntakeError::Validation(errors) = error {
        for diagnostic in errors {
            eprintln!("  {diagnostic}");
        }
    }
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
