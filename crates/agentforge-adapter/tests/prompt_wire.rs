//! The task-prompt document as the remote-worker contract wire format (P4-M007).

use agentforge_adapter::{MAX_TASK_PROMPT_BYTES, parse_task_prompt, render_task_prompt};
use agentforge_core::agent::{AgentRole, AgentTask, ApprovalBoundary, Capability};

fn full_task() -> AgentTask {
    let mut task = AgentTask::new(
        "P4-M007-T0001",
        "P4-M007",
        AgentRole::Implementer,
        "multi\nline goal with café and \u{1F680}",
    );
    task.non_goals = vec!["no transport".into(), "".into()];
    task.dependency_task_ids = vec!["P4-M007-T0000".into()];
    task.allowed_paths = vec!["crates/a".into(), "docs/x.md".into()];
    task.forbidden_paths = vec!["secrets".into()];
    task.capabilities = vec![
        Capability::RunLocalCommands,
        Capability::WriteOwnedPaths,
        Capability::MergeProtectedBranch,
    ];
    task.required_approvals = vec![
        ApprovalBoundary::ActivateImplementationPlan,
        ApprovalBoundary::MergeProtectedBranch,
    ];
    task.required_gates = vec!["workspace".into()];
    task.expected_outputs = vec!["a commit".into()];
    task.evidence_requirements = vec!["gate log\nwith newline".into()];
    task
}

#[test]
fn render_then_parse_is_identity() {
    let task = full_task();
    let document = render_task_prompt(&task);
    let parsed = parse_task_prompt(&document).expect("parse");
    assert_eq!(parsed, task);
    assert_eq!(
        render_task_prompt(&parsed),
        document,
        "byte-identical re-render"
    );

    let minimal = AgentTask::new("P4-M007-T0002", "P4-M007", AgentRole::Reviewer, "g");
    assert_eq!(
        parse_task_prompt(&render_task_prompt(&minimal)).expect("minimal"),
        minimal
    );
}

#[test]
fn malformed_documents_fail_closed() {
    let document = render_task_prompt(&full_task());
    let text = String::from_utf8(document.clone()).expect("utf8");
    let cases: Vec<(&str, Vec<u8>)> = vec![
        (
            "header",
            text.replacen("prompt-v1", "prompt-v2", 1).into_bytes(),
        ),
        ("truncated", document[..document.len() - 5].to_vec()),
        ("trailing", [document.clone(), b"x".to_vec()].concat()),
        (
            "bad length",
            text.replacen("task_id 13\n", "task_id 14\n", 1)
                .into_bytes(),
        ),
        (
            "renamed field",
            text.replacen("\ngoal ", "\nGOAL ", 1).into_bytes(),
        ),
        (
            "unknown capability",
            text.replacen("run_local_commands", "run_local_commandz", 1)
                .into_bytes(),
        ),
        (
            "unknown approval",
            text.replacen("change_governance", "xchange_governanc", 1)
                .replacen(
                    "activate_implementation_plan",
                    "activate_implementation_plaX",
                    1,
                )
                .into_bytes(),
        ),
        (
            "huge count",
            text.replacen("non_goals 2\n", "non_goals 999999\n", 1)
                .into_bytes(),
        ),
        ("empty", Vec::new()),
    ];
    for (label, bytes) in cases {
        assert!(parse_task_prompt(&bytes).is_err(), "{label} was accepted");
    }
    let oversize = vec![b'a'; MAX_TASK_PROMPT_BYTES + 1];
    assert!(parse_task_prompt(&oversize).is_err());
}

#[test]
fn reordered_fields_are_rejected() {
    let text = String::from_utf8(render_task_prompt(&full_task())).expect("utf8");
    // Swap the task_id and milestone_id fields.
    let task_line = "task_id 13\nP4-M007-T0001\n";
    let milestone_line = "milestone_id 7\nP4-M007\n";
    assert!(text.contains(task_line) && text.contains(milestone_line));
    let swapped = text
        .replacen(task_line, "@@", 1)
        .replacen(milestone_line, task_line, 1)
        .replacen("@@", milestone_line, 1);
    assert!(parse_task_prompt(swapped.as_bytes()).is_err());
}
