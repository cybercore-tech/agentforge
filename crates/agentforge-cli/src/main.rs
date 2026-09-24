//! `forge` command-line entry point.

use agentforge_adapter::{AgentProfileStore, ProcessAdapter, ProcessAdapterConfig};
use agentforge_audit::FileAuditStore;
use agentforge_core::agent::{ApprovalBoundary, Capability};
use agentforge_core::task::{TaskGraph, TaskId, TaskRecord};
use agentforge_daemon::{
    launch_profile as daemon_launch_profile, launch_task as daemon_launch_task,
    restart_with_program as daemon_restart_with_program, run_profile as daemon_run_profile,
    run_task as daemon_run_task, start_with_program as daemon_start_with_program,
    status as daemon_status, stop as daemon_stop,
};
use agentforge_intake::{
    GuidelineDocument, IntakeError, ProjectBlueprint, TaskDraft, approval_from_name, build_task,
    capability_from_name, commit_documents, initialize, load, restore_documents, role_from_name,
    serialize_blueprint, serialize_guidelines, snapshot, starter_bundle,
};
use agentforge_operator::{
    approve_task, approved_boundaries, inspect_task_diff, inspect_tasks, integrate_task,
    parse_approval_boundary, transition_task,
};
use agentforge_orchestrator::{execute_process_persisted, launch_process_persisted};
use agentforge_state::{FileTaskStore, TaskStore};
use agentforge_worktree::{GitOperation, WorktreeManager, WorktreeSpec};
use std::io::{self, Cursor, Read, Write};
use std::net::SocketAddr;
use std::path::Path;
use std::process::ExitCode;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::Duration;

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
        Some("intake") => intake_command(args.collect()),
        Some("blueprint") => blueprint_command(args.collect()),
        Some("task") => task_command(args.collect()),
        Some("agent") => agent_command(args.collect()),
        Some("gate") => gate_command(args.collect()),
        Some("ci") => ci_command(args.collect()),
        Some("run") => run_command(args.collect()),
        Some("daemon") => daemon_command(args.collect()),
        Some("worktree") => worktree_command(args.collect()),
        Some("hud") => hud_command(args.collect()),
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
        "usage: forge <version|doctor|status|init <root>|intake <root> [--task] [--input-file <path>]|blueprint validate <root>|task create|inspect|launch|launch-batch|diff|approve|integrate|accept|cancel|retry ...|agent list|validate|inspect <root> [<profile>]|gate list <root>|ci observe <root> <repository> <workflow> <sha> [--task <task-id>]|run <root> <task-id> <absolute-executable> [--interactive] [--pty]|run <root> <task-id> --profile <profile> [--interactive] [--pty]|daemon start|restart|status|run|launch|stop ...|worktree create|inspect|list|retire ...|hud <root> [--watch [--interval-ms <milliseconds>]]>"
    );
}

fn gate_command(arguments: Vec<String>) -> ExitCode {
    match arguments.first().map(String::as_str) {
        Some("list") if arguments.len() == 2 => {
            match agentforge_gate::GateProfileStore::new(&arguments[1]).list() {
                Ok(gates) => {
                    if gates.is_empty() {
                        println!("no gates");
                    }
                    for gate in gates {
                        println!(
                            "gate id={} executable={} arguments={} environment={} timeout_ms={} max_output_bytes={}",
                            gate.name(),
                            gate.executable().display(),
                            gate.arguments().len(),
                            gate.environment().len(),
                            gate.timeout().as_millis(),
                            gate.max_output_bytes()
                        );
                    }
                    ExitCode::SUCCESS
                }
                Err(error) => {
                    eprintln!("gate list failed: {error}");
                    ExitCode::from(1)
                }
            }
        }
        _ => {
            eprintln!("gate requires: list <root>");
            print_usage();
            ExitCode::from(2)
        }
    }
}

fn ci_command(arguments: Vec<String>) -> ExitCode {
    let usage = "ci requires: observe <root> <repository> <workflow> <sha> [--task <task-id>]";
    let task_id = match arguments.get(5..) {
        Some([]) => None,
        Some([flag, value]) if flag == "--task" => match TaskId::parse(value.clone()) {
            Ok(task_id) => Some(task_id),
            Err(error) => {
                eprintln!("invalid task ID: {error}");
                return ExitCode::from(2);
            }
        },
        _ => {
            eprintln!("{usage}");
            print_usage();
            return ExitCode::from(2);
        }
    };
    if arguments.first().map(String::as_str) != Some("observe") {
        eprintln!("{usage}");
        print_usage();
        return ExitCode::from(2);
    }
    let request = agentforge_ci::CiObservationRequest::new(
        arguments[2].clone(),
        arguments[3].clone(),
        arguments[4].clone(),
    );
    match agentforge_operator::observe_ci(&arguments[1], &request, task_id.as_ref()) {
        Ok(observation) => {
            let run = &observation.run;
            println!(
                "ci run={} sha={} status={} conclusion={}",
                run.provider_id(),
                run.head_sha(),
                run.status().as_str(),
                run.conclusion()
                    .map_or("none", agentforge_ci::CiConclusion::as_str)
            );
            for job in run.jobs() {
                let classification = observation
                    .classifications
                    .iter()
                    .find(|(name, _)| name == job.name())
                    .map(|(_, classification)| {
                        format!(" category={}", classification.category().as_str())
                    })
                    .unwrap_or_default();
                println!(
                    "job {:?} status={} conclusion={}{classification}",
                    job.name(),
                    job.status().as_str(),
                    job.conclusion()
                        .map_or("none", agentforge_ci::CiConclusion::as_str)
                );
            }
            println!(
                "recorded CiObserved and {} FailureClassified event(s)",
                observation.classifications.len()
            );
            if observation.pending() {
                ExitCode::from(3)
            } else if observation.succeeded() {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(1)
            }
        }
        Err(error) => {
            eprintln!("ci observe failed: {error}");
            ExitCode::from(1)
        }
    }
}

/// Prints one line per orchestrated gate and returns whether all gates passed.
fn report_gates(execution: &agentforge_orchestrator::ProcessExecution) -> bool {
    for gate in &execution.gates {
        println!(
            "gate {} outcome={} exit={}",
            gate.name(),
            gate.outcome_label(),
            gate.exit_code()
                .map_or_else(|| "none".to_owned(), |code| code.to_string())
        );
    }
    let passed = execution.gates.iter().filter(|gate| gate.passed()).count();
    println!("gates={passed}/{}", execution.gates.len());
    if !execution.gates_passed() {
        eprintln!(
            "gates failed; task {} was marked failed with gate evidence in the audit log",
            execution.report.task_id()
        );
    }
    execution.gates_passed()
}

fn agent_command(arguments: Vec<String>) -> ExitCode {
    match arguments.first().map(String::as_str) {
        Some("list") if arguments.len() == 2 => {
            let store = AgentProfileStore::new(&arguments[1]);
            match store.list() {
                Ok(profiles) => {
                    if profiles.is_empty() {
                        println!("no agent profiles");
                    } else {
                        for profile in profiles {
                            print_agent_profile("profile", &profile);
                        }
                    }
                    ExitCode::SUCCESS
                }
                Err(error) => agent_profile_error("list", error),
            }
        }
        Some("validate") if arguments.len() == 3 => {
            let store = AgentProfileStore::new(&arguments[1]);
            match store.load(&arguments[2]) {
                Ok(profile) => {
                    println!(
                        "agent profile valid id={} executable={}",
                        profile.id(),
                        profile.executable().display()
                    );
                    ExitCode::SUCCESS
                }
                Err(error) => agent_profile_error("validate", error),
            }
        }
        Some("inspect") if arguments.len() == 3 => {
            let store = AgentProfileStore::new(&arguments[1]);
            match store.load(&arguments[2]) {
                Ok(profile) => {
                    print_agent_profile("inspected", &profile);
                    ExitCode::SUCCESS
                }
                Err(error) => agent_profile_error("inspect", error),
            }
        }
        _ => {
            eprintln!(
                "agent requires: list <root>, validate <root> <profile>, or inspect <root> <profile>"
            );
            print_usage();
            ExitCode::from(2)
        }
    }
}

