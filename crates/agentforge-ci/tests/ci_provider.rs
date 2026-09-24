//! Project-local CI provider profile loading.

use agentforge_ci::{
    CI_PROVIDER_RELATIVE_PATH, CiConclusion, CiProviderError, CiProviderStore, CiStatus,
    FailureCategory,
};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static TEMPORARY_ROOT_COUNTER: AtomicU64 = AtomicU64::new(0);

fn temporary_root() -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "agentforge-ci-provider-{}-{stamp}-{}",
        std::process::id(),
        TEMPORARY_ROOT_COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir_all(root.join(".forge/ci")).expect("provider directory");
    root
}

fn executable() -> String {
    std::env::current_exe()
        .expect("test executable")
        .to_string_lossy()
        .into_owned()
}

fn write_provider(root: &Path, body: &str) {
    fs::write(root.join(CI_PROVIDER_RELATIVE_PATH), body).expect("provider profile");
}

#[test]
fn documented_provider_profile_loads() {
    let root = temporary_root();
    write_provider(
        &root,
        &format!(
            "# reviewed provider\nversion=1\nexecutable={}\nargument=--json\nenv.GH_TOKEN_FILE=/run/token\ntimeout_ms=30000\nmax_output_bytes=65536\n",
            executable()
        ),
    );
    assert!(CiProviderStore::new(&root).load().is_ok());
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn missing_provider_is_reported_distinctly() {
    let root = temporary_root();
    assert!(matches!(
        CiProviderStore::new(&root).load(),
        Err(CiProviderError::Missing(_))
    ));
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn malformed_provider_profiles_fail_closed() {
    let root = temporary_root();
    let exe = executable();
    let cases = [
        (format!("executable={exe}\n"), "version=1"),
        ("version=1\n".to_owned(), "missing executable"),
        ("version=1\nexecutable=relative\n".to_owned(), "absolute"),
        (
            format!("version=1\nexecutable={exe}\nshell=sh\n"),
            "unknown key",
        ),
        (
            format!("version=1\nexecutable={exe}\nexecutable={exe}\n"),
            "repeats executable",
        ),
        (
            format!("version=1\nexecutable={exe}\ntimeout_ms=0\n"),
            "timeout_ms",
        ),
        (
            format!("version=1\nexecutable={exe}\ntimeout_ms=3600001\n"),
            "timeout_ms",
        ),
        (
            format!("version=1\nexecutable={exe}\nenv.AGENTFORGE_CI_SHA=forged\n"),
            "reserved",
        ),
        (
            format!("version=1\nexecutable={exe}\nenv.A=1\nenv.A=2\n"),
            "environment key",
        ),
    ];
    for (body, expected) in cases {
        write_provider(&root, &body);
        match CiProviderStore::new(&root).load() {
            Err(CiProviderError::Invalid(reason)) => {
                assert!(reason.contains(expected), "{body:?}: {reason}");
            }
            other => panic!("{body:?}: expected invalid profile, got {other:?}"),
        }
    }
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn oversized_profile_is_rejected() {
    let root = temporary_root();
    write_provider(
        &root,
        &format!(
            "version=1\nexecutable={}\n{}",
            executable(),
            "# padding\n".repeat(8 * 1024)
        ),
    );
    assert!(matches!(
        CiProviderStore::new(&root).load(),
        Err(CiProviderError::Invalid(reason)) if reason.contains("exceeds")
    ));
    fs::remove_dir_all(root).expect("cleanup");
}

#[cfg(unix)]
#[test]
fn symlinked_profile_is_rejected() {
    let root = temporary_root();
    let target = root.join("elsewhere.conf");
    fs::write(&target, format!("version=1\nexecutable={}\n", executable())).expect("target");
    std::os::unix::fs::symlink(&target, root.join(CI_PROVIDER_RELATIVE_PATH)).expect("symlink");
    assert!(matches!(
        CiProviderStore::new(&root).load(),
        Err(CiProviderError::Invalid(reason)) if reason.contains("regular file")
    ));
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn labels_are_stable() {
    assert_eq!(CiStatus::InProgress.as_str(), "in_progress");
    assert_eq!(CiConclusion::TimedOut.as_str(), "timed_out");
    assert_eq!(FailureCategory::SemanticTest.as_str(), "semantic_test");
    assert_eq!(
        FailureCategory::DocumentationTextPolicy.as_str(),
        "documentation_text_policy"
    );
}
