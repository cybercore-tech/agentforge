//! Integration coverage for exact-SHA CI observation and classification.

use agentforge_ci::{
    CiConclusion, CiMonitor, CiObservationError, CiObservationRequest, CiStatus, CommandCiMonitor,
    CommandCiMonitorConfig, FailureCategory, FailureClassifier,
};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

const SHA: &str = "0123456789abcdef0123456789abcdef01234567";
static NEXT: AtomicU64 = AtomicU64::new(0);

fn executable(name: &str) -> PathBuf {
    PathBuf::from(format!("/usr/bin/{name}"))
}

fn directory() -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "agentforge-ci-test-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&path).unwrap();
    path
}

fn observe(payload: &str) -> Result<agentforge_ci::CiRun, CiObservationError> {
    let dir = directory();
    let config = CommandCiMonitorConfig::new(executable("printf"))
        .with_argument("%s")
        .with_argument(payload)
        .with_environment("AGENTFORGE_CI_VALUE", "explicit")
        .with_timeout(Duration::from_secs(1));
    let request = CiObservationRequest::new("owner/repo", "ci.yml", SHA);
    let result = CommandCiMonitor::new(config).observe_exact(&request, &dir);
    fs::remove_dir(&dir).unwrap();
    result
}

fn protocol(runs: &str, jobs: &str) -> String {
    format!("agentforge-ci-v1\n{runs}{jobs}")
}

#[test]
fn exact_sha_selection_preserves_order_and_terminal_state() {
    let output = protocol(
        &format!(
            "run\told\t1111111111111111111111111111111111111111\tcompleted\tsuccess\nrun\ttarget\t{SHA}\tcompleted\tfailure\n"
        ),
        "job\ttarget\tjob-1\tstable\tcompleted\tfailure\ttest failed\njob\ttarget\tjob-2\tpolicy\tcompleted\tfailure\ttext policy\n",
    );
    let run = observe(&output).unwrap();
    assert_eq!(run.provider_id(), "target");
    assert_eq!(run.status(), CiStatus::Completed);
    assert_eq!(run.conclusion(), Some(CiConclusion::Failure));
    assert_eq!(run.jobs()[0].name(), "stable");
    assert_eq!(run.jobs()[1].name(), "policy");
    assert_eq!(
        FailureClassifier::classify(&run.jobs()[0]).category(),
        FailureCategory::SemanticTest
    );
}

#[test]
fn stale_absent_and_ambiguous_sha_evidence_is_rejected() {
    let stale = protocol(
        "run\told\t1111111111111111111111111111111111111111\tcompleted\tsuccess\n",
        "",
    );
    assert!(matches!(
        observe(&stale),
        Err(CiObservationError::ExactRunNotFound)
    ));
    let absent = protocol(
        "run\tother\t1111111111111111111111111111111111111111\tcompleted\tsuccess\n",
        "",
    );
    assert!(matches!(
        observe(&absent),
        Err(CiObservationError::ExactRunNotFound)
    ));
    let ambiguous = protocol(
        &format!("run\ta\t{SHA}\tcompleted\tsuccess\nrun\tb\t{SHA}\tcompleted\tsuccess\n"),
        "",
    );
    assert!(matches!(
        observe(&ambiguous),
        Err(CiObservationError::AmbiguousExactRun)
    ));
}

#[test]
fn nonterminal_evidence_and_protocol_errors_are_controlled() {
    let queued = protocol(&format!("run\tqueued\t{SHA}\tqueued\t-\n"), "");
    let run = observe(&queued).unwrap();
    assert_eq!(run.status(), CiStatus::Queued);
    assert_eq!(run.conclusion(), None);
    assert!(matches!(
        observe("wrong-header\n"),
        Err(CiObservationError::InvalidProtocol(_))
    ));
    let invalid = format!("agentforge-ci-v1\nrun\tbad\t{SHA}\tcompleted\tunknown\n");
    assert!(matches!(
        observe(&invalid),
        Err(CiObservationError::InvalidProtocol(_))
    ));
}

#[test]
fn classifier_precedence_covers_taxonomy_and_unknown() {
    let jobs = [
        (
            "generated",
            "checksum mismatch and test failed",
            FailureCategory::GeneratedContentCorruption,
        ),
        (
            "docs",
            "text policy rejected",
            FailureCategory::DocumentationTextPolicy,
        ),
        (
            "workflow",
            "active plan validation failed",
            FailureCategory::WorkflowGovernance,
        ),
        (
            "lint",
            "clippy reported an error",
            FailureCategory::FormattingLint,
        ),
        (
            "compile",
            "could not compile crate",
            FailureCategory::CompilationType,
        ),
        (
            "dependency",
            "failed to download dependency",
            FailureCategory::DependencyToolchain,
        ),
        (
            "infra",
            "runner unavailable",
            FailureCategory::Infrastructure,
        ),
        (
            "unknown",
            "failure without a known marker",
            FailureCategory::Unknown,
        ),
    ];
    let mut records = String::new();
    for (index, (name, excerpt, _)) in jobs.iter().enumerate() {
        records.push_str(&format!(
            "job\ttarget\tjob-{index}\t{name}\tcompleted\tfailure\t{excerpt}\n"
        ));
    }
    let run = observe(&protocol(
        &format!("run\ttarget\t{SHA}\tcompleted\tfailure\n"),
        &records,
    ))
    .unwrap();
    for (job, (_, _, expected)) in run.jobs().iter().zip(jobs) {
        assert_eq!(FailureClassifier::classify(job).category(), expected);
    }
}

#[test]
fn invalid_configuration_and_output_limits_fail_without_guessing() {
    let dir = directory();
    let request = CiObservationRequest::new("owner/repo", "ci.yml", SHA);
    let config = CommandCiMonitorConfig::new("printf");
    assert!(matches!(
        CommandCiMonitor::new(config).observe_exact(&request, &dir),
        Err(CiObservationError::InvalidConfiguration(_))
    ));
    fs::remove_dir(&dir).unwrap();
    let dir = directory();
    let config = CommandCiMonitorConfig::new(executable("yes")).with_max_output_bytes(64);
    assert!(matches!(
        CommandCiMonitor::new(config).observe_exact(&request, &dir),
        Err(CiObservationError::OutputLimitExceeded)
    ));
    fs::remove_dir(&dir).unwrap();

    let dir = directory();
    let config = CommandCiMonitorConfig::new(executable("printf")).with_argument("\\377");
    assert!(matches!(
        CommandCiMonitor::new(config).observe_exact(&request, &dir),
        Err(CiObservationError::InvalidUtf8)
    ));
    fs::remove_dir(&dir).unwrap();
}