fn print_agent_profile(label: &str, profile: &agentforge_adapter::AgentProfile) {
    println!(
        "{label} agent id={} executable={} arguments={} environment={} timeout_ms={} max_output_bytes={}",
        profile.id(),
        profile.executable().display(),
        profile.arguments().len(),
        profile.environment().len(),
        profile.timeout().as_millis(),
        profile.max_output_bytes()
    );
}

fn agent_profile_error(operation: &str, error: agentforge_adapter::AdapterError) -> ExitCode {
    eprintln!("agent profile {operation} failed: {error}");
    ExitCode::from(1)
}

fn worktree_command(arguments: Vec<String>) -> ExitCode {
    match arguments.first().map(String::as_str) {
        Some("create") if arguments.len() == 4 => {
            let task_id = match parse_worktree_task_id(&arguments[2]) {
                Ok(value) => value,
                Err(code) => return code,
            };
            let manager = match WorktreeManager::new(&arguments[1]) {
                Ok(value) => value,
                Err(error) => return worktree_error("create", error),
            };
            match manager.create(&WorktreeSpec::new(task_id.clone(), &arguments[3])) {
                Ok(status) => {
                    print_worktree_status("created", &status);
                    ExitCode::SUCCESS
                }
                Err(error) => worktree_error("create", error),
            }
        }
        Some("inspect") if arguments.len() == 3 => {
            let task_id = match parse_worktree_task_id(&arguments[2]) {
                Ok(value) => value,
                Err(code) => return code,
            };
            let manager = match WorktreeManager::new(&arguments[1]) {
                Ok(value) => value,
                Err(error) => return worktree_error("inspect", error),
            };
            match manager.inspect(&task_id) {
                Ok(Some(status)) => {
                    print_worktree_status("inspected", &status);
                    ExitCode::SUCCESS
                }
                Ok(None) => {
                    eprintln!("worktree inspect failed: task is not managed: {task_id}");
                    ExitCode::from(1)
                }
                Err(error) => worktree_error("inspect", error),
            }
        }
        Some("list") if arguments.len() == 2 => {
            let manager = match WorktreeManager::new(&arguments[1]) {
                Ok(value) => value,
                Err(error) => return worktree_error("list", error),
            };
            match manager.list() {
                Ok(statuses) => {
                    for status in &statuses {
                        print_worktree_status("managed", status);
                    }
                    if statuses.is_empty() {
                        println!("no managed worktrees");
                    }
                    ExitCode::SUCCESS
                }
                Err(error) => worktree_error("list", error),
            }
        }
        Some("retire") if arguments.len() == 3 => {
            let task_id = match parse_worktree_task_id(&arguments[2]) {
                Ok(value) => value,
                Err(code) => return code,
            };
            let manager = match WorktreeManager::new(&arguments[1]) {
                Ok(value) => value,
                Err(error) => return worktree_error("retire", error),
            };
            let branch = WorktreeManager::branch_for(&task_id);
            match manager.retire(&task_id) {
                Ok(()) => {
                    println!(
                        "retired worktree task={task_id} branch={branch} branch-preserved=true"
                    );
                    ExitCode::SUCCESS
                }
                Err(error) => worktree_error("retire", error),
            }
        }
        _ => {
            eprintln!(
                "worktree requires: create <root> <task-id> <base-ref>, inspect <root> <task-id>, list <root>, or retire <root> <task-id>"
            );
            print_usage();
            ExitCode::from(2)
        }
    }
}

fn parse_worktree_task_id(value: &str) -> Result<TaskId, ExitCode> {
    TaskId::parse(value.to_owned()).map_err(|error| {
        eprintln!("invalid task ID: {error}");
        ExitCode::from(2)
    })
}

fn print_worktree_status(label: &str, status: &agentforge_worktree::WorktreeStatus) {
    println!(
        "{label} worktree task={} branch={} path={} head={} dirty={} operation={}",
        status.task_id(),
        status.branch(),
        status.path().display(),
        status.head(),
        status.is_dirty(),
        operation_label(status.operation())
    );
}

fn operation_label(operation: Option<GitOperation>) -> &'static str {
    match operation {
        None => "none",
        Some(GitOperation::Merge) => "merge",
        Some(GitOperation::Rebase) => "rebase",
        Some(GitOperation::CherryPick) => "cherry-pick",
        Some(GitOperation::Revert) => "revert",
    }
}

fn worktree_error(operation: &str, error: agentforge_worktree::WorktreeError) -> ExitCode {
    eprintln!("worktree {operation} failed: {error}");
    ExitCode::from(1)
}

fn daemon_command(arguments: Vec<String>) -> ExitCode {
    match arguments.first().map(String::as_str) {
        Some("start" | "restart") if arguments.len() == 2 || arguments.len() == 4 => {
            let root = &arguments[1];
            let bind = match parse_daemon_bind(&arguments) {
                Ok(bind) => bind,
                Err(error) => {
                    eprintln!("{error}");
                    return ExitCode::from(2);
                }
            };
            let result = if arguments[0] == "start" {
                daemon_start_with_program(root, bind, forged_program())
            } else {
                daemon_restart_with_program(root, bind, forged_program())
            };
            match result {
                Ok(status) => {
                    println!(
                        "daemon running at {} (pid {})",
                        status.endpoint.address, status.endpoint.pid
                    );
                    ExitCode::SUCCESS
                }
                Err(error) => {
                    eprintln!("daemon {} failed: {error}", arguments[0]);
                    ExitCode::from(1)
                }
            }
        }
        Some("status") if arguments.len() == 2 => match daemon_status(&arguments[1]) {
            Ok(status) => {
                println!(
                    "daemon running at {} (pid {})",
                    status.endpoint.address, status.endpoint.pid
                );
                ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!("daemon status failed: {error}");
                ExitCode::from(1)
            }
        },
        Some("run") if arguments.len() == 4 || arguments.len() == 5 => {
            let task_id = match TaskId::parse(arguments[2].clone()) {
                Ok(value) => value,
                Err(error) => {
                    eprintln!("invalid task ID: {error}");
                    return ExitCode::from(2);
                }
            };
            let result = if arguments.len() == 5 {
                if arguments[3] != "--profile" {
                    eprintln!("daemon run requires --profile before a profile ID");
                    return ExitCode::from(2);
                }
                daemon_run_profile(&arguments[1], &task_id, &arguments[4])
            } else {
                daemon_run_task(&arguments[1], &task_id, &arguments[3])
            };
            match result {
                Ok(message) => {
                    println!("{message}");
                    ExitCode::SUCCESS
                }
                Err(error) => {
                    eprintln!("daemon run failed: {error}");
                    ExitCode::from(1)
                }
            }
        }
        Some("launch") => daemon_launch_command(&arguments[1..]),
        Some("stop") if arguments.len() == 2 => match daemon_stop(&arguments[1]) {
            Ok(()) => {
                println!("daemon stopped");
                ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!("daemon stop failed: {error}");
                ExitCode::from(1)
            }
        },
        _ => {
            eprintln!(
                "daemon requires: status <root>, run <root> <task-id> <absolute-executable>, run <root> <task-id> --profile <profile>, launch <root> <task-id> <absolute-executable> [--base <ref>], launch <root> <task-id> --profile <profile> [--base <ref>], or stop <root>"
            );
            print_usage();
            ExitCode::from(2)
        }
    }
}

