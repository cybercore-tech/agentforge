//! Integration coverage for bounded direct process gates.

use agentforge_gate::{GateDefinition, GateError, GateOutcome, GateRunner};
use std::time::Duration;

fn executable(name: &str) -> String {
    format!("/usr/bin/{name}")
}

#[test]
fn direct_arguments_and_explicit_environment_are_reported() {
    let gate = GateDefinition::new("environment", executable("printenv"))
        .with_environment("AGENTFORGE_GATE_VALUE", "configured");
    let report = GateRunner.run(&gate, ".").unwrap();
    assert_eq!(report.outcome(), GateOutcome::Passed);
    let output = String::from_utf8_lossy(report.stdout());
    assert!(output.contains("AGENTFORGE_GATE_VALUE=configured"));
    assert!(!output.contains("GIT_INDEX_FILE="));

    let literal = GateDefinition::new("literal", executable("printf"))
        .with_argument("%s")
        .with_argument("literal;$(not-a-shell)");
    let report = GateRunner.run(&literal, ".").unwrap();
    assert_eq!(report.stdout(), b"literal;$(not-a-shell)");

    let directory = GateDefinition::new("directory", executable("pwd"));
    let report = GateRunner.run(&directory, ".").unwrap();
    assert!(report.directory().is_absolute());
    assert_eq!(
        report.stdout(),
        format!("{}\n", report.directory().display()).as_bytes()
    );
}

#[test]
fn nonzero_timeout_and_duplicate_names_are_structured() {
    let runner = GateRunner;
    let failed = GateDefinition::new("failed", executable("false"));
    assert_eq!(
        runner.run(&failed, ".").unwrap().outcome(),
        GateOutcome::Failed
    );

    let timed_out = GateDefinition::new("timeout", executable("sleep"))
        .with_argument("30")
        .with_timeout(Duration::from_millis(30));
    assert_eq!(
        runner.run(&timed_out, ".").unwrap().outcome(),
        GateOutcome::TimedOut
    );

    let duplicate = GateDefinition::new("failed", executable("true"));
    assert!(matches!(
        runner.run_all(&[failed, duplicate], "."),
        Err(GateError::DuplicateName(name)) if name == "failed"
    ));
}

#[test]
fn output_limits_and_raw_non_utf8_evidence_are_preserved() {
    let flood = GateDefinition::new("flood", executable("yes")).with_max_output_bytes(64);
    let report = GateRunner.run(&flood, ".").unwrap();
    assert_eq!(report.outcome(), GateOutcome::OutputLimitExceeded);
    assert!(report.output_truncated());
    assert!(report.stdout().len() + report.stderr().len() <= 64);

    let bytes = GateDefinition::new("bytes", executable("printf")).with_argument("\\377");
    let report = GateRunner.run(&bytes, ".").unwrap();
    assert_eq!(report.stdout(), [0xff]);
}

#[test]
fn batches_preserve_input_order_and_invalid_definitions_fail() {
    let runner = GateRunner;
    let first = GateDefinition::new("first", executable("true"));
    let second = GateDefinition::new("second", executable("true"));
    let reports = runner.run_all(&[first, second], ".").unwrap();
    assert_eq!(
        reports
            .iter()
            .map(|report| report.name())
            .collect::<Vec<_>>(),
        ["first", "second"]
    );

    let relative = GateDefinition::new("relative", "true");
    assert!(matches!(
        runner.run(&relative, "."),
        Err(GateError::InvalidDefinition(_))
    ));
}