fn daemon_launch_command(arguments: &[String]) -> ExitCode {
    if arguments.len() < 3 {
        eprintln!(
            "daemon launch requires: <root> <task-id> <absolute-executable> [--base <ref>] or --profile <profile> [--base <ref>]"
        );
        return ExitCode::from(2);
    }
    let task_id = match TaskId::parse(arguments[1].clone()) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("invalid task ID: {error}");
            return ExitCode::from(2);
        }
    };
    let mut executable = None;
    let mut profile = None;
    let mut base_ref = String::from("HEAD");
    let mut base_supplied = false;
    let mut index = 2;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--base" => {
                let Some(value) = arguments.get(index + 1) else {
                    eprintln!("--base requires a non-empty ref");
                    return ExitCode::from(2);
                };
                if value.is_empty() || value.starts_with('-') || base_supplied {
                    eprintln!("daemon launch accepts one non-empty --base ref");
                    return ExitCode::from(2);
                }
                base_ref = value.clone();
                base_supplied = true;
                index += 2;
            }
            "--profile" => {
                let Some(value) = arguments.get(index + 1) else {
                    eprintln!("--profile requires a profile ID");
                    return ExitCode::from(2);
                };
                if value.is_empty()
                    || value.starts_with('-')
                    || profile.is_some()
                    || executable.is_some()
                {
                    eprintln!("--profile requires one profile ID");
                    return ExitCode::from(2);
                }
                profile = Some(value.clone());
                index += 2;
            }
            value if value.starts_with('-') => {
                eprintln!("unknown daemon launch option: {value}");
                return ExitCode::from(2);
            }
            value => {
                if executable.is_some() || profile.is_some() {
                    eprintln!("daemon launch accepts one executable or one --profile");
                    return ExitCode::from(2);
                }
                executable = Some(value.to_owned());
                index += 1;
            }
        }
    }
    let result = match (executable, profile) {
        (Some(executable), None) => {
            daemon_launch_task(&arguments[0], &task_id, executable, &base_ref)
        }
        (None, Some(profile)) => {
            daemon_launch_profile(&arguments[0], &task_id, &profile, &base_ref)
        }
        (None, None) => {
            eprintln!("daemon launch requires an executable or --profile <profile>");
            return ExitCode::from(2);
        }
        (Some(_), Some(_)) => unreachable!("daemon launch parser prevents both modes"),
    };
    match result {
        Ok(message) => {
            println!("{message}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("daemon launch failed: {error}");
            ExitCode::from(1)
        }
    }
}

fn parse_daemon_bind(arguments: &[String]) -> Result<SocketAddr, String> {
    if arguments.len() == 2 {
        return Ok(agentforge_daemon::DEFAULT_BIND);
    }
    if arguments.len() != 4 || arguments[2] != "--bind" {
        return Err("daemon start/restart accepts [--bind <loopback-address>]".to_owned());
    }
    let bind = arguments[3]
        .parse::<SocketAddr>()
        .map_err(|_| "daemon bind must be a socket address".to_owned())?;
    if !bind.ip().is_loopback() {
        return Err("daemon bind address must be loopback".to_owned());
    }
    Ok(bind)
}

fn forged_program() -> std::path::PathBuf {
    let current = std::env::current_exe().unwrap_or_else(|_| std::path::PathBuf::from("forge"));
    let candidate = current.parent().map(|parent| {
        parent.join(if cfg!(windows) {
            "forged.exe"
        } else {
            "forged"
        })
    });
    match candidate {
        Some(path) if path.is_file() => path,
        _ => std::path::PathBuf::from(if cfg!(windows) {
            "forged.exe"
        } else {
            "forged"
        }),
    }
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

const MAX_GUIDED_LINE_BYTES: usize = 4096;
const MAX_GUIDED_LIST_ITEMS: usize = 128;
const MAX_GUIDED_INPUT_FILE_BYTES: u64 = 512 * 1024;

fn intake_command(arguments: Vec<String>) -> ExitCode {
    if arguments.is_empty() {
        eprintln!("intake requires: <root> [--task] [--input-file <path>]");
        print_usage();
        return ExitCode::from(2);
    }
    let root = std::path::PathBuf::from(&arguments[0]);
    let mut with_task = false;
    let mut input_file = None;
    let mut index = 1;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--task" if !with_task => with_task = true,
            "--input-file" if input_file.is_none() => {
                let Some(value) = arguments.get(index + 1) else {
                    eprintln!("--input-file requires a path");
                    return ExitCode::from(2);
                };
                if value.is_empty() || value.starts_with('-') {
                    eprintln!("--input-file requires a non-empty path");
                    return ExitCode::from(2);
                }
                input_file = Some(std::path::PathBuf::from(value));
                index += 1;
            }
            option => {
                eprintln!("unknown intake option: {option}");
                print_usage();
                return ExitCode::from(2);
            }
        }
        index += 1;
    }
    let expected_sources = match snapshot(&root) {
        Ok(value) => value,
        Err(error) => {
            print_intake_error(&error);
            return ExitCode::from(1);
        }
    };
    let bundle = match (&expected_sources.blueprint, &expected_sources.guidelines) {
        (Some(_), Some(_)) => match load(&root) {
            Ok(value) => value,
            Err(error) => {
                print_intake_error(&error);
                return ExitCode::from(1);
            }
        },
        (None, None) => starter_bundle(),
        _ => {
            eprintln!("intake requires both blueprint and guidelines, or neither");
            return ExitCode::from(1);
        }
    };

    let task_store = FileTaskStore::for_project_root(&root);
    let expected_task_state = match read_optional_file(task_store.path()) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("cannot read task state: {error}");
            return ExitCode::from(1);
        }
    };
    let existing_graph = if with_task {
        match task_store.load() {
            Ok(Some(graph)) => graph,
            Ok(None) => TaskGraph::new(),
            Err(error) => {
                eprintln!("cannot load task state: {error}");
                return ExitCode::from(1);
            }
        }
    } else {
        TaskGraph::new()
    };

    let stdin = io::stdin();
    let mut reader: Box<dyn Read> = match input_file.as_deref() {
        Some(path) => match read_guided_input_file(path) {
            Ok(bytes) => Box::new(Cursor::new(bytes)),
            Err(error) => {
                eprintln!("intake input file failed: {error}");
                return ExitCode::from(1);
            }
        },
        None => Box::new(stdin.lock()),
    };
    let blueprint = match guided_blueprint(&mut reader, &bundle.blueprint) {
        Ok(Some(value)) => value,
        Ok(None) => return intake_cancelled(),
        Err(error) => return guided_input_error(error),
    };
    let guidelines = match guided_guidelines(&mut reader, &bundle.guidelines) {
        Ok(Some(value)) => value,
        Ok(None) => return intake_cancelled(),
        Err(error) => return guided_input_error(error),
    };
    let task = if with_task {
        match guided_task(&mut reader, &blueprint) {
            Ok(Some(value)) => Some(value),
            Ok(None) => return intake_cancelled(),
            Err(error) => return guided_input_error(error),
        }
    } else {
        None
    };
    let next_graph = match task.as_ref() {
        Some(task) => match build_guided_graph(&existing_graph, task, &blueprint, &root) {
            Ok(graph) => Some(graph),
            Err(error) => {
                print_intake_error(&error);
                return ExitCode::from(1);
            }
        },
        None => None,
    };

    if let Err(error) = print_guided_preview(&blueprint, &guidelines, task.as_ref()) {
        eprintln!("cannot render intake preview: {error}");
        return ExitCode::from(1);
    }
    let confirmed = match prompt_line(&mut reader, "Confirm changes? [y/N]: ") {
        Ok(Some(value)) => matches!(value.trim().to_ascii_lowercase().as_str(), "y" | "yes"),
        Ok(None) => return intake_cancelled(),
        Err(error) => return guided_input_error(error),
    };
    if !confirmed {
        return intake_cancelled();
    }

    if let Err(error) = verify_task_state(task_store.path(), &expected_task_state) {
        eprintln!("intake commit failed: {error}");
        return ExitCode::from(1);
    }
    if let Err(error) = commit_documents(&root, &blueprint, &guidelines, &expected_sources) {
        print_intake_error(&error);
        return ExitCode::from(1);
    }
    if let Some(graph) = next_graph {
        if let Err(error) = task_store.save(&graph) {
            if let Err(rollback) = restore_documents(&root, &expected_sources) {
                eprintln!("cannot restore intake after task-state failure: {rollback}");
            }
            eprintln!("cannot save task state: {error}");
            return ExitCode::from(1);
        }
        println!("created task {}", task.expect("task exists").task_id);
    }
    println!("updated {}", agentforge_intake::BLUEPRINT_RELATIVE_PATH);
    println!("updated {}", agentforge_intake::GUIDELINES_RELATIVE_PATH);
    ExitCode::SUCCESS
}

fn guided_blueprint(
    reader: &mut impl Read,
    current: &ProjectBlueprint,
) -> Result<Option<ProjectBlueprint>, String> {
    let name = match prompt_text(reader, "Project name", Some(&current.name), true)? {
        Some(value) => value,
        None => return Ok(None),
    };
    let mission = match prompt_text(reader, "Mission", Some(&current.mission), true)? {
        Some(value) => value,
        None => return Ok(None),
    };
    let default_milestone = match prompt_text(
        reader,
        "Default milestone (blank for none)",
        current.default_milestone.as_deref(),
        false,
    )? {
        Some(value) if value == "-" => None,
        Some(value) if value.is_empty() => None,
        Some(value) => Some(value),
        None => return Ok(None),
    };
    let allowed_paths = match prompt_list(reader, "Allowed paths", &current.allowed_paths)? {
        Some(value) => value,
        None => return Ok(None),
    };
    let forbidden_paths = match prompt_list(reader, "Forbidden paths", &current.forbidden_paths)? {
        Some(value) => value,
        None => return Ok(None),
    };
    let capabilities = match prompt_capabilities(reader, "Capabilities", &current.capabilities)? {
        Some(value) => value,
        None => return Ok(None),
    };
    let approvals = match prompt_approvals(reader, "Approvals", &current.approvals)? {
        Some(value) => value,
        None => return Ok(None),
    };
    let gates = match prompt_list(reader, "Quality gates", &current.gates)? {
        Some(value) => value,
        None => return Ok(None),
    };
    Ok(Some(ProjectBlueprint {
        version: agentforge_intake::BLUEPRINT_VERSION,
        name,
        mission,
        default_milestone,
        allowed_paths,
        forbidden_paths,
        capabilities,
        approvals,
        gates,
    }))
}

fn guided_guidelines(
    reader: &mut impl Read,
    current: &GuidelineDocument,
) -> Result<Option<GuidelineDocument>, String> {
    println!("Guidelines body (finish with a single '.'; blank first line keeps current):");
    let first = match bounded_line(reader, MAX_GUIDED_LINE_BYTES)? {
        Some(value) => value,
        None => return Ok(None),
    };
    if first.trim() == "." || first.trim().is_empty() {
        return Ok(Some(current.clone()));
    }
    let mut lines = vec![first];
    loop {
        let line = match bounded_line(reader, MAX_GUIDED_LINE_BYTES)? {
            Some(value) => value,
            None => return Ok(None),
        };
        if line.trim() == "." {
            break;
        }
        lines.push(line);
        if lines.join("\n").len() > 256 * 1024 {
            return Err("guidelines exceed 256 KiB".to_owned());
        }
    }
    Ok(Some(GuidelineDocument {
        version: agentforge_intake::GUIDELINES_VERSION,
        body: lines.join("\n"),
    }))
}

fn guided_task(
    reader: &mut impl Read,
    blueprint: &ProjectBlueprint,
) -> Result<Option<TaskDraft>, String> {
    let task_id = match prompt_text(reader, "Task ID", None, true)? {
        Some(value) => value,
        None => return Ok(None),
    };
    let milestone_id = match prompt_text(
        reader,
        "Task milestone",
        blueprint.default_milestone.as_deref(),
        true,
    )? {
        Some(value) => Some(value),
        None => return Ok(None),
    };
    let role_name = match prompt_text(reader, "Task role", Some("implementer"), true)? {
        Some(value) => value,
        None => return Ok(None),
    };
    let primary_role =
        role_from_name(&role_name).ok_or_else(|| format!("unknown task role: {role_name}"))?;
    let goal = match prompt_text(reader, "Task goal", None, true)? {
        Some(value) => value,
        None => return Ok(None),
    };
    let non_goals = match prompt_list(reader, "Task non-goals", &[])? {
        Some(value) => value,
        None => return Ok(None),
    };
    let dependency_task_ids = match prompt_list(reader, "Task dependencies", &[])? {
        Some(value) => value,
        None => return Ok(None),
    };
    let allowed_paths = match prompt_list(reader, "Task allowed paths (blank uses blueprint)", &[])?
    {
        Some(value) => value,
        None => return Ok(None),
    };
    let forbidden_paths =
        match prompt_list(reader, "Task forbidden paths (blank uses blueprint)", &[])? {
            Some(value) => value,
            None => return Ok(None),
        };
    let capabilities =
        match prompt_capabilities(reader, "Task capabilities (blank uses blueprint)", &[])? {
            Some(value) => value,
            None => return Ok(None),
        };
    let required_approvals =
        match prompt_approvals(reader, "Task approvals (blank uses blueprint)", &[])? {
            Some(value) => value,
            None => return Ok(None),
        };
    let required_gates =
        match prompt_list(reader, "Task quality gates (blank uses blueprint)", &[])? {
            Some(value) => value,
            None => return Ok(None),
        };
    let expected_outputs = match prompt_list(reader, "Task expected outputs", &[])? {
        Some(value) => value,
        None => return Ok(None),
    };
    let evidence_requirements = match prompt_list(reader, "Task evidence requirements", &[])? {
        Some(value) => value,
        None => return Ok(None),
    };
    Ok(Some(TaskDraft {
        task_id,
        milestone_id,
        primary_role,
        goal,
        non_goals,
        dependency_task_ids,
        allowed_paths,
        forbidden_paths,
        capabilities,
        required_approvals,
        required_gates,
        expected_outputs,
        evidence_requirements,
    }))
}

fn build_guided_graph(
    graph: &TaskGraph,
    draft: &TaskDraft,
    blueprint: &ProjectBlueprint,
    root: &Path,
) -> Result<TaskGraph, IntakeError> {
    let task = build_task(draft, blueprint, root)?;
    let task_id = TaskId::parse(task.task_id.clone())
        .map_err(|error| IntakeError::Task(format!("invalid task ID: {error}")))?;
    if graph.get(&task_id).is_some() {
        return Err(IntakeError::Task(format!("task already exists: {task_id}")));
    }
    let record = TaskRecord::new(task).map_err(|error| IntakeError::Task(error.to_string()))?;
    let mut records = graph.records().cloned().collect::<Vec<_>>();
    records.push(record);
    TaskGraph::from_records(records).map_err(|error| IntakeError::Task(error.to_string()))
}

fn print_guided_preview(
    blueprint: &ProjectBlueprint,
    guidelines: &GuidelineDocument,
    task: Option<&TaskDraft>,
) -> Result<(), IntakeError> {
    let blueprint = serialize_blueprint(blueprint)?;
    let guidelines = serialize_guidelines(guidelines)?;
    println!("\nintake preview");
    println!("blueprint:");
    for line in String::from_utf8_lossy(&blueprint).lines() {
        println!("  {line}");
    }
    println!("guidelines:");
    for line in String::from_utf8_lossy(&guidelines).lines() {
        println!("  {line}");
    }
    if let Some(task) = task {
        println!("task draft:");
        println!("  id={}", task.task_id);
        println!(
            "  milestone={}",
            task.milestone_id
                .as_deref()
                .unwrap_or("<blueprint default>")
        );
        println!("  role={}", task.primary_role.as_str());
        println!("  goal={}", task.goal);
    }
    Ok(())
}

fn prompt_text(
    reader: &mut impl Read,
    label: &str,
    default: Option<&str>,
    required: bool,
) -> Result<Option<String>, String> {
    let suffix = default
        .map(|value| format!(" [{value}]"))
        .unwrap_or_default();
    let value = match prompt_line(reader, &format!("{label}{suffix}: "))? {
        Some(value) => value.trim().to_owned(),
        None => return Ok(None),
    };
    if value.is_empty() {
        if let Some(default) = default {
            return Ok(Some(default.to_owned()));
        }
        if required {
            return Err(format!("{label} must not be empty"));
        }
    }
    Ok(Some(value))
}

fn prompt_list(
    reader: &mut impl Read,
    label: &str,
    defaults: &[String],
) -> Result<Option<Vec<String>>, String> {
    let default_text = if defaults.is_empty() {
        None
    } else {
        Some(defaults.join(","))
    };
    let value = match prompt_text(reader, label, default_text.as_deref(), false)? {
        Some(value) => value,
        None => return Ok(None),
    };
    if value.is_empty() {
        return Ok(Some(defaults.to_vec()));
    }
    if value == "-" {
        return Ok(Some(Vec::new()));
    }
    let values = value
        .split(',')
        .map(str::trim)
        .map(str::to_owned)
        .collect::<Vec<_>>();
    validate_list_values(label, &values)?;
    Ok(Some(values))
}

fn prompt_capabilities(
    reader: &mut impl Read,
    label: &str,
    defaults: &[Capability],
) -> Result<Option<Vec<Capability>>, String> {
    let names = defaults
        .iter()
        .map(|value| value.as_str().to_owned())
        .collect::<Vec<_>>();
    let values = match prompt_list(reader, label, &names)? {
        Some(value) => value,
        None => return Ok(None),
    };
    values
        .into_iter()
        .map(|value| {
            capability_from_name(&value).ok_or_else(|| format!("unknown capability: {value}"))
        })
        .collect::<Result<Vec<_>, _>>()
        .map(Some)
}

fn prompt_approvals(
    reader: &mut impl Read,
    label: &str,
    defaults: &[ApprovalBoundary],
) -> Result<Option<Vec<ApprovalBoundary>>, String> {
    let names = defaults
        .iter()
        .map(|value| value.as_str().to_owned())
        .collect::<Vec<_>>();
    let values = match prompt_list(reader, label, &names)? {
        Some(value) => value,
        None => return Ok(None),
    };
    values
        .into_iter()
        .map(|value| {
            approval_from_name(&value).ok_or_else(|| format!("unknown approval boundary: {value}"))
        })
        .collect::<Result<Vec<_>, _>>()
        .map(Some)
}

fn validate_list_values(label: &str, values: &[String]) -> Result<(), String> {
    if values.len() > MAX_GUIDED_LIST_ITEMS {
        return Err(format!("{label} exceeds {MAX_GUIDED_LIST_ITEMS} items"));
    }
    if values.iter().any(|value| value.is_empty()) {
        return Err(format!("{label} contains an empty item"));
    }
    if values
        .iter()
        .any(|value| value.len() > MAX_GUIDED_LINE_BYTES)
    {
        return Err(format!(
            "{label} item exceeds {MAX_GUIDED_LINE_BYTES} bytes"
        ));
    }
    Ok(())
}

fn prompt_line(reader: &mut impl Read, prompt: &str) -> Result<Option<String>, String> {
    print!("{prompt}");
    io::stdout().flush().map_err(|error| error.to_string())?;
    bounded_line(reader, MAX_GUIDED_LINE_BYTES)
}

fn bounded_line(reader: &mut impl Read, limit: usize) -> Result<Option<String>, String> {
    let mut bytes = Vec::new();
    let mut buffer = [0_u8; 1];
    let mut oversized = false;
    loop {
        match reader.read(&mut buffer) {
            Ok(0) => {
                if bytes.is_empty() && !oversized {
                    return Ok(None);
                }
                break;
            }
            Ok(_) if buffer[0] == b'\n' => break,
            Ok(_) if bytes.len() < limit => bytes.push(buffer[0]),
            Ok(_) => oversized = true,
            Err(error) => return Err(error.to_string()),
        }
    }
    if oversized {
        return Err(format!("input exceeds {limit} bytes"));
    }
    let value = String::from_utf8(bytes).map_err(|_| "input must be valid UTF-8".to_owned())?;
    Ok(Some(value.trim_end_matches('\r').to_owned()))
}

fn read_optional_file(path: &Path) -> Result<Option<Vec<u8>>, std::io::Error> {
    match std::fs::read(path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

fn verify_task_state(path: &Path, expected: &Option<Vec<u8>>) -> Result<(), String> {
    let current = read_optional_file(path).map_err(|error| error.to_string())?;
    if &current == expected {
        Ok(())
    } else {
        Err(format!(
            "task state changed during edit: {}",
            path.display()
        ))
    }
}

fn intake_cancelled() -> ExitCode {
    println!("intake cancelled; no changes written");
    ExitCode::SUCCESS
}

fn guided_input_error(error: String) -> ExitCode {
    eprintln!("intake input failed: {error}");
    ExitCode::from(1)
}

fn read_guided_input_file(path: &Path) -> Result<Vec<u8>, String> {
    let metadata = std::fs::metadata(path).map_err(|error| error.to_string())?;
    if !metadata.is_file() {
        return Err("input path is not a regular file".to_owned());
    }
    if metadata.len() > MAX_GUIDED_INPUT_FILE_BYTES {
        return Err(format!(
            "input file exceeds {MAX_GUIDED_INPUT_FILE_BYTES} bytes"
        ));
    }
    let bytes = std::fs::read(path).map_err(|error| error.to_string())?;
    if bytes.len() as u64 > MAX_GUIDED_INPUT_FILE_BYTES {
        return Err(format!(
            "input file exceeds {MAX_GUIDED_INPUT_FILE_BYTES} bytes"
        ));
    }
    std::str::from_utf8(&bytes).map_err(|_| "input file must be valid UTF-8".to_owned())?;
    Ok(bytes)
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
    match arguments.first().map(String::as_str) {
        Some("create") => task_create_command(arguments[1..].to_vec()),
        Some("inspect") => task_inspect_command(&arguments[1..]),
        Some("launch") => task_launch_command(&arguments[1..]),
        Some("launch-batch") => task_launch_batch_command(&arguments[1..]),
        Some("diff") => task_diff_command(&arguments[1..]),
        Some("approve") => task_approve_command(&arguments[1..]),
        Some("integrate") => task_integrate_command(&arguments[1..]),
        Some("accept") => {
            task_transition_command(&arguments[1..], agentforge_core::task::TaskState::Succeeded)
        }
        Some("cancel") => {
            task_transition_command(&arguments[1..], agentforge_core::task::TaskState::Cancelled)
        }
        Some("retry") => {
            task_transition_command(&arguments[1..], agentforge_core::task::TaskState::Pending)
        }
        _ => {
            eprintln!(
                "task requires: create, inspect, launch, launch-batch, diff, approve, integrate, accept, cancel, or retry"
            );
            print_usage();
            ExitCode::from(2)
        }
    }
}

const DEFAULT_BATCH_MAX: usize = 4;
const MAX_BATCH_MAX: usize = 16;

fn task_launch_batch_command(arguments: &[String]) -> ExitCode {
    let usage = "task launch-batch requires: <root> <absolute-executable> or --profile <profile> [--base <ref>] [--max <1-16>]";
    let Some(root) = arguments.first().map(std::path::PathBuf::from) else {
        eprintln!("{usage}");
        return ExitCode::from(2);
    };
    let mut executable: Option<String> = None;
    let mut profile: Option<String> = None;
    let mut base_ref: Option<String> = None;
    let mut max: Option<usize> = None;
    let mut index = 1;
    while index < arguments.len() {
        let value = arguments.get(index + 1);
        match arguments[index].as_str() {
            "--base" => match value {
                Some(value)
                    if !value.is_empty() && !value.starts_with('-') && base_ref.is_none() =>
                {
                    base_ref = Some(value.clone());
                    index += 2;
                }
                _ => {
                    eprintln!("--base requires one non-empty ref");
                    return ExitCode::from(2);
                }
            },
            "--max" => match value.and_then(|value| value.parse::<usize>().ok()) {
                Some(parsed) if (1..=MAX_BATCH_MAX).contains(&parsed) && max.is_none() => {
                    max = Some(parsed);
                    index += 2;
                }
                _ => {
                    eprintln!("--max requires one number from 1 to {MAX_BATCH_MAX}");
                    return ExitCode::from(2);
                }
            },
            "--profile" => match value {
                Some(value)
                    if !value.is_empty()
                        && !value.starts_with('-')
                        && profile.is_none()
                        && executable.is_none() =>
                {
                    profile = Some(value.clone());
                    index += 2;
                }
                _ => {
                    eprintln!("--profile requires one profile ID");
                    return ExitCode::from(2);
                }
            },
            option if option.starts_with('-') => {
                eprintln!("unknown task launch-batch option: {option}");
                return ExitCode::from(2);
            }
            other => {
                if executable.is_some() || profile.is_some() {
                    eprintln!("task launch-batch accepts one executable or one --profile");
                    return ExitCode::from(2);
                }
                executable = Some(other.to_owned());
                index += 1;
            }
        }
    }
    let config = match (profile, executable) {
        (Some(profile), None) => match AgentProfileStore::new(&root).load(&profile) {
            Ok(profile) => profile.adapter_config(),
            Err(error) => {
                eprintln!("cannot load agent profile: {error}");
                return ExitCode::from(2);
            }
        },
        (None, Some(executable)) => ProcessAdapterConfig::new("task-launch-batch", executable),
        _ => {
            eprintln!("{usage}");
            return ExitCode::from(2);
        }
    };
    let adapter = match ProcessAdapter::new(config) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("invalid adapter configuration: {error}");
            return ExitCode::from(2);
        }
    };

    let task_store = FileTaskStore::for_project_root(&root);
    let graph = match task_store.load() {
        Ok(Some(graph)) => graph,
        Ok(None) => {
            eprintln!("task launch-batch failed: task state snapshot is missing");
            return ExitCode::from(1);
        }
        Err(error) => {
            eprintln!("task launch-batch failed: {error}");
            return ExitCode::from(1);
        }
    };
    let mut approvals = std::collections::BTreeMap::new();
    for record in graph.records() {
        match approved_boundaries(&root, record.id()) {
            Ok(boundaries) => {
                approvals.insert(record.id().clone(), boundaries);
            }
            Err(error) => {
                eprintln!("cannot verify approvals: {error}");
                return ExitCode::from(1);
            }
        }
    }
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
    let batch = match agentforge_orchestrator::launch_batch_persisted(
        &root,
        &task_store,
        &mut audit_store,
        &adapter,
        &approvals,
        base_ref.as_deref().unwrap_or("HEAD"),
        max.unwrap_or(DEFAULT_BATCH_MAX),
    ) {
        Ok(batch) => batch,
        Err(error) => {
            eprintln!("task launch-batch failed: {error}");
            return ExitCode::from(1);
        }
    };
    if batch.outcomes.is_empty() && batch.deferred.is_empty() {
        println!("no ready tasks");
        return ExitCode::SUCCESS;
    }
    for outcome in &batch.outcomes {
        match outcome {
            agentforge_orchestrator::BatchTaskOutcome::Launched {
                task_id,
                report,
                gates,
                worktree,
                worktree_created,
            } => {
                let passed = gates.iter().filter(|gate| gate.passed()).count();
                println!(
                    "launched {task_id} termination={:?} exit={} gates={passed}/{} worktree-created={worktree_created} path={}",
                    report.termination(),
                    report
                        .exit_code()
                        .map_or_else(|| "none".to_owned(), |code| code.to_string()),
                    gates.len(),
                    worktree.display()
                );
            }
            agentforge_orchestrator::BatchTaskOutcome::AdapterFailed { task_id, error } => {
                println!("failed {task_id} error={error}");
            }
            agentforge_orchestrator::BatchTaskOutcome::Skipped { task_id, reason } => {
                println!("skipped {task_id} reason={reason}");
            }
        }
    }
    for deferred in &batch.deferred {
        match &deferred.reason {
            agentforge_orchestrator::DeferReason::Overlap { owner, path } => println!(
                "deferred {} reason=overlap owner={owner} path={path}",
                deferred.task_id
            ),
            agentforge_orchestrator::DeferReason::Capacity => {
                println!("deferred {} reason=capacity", deferred.task_id);
            }
        }
    }
    let launched = batch
        .outcomes
        .iter()
        .filter(|outcome| {
            matches!(
                outcome,
                agentforge_orchestrator::BatchTaskOutcome::Launched { .. }
            )
        })
        .count();
    println!(
        "batch base={} launched={launched} succeeded={} deferred={}",
        batch.base_commit,
        batch
            .outcomes
            .iter()
            .filter(|outcome| outcome.succeeded())
            .count(),
        batch.deferred.len()
    );
    if batch.succeeded() {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}

fn task_launch_command(arguments: &[String]) -> ExitCode {
    if arguments.len() < 3 {
        eprintln!(
            "task launch requires: <root> <task-id> <absolute-executable> or --profile <profile> [--base <ref>] [--interactive] [--pty]"
        );
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
    let mut executable: Option<String> = None;
    let mut profile: Option<String> = None;
    let mut base_ref = String::from("HEAD");
    let mut base_supplied = false;
    let mut interactive = false;
    let mut pty = false;
    let mut index = 2;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--base" => {
                let Some(value) = arguments.get(index + 1) else {
                    eprintln!("--base requires a non-empty ref");
                    return ExitCode::from(2);
                };
                if value.is_empty() || value.starts_with('-') {
                    eprintln!("--base requires a non-empty ref");
                    return ExitCode::from(2);
                }
                if base_supplied {
                    eprintln!("task launch accepts --base at most once");
                    return ExitCode::from(2);
                }
                base_supplied = true;
                base_ref = value.clone();
                index += 2;
            }
            "--interactive" => {
                if interactive {
                    eprintln!("task launch accepts --interactive at most once");
                    return ExitCode::from(2);
                }
                interactive = true;
                index += 1;
            }
            "--pty" => {
                if pty {
                    eprintln!("task launch accepts --pty at most once");
                    return ExitCode::from(2);
                }
                pty = true;
                index += 1;
            }
            "--profile" => {
                let Some(value) = arguments.get(index + 1) else {
                    eprintln!("--profile requires a profile ID");
                    return ExitCode::from(2);
                };
                if value.is_empty()
                    || value.starts_with('-')
                    || profile.is_some()
                    || executable.is_some()
                {
                    eprintln!("--profile requires one profile ID");
                    return ExitCode::from(2);
                }
                profile = Some(value.clone());
                index += 2;
            }
            value if value.starts_with('-') => {
                eprintln!("unknown task launch option: {value}");
                return ExitCode::from(2);
            }
            value => {
                if executable.is_some() || profile.is_some() {
                    eprintln!("task launch accepts one executable or one --profile");
                    return ExitCode::from(2);
                }
                executable = Some(value.to_owned());
                index += 1;
            }
        }
    }
    if pty && !interactive {
        eprintln!("task launch requires --interactive when --pty is selected");
        return ExitCode::from(2);
    }
    if executable.is_none() && profile.is_none() {
        eprintln!("task launch requires an executable or --profile <profile>");
        return ExitCode::from(2);
    }

    let mut config = if let Some(profile) = profile {
        match AgentProfileStore::new(&root).load(&profile) {
            Ok(profile) => profile.adapter_config(),
            Err(error) => {
                eprintln!("cannot load agent profile: {error}");
                return ExitCode::from(2);
            }
        }
    } else {
        ProcessAdapterConfig::new(
            "task-launch-process",
            executable.expect("validated executable"),
        )
    };
    if interactive {
        config = config.with_interactive();
    }
    if pty {
        config = config.with_pty();
    }
    let adapter = match ProcessAdapter::new(config) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("invalid adapter configuration: {error}");
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
    let approvals = match approved_boundaries(&root, &task_id) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("cannot verify approvals: {error}");
            return ExitCode::from(1);
        }
    };
    match launch_process_persisted(
        &root,
        &task_store,
        &mut audit_store,
        &task_id,
        &adapter,
        &approvals,
        &base_ref,
    ) {
        Ok(launch) => {
            println!(
                "task {} launched; termination={:?} base={} worktree-created={} path={}",
                launch.execution.report.task_id(),
                launch.execution.report.termination(),
                launch.base_commit,
                launch.worktree_created,
                launch.worktree.path().display()
            );
            let gates_passed = report_gates(&launch.execution);
            println!(
                "recovery: inspect with `forge task inspect {} {}` and review with `forge task diff {} {}`",
                root.display(),
                task_id,
                root.display(),
                task_id
            );
            if gates_passed {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(1)
            }
        }
        Err(error) => {
            eprintln!("task launch failed: {error}");
            eprintln!(
                "recovery: inspect with `forge task inspect {} {}` and `forge worktree inspect {} {}`",
                root.display(),
                task_id,
                root.display(),
                task_id
            );
            ExitCode::from(1)
        }
    }
}

fn task_diff_command(arguments: &[String]) -> ExitCode {
    if arguments.len() != 2 {
        eprintln!("task diff requires: <root> <task-id>");
        return ExitCode::from(2);
    }
    let task_id = match TaskId::parse(arguments[1].clone()) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("invalid task ID: {error}");
            return ExitCode::from(2);
        }
    };
    match inspect_task_diff(&arguments[0], &task_id) {
        Ok(diff) => {
            println!(
                "task diff task={} source={} source_head={} target={} target_head={} merge_base={} dirty={}",
                diff.task_id(),
                diff.source_branch(),
                diff.source_head(),
                diff.target_branch(),
                diff.target_head(),
                diff.merge_base(),
                diff.source_dirty()
            );
            for path in diff.changed_files() {
                println!("  {path}");
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("task diff failed: {error}");
            ExitCode::from(1)
        }
    }
}

fn task_integrate_command(arguments: &[String]) -> ExitCode {
    if arguments.len() != 6 || arguments[2] != "--target" || arguments[4] != "--actor" {
        eprintln!("task integrate requires: <root> <task-id> --target <branch> --actor <actor-id>");
        return ExitCode::from(2);
    }
    let task_id = match TaskId::parse(arguments[1].clone()) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("invalid task ID: {error}");
            return ExitCode::from(2);
        }
    };
    match integrate_task(&arguments[0], &task_id, &arguments[3], &arguments[5]) {
        Ok(report) => {
            println!(
                "task {} integrated target={} source_head={} target_before={} target_after={} already_integrated={}",
                report.task_id(),
                report.target_branch(),
                report.source_head(),
                report.target_before(),
                report.target_after(),
                report.already_integrated()
            );
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("task integrate failed: {error}");
            ExitCode::from(1)
        }
    }
}

fn task_inspect_command(arguments: &[String]) -> ExitCode {
    if !(arguments.len() == 1 || arguments.len() == 2) {
        eprintln!("task inspect requires: <root> [<task-id>]");
        return ExitCode::from(2);
    }
    let task_id = match arguments.get(1).map(|value| TaskId::parse(value.clone())) {
        Some(Ok(value)) => Some(value),
        Some(Err(error)) => {
            eprintln!("invalid task ID: {error}");
            return ExitCode::from(2);
        }
        None => None,
    };
    match inspect_tasks(&arguments[0], task_id.as_ref()) {
        Ok(tasks) => {
            for task in tasks {
                println!(
                    "task={} state={} revision={} milestone={} ready={} goal={}",
                    task.task_id, task.state, task.revision, task.milestone, task.ready, task.goal
                );
                println!("  dependencies: {}", task.dependencies.join(","));
                println!("  approvals: {}", task.required_approvals.join(","));
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("task inspect failed: {error}");
            ExitCode::from(1)
        }
    }
}

fn task_approve_command(arguments: &[String]) -> ExitCode {
    if arguments.len() != 5 || arguments[3] != "--actor" {
        eprintln!("task approve requires: <root> <task-id> <approval-boundary> --actor <actor-id>");
        return ExitCode::from(2);
    }
    let task_id = match TaskId::parse(arguments[1].clone()) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("invalid task ID: {error}");
            return ExitCode::from(2);
        }
    };
    let boundary = match parse_approval_boundary(&arguments[2]) {
        Some(value) => value,
        None => {
            eprintln!("unknown approval boundary: {}", arguments[2]);
            return ExitCode::from(2);
        }
    };
    match approve_task(&arguments[0], &task_id, boundary, &arguments[4]) {
        Ok(()) => {
            println!("approved {} for {}", boundary.as_str(), task_id);
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("task approval failed: {error}");
            ExitCode::from(1)
        }
    }
}

fn task_transition_command(
    arguments: &[String],
    next: agentforge_core::task::TaskState,
) -> ExitCode {
    if arguments.len() != 4 || arguments[2] != "--actor" {
        eprintln!("task action requires: <root> <task-id> --actor <actor-id>");
        return ExitCode::from(2);
    }
    let task_id = match TaskId::parse(arguments[1].clone()) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("invalid task ID: {error}");
            return ExitCode::from(2);
        }
    };
    match transition_task(&arguments[0], &task_id, next, &arguments[3]) {
        Ok(revision) => {
            println!(
                "task {} transitioned to {} at revision {}",
                task_id,
                next.as_str(),
                revision
            );
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("task transition failed: {error}");
            ExitCode::from(1)
        }
    }
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
    let task = match build_task(&draft, &bundle.blueprint, &root) {
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
    let interactive_count = arguments
        .iter()
        .filter(|argument| argument.as_str() == "--interactive")
        .count();
    if interactive_count > 1 {
        eprintln!("run accepts --interactive at most once");
        print_usage();
        return ExitCode::from(2);
    }
    let interactive = interactive_count == 1;
    let pty_count = arguments
        .iter()
        .filter(|argument| argument.as_str() == "--pty")
        .count();
    if pty_count > 1 {
        eprintln!("run accepts --pty at most once");
        print_usage();
        return ExitCode::from(2);
    }
    let pty = pty_count == 1;
    if pty && !interactive {
        eprintln!("run requires --interactive when --pty is selected");
        print_usage();
        return ExitCode::from(2);
    }
    let mut command_arguments = arguments;
    if interactive {
        command_arguments.retain(|argument| argument != "--interactive");
    }
    if pty {
        command_arguments.retain(|argument| argument != "--pty");
    }
    if command_arguments.len() != 3
        && !(command_arguments.len() == 4
            && command_arguments.get(2).map(String::as_str) == Some("--profile"))
    {
        eprintln!("run requires: <root> <task-id> <absolute-executable> or --profile <profile>");
        print_usage();
        return ExitCode::from(2);
    }
    let root = std::path::PathBuf::from(&command_arguments[0]);
    let task_id = match TaskId::parse(command_arguments[1].clone()) {
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
    let mut config = if command_arguments.len() == 4 {
        match AgentProfileStore::new(&root).load(&command_arguments[3]) {
            Ok(profile) => profile.adapter_config(),
            Err(error) => {
                eprintln!("cannot load agent profile: {error}");
                return ExitCode::from(2);
            }
        }
    } else {
        ProcessAdapterConfig::new("cli-process", command_arguments[2].clone())
    };
    if interactive {
        config = config.with_interactive();
    };
    if pty {
        config = config.with_pty();
    }
    let adapter = match ProcessAdapter::new(config) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("invalid adapter configuration: {error}");
            return ExitCode::from(2);
        }
    };
    let approvals = match approved_boundaries(&root, &task_id) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("cannot verify approvals: {error}");
            return ExitCode::from(1);
        }
    };
    match execute_process_persisted(
        &root,
        &task_store,
        &mut audit_store,
        &task_id,
        &adapter,
        &approvals,
    ) {
        Ok(execution) => {
            println!(
                "task {} launched; termination={:?}",
                execution.report.task_id(),
                execution.report.termination()
            );
            if report_gates(&execution) {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(1)
            }
        }
        Err(error) => {
            eprintln!("task run failed: {error}");
            ExitCode::from(1)
        }
    }
}

fn hud_command(arguments: Vec<String>) -> ExitCode {
    if arguments.is_empty() {
        eprintln!("hud requires: <root> [--watch [--interval-ms <milliseconds>]]");
        print_usage();
        return ExitCode::from(2);
    }
    let root = std::path::PathBuf::from(&arguments[0]);
    if arguments.len() == 1 {
        return render_one_shot_hud(&root);
    }
    if arguments.get(1).map(String::as_str) != Some("--watch") {
        eprintln!("hud accepts only --watch after <root>");
        print_usage();
        return ExitCode::from(2);
    }
    let mut config = agentforge_hud::WatchConfig::default();
    let mut index = 2;
    while index < arguments.len() {
        if arguments.get(index).map(String::as_str) != Some("--interval-ms")
            || index + 1 >= arguments.len()
        {
            eprintln!("--interval-ms requires a numeric value");
            return ExitCode::from(2);
        }
        let value = match arguments[index + 1].parse::<u64>() {
            Ok(value) => value,
            Err(_) => {
                eprintln!("--interval-ms requires a numeric value");
                return ExitCode::from(2);
            }
        };
        config = agentforge_hud::WatchConfig::new(value);
        index += 2;
    }
    run_hud_watch(&root, config)
}

fn render_one_shot_hud(root: &Path) -> ExitCode {
    match agentforge_hud::collect(root) {
        Ok(snapshot) => {
            print!("{}", agentforge_hud::render(&snapshot));
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("cannot render HUD: {error}");
            ExitCode::from(1)
        }
    }
}

fn run_hud_watch(root: &Path, config: agentforge_hud::WatchConfig) -> ExitCode {
    if !root.is_dir() {
        eprintln!("HUD root is not a directory: {}", root.display());
        return ExitCode::from(1);
    }
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || read_watch_commands(sender));
    let interval = Duration::from_millis(config.interval_ms());

    loop {
        match agentforge_hud::collect(root) {
            Ok(snapshot) => print!("{}", agentforge_hud::render(&snapshot)),
            Err(error) => println!("HUD diagnostic: {error}"),
        }
        let _ = io::stdout().flush();
        match receiver.recv_timeout(interval) {
            Ok(agentforge_hud::WatchCommand::Refresh) => continue,
            Ok(agentforge_hud::WatchCommand::Help) => {
                println!("{}", agentforge_hud::watch_help());
            }
            Ok(agentforge_hud::WatchCommand::Quit) | Err(RecvTimeoutError::Disconnected) => {
                return ExitCode::SUCCESS;
            }
            Ok(agentforge_hud::WatchCommand::Ignore) => {}
            Ok(agentforge_hud::WatchCommand::Invalid(input)) => {
                println!("HUD diagnostic: unknown watch command: {input}");
            }
            Err(RecvTimeoutError::Timeout) => {}
        }
    }
}

fn read_watch_commands(sender: mpsc::Sender<agentforge_hud::WatchCommand>) {
    let stdin = io::stdin();
    let mut input = stdin.lock();
    loop {
        let Some(line) = read_bounded_line(&mut input) else {
            return;
        };
        if sender
            .send(agentforge_hud::parse_watch_command(&line))
            .is_err()
        {
            return;
        }
    }
}

fn read_bounded_line(reader: &mut impl Read) -> Option<String> {
    let mut line = String::new();
    let mut overflow = false;
    let mut bytes = 0;
    let mut buffer = [0_u8; 1];
    loop {
        match reader.read(&mut buffer) {
            Ok(0) => {
                if line.is_empty() && !overflow {
                    return None;
                }
                break;
            }
            Ok(_) if buffer[0] == b'\n' => break,
            Ok(_) if bytes < agentforge_hud::MAX_WATCH_INPUT_BYTES => {
                line.push(buffer[0] as char);
                bytes += 1;
            }
            Ok(_) => overflow = true,
            Err(_) => return None,
        }
    }
    if overflow {
        Some("<oversized input>".to_owned())
    } else {
        Some(line.trim_end_matches('\r').to_owned())
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
