//! Project-local gate profile loading.

use agentforge_gate::{GATE_PROFILE_RELATIVE_PATH, GateProfileError, GateProfileStore};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

static TEMPORARY_ROOT_COUNTER: AtomicU64 = AtomicU64::new(0);

fn temporary_root() -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "agentforge-gate-profiles-{}-{stamp}-{}",
        std::process::id(),
        TEMPORARY_ROOT_COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir_all(root.join(GATE_PROFILE_RELATIVE_PATH)).expect("gate directory");
    root
}

fn executable() -> String {
    std::env::current_exe()
        .expect("test executable")
        .to_string_lossy()
        .into_owned()
}

fn write_gate(root: &std::path::Path, id: &str, body: &str) {
    fs::write(
        root.join(GATE_PROFILE_RELATIVE_PATH)
            .join(format!("{id}.conf")),
        body,
    )
    .expect("gate profile");
}

fn invalid(result: Result<impl std::fmt::Debug, GateProfileError>) -> String {
    match result {
        Err(GateProfileError::Invalid(reason)) => reason,
        other => panic!("expected invalid gate profile, got {other:?}"),
    }
}

#[test]
fn documented_keys_load_into_a_definition() {
    let root = temporary_root();
    write_gate(
        &root,
        "unit-tests",
        &format!(
            "# reviewed gate\nversion=1\nexecutable={}\nargument=--quiet\nargument=--locked\nenv.PATH=/usr/bin\ntimeout_ms=1500\nmax_output_bytes=4096\n",
            executable()
        ),
    );
    let gate = GateProfileStore::new(&root)
        .load("unit-tests")
        .expect("valid gate");
    assert_eq!(gate.name(), "unit-tests");
    assert_eq!(gate.arguments(), ["--quiet", "--locked"]);
    assert_eq!(
        gate.environment().get("PATH").map(String::as_str),
        Some("/usr/bin")
    );
    assert_eq!(gate.timeout(), Duration::from_millis(1500));
    assert_eq!(gate.max_output_bytes(), 4096);
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn defaults_apply_when_limits_are_omitted() {
    let root = temporary_root();
    write_gate(
        &root,
        "lint",
        &format!("version=1\nexecutable={}\n", executable()),
    );
    let gate = GateProfileStore::new(&root).load("lint").expect("gate");
    assert_eq!(gate.timeout(), Duration::from_secs(60));
    assert_eq!(gate.max_output_bytes(), 1024 * 1024);
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn list_is_lexical_and_ignores_other_files() {
    let root = temporary_root();
    for id in ["zeta", "alpha", "mid_1"] {
        write_gate(
            &root,
            id,
            &format!("version=1\nexecutable={}\n", executable()),
        );
    }
    fs::write(
        root.join(GATE_PROFILE_RELATIVE_PATH).join("README.md"),
        "notes\n",
    )
    .expect("unrelated file");
    let names = GateProfileStore::new(&root)
        .list()
        .expect("list")
        .iter()
        .map(|gate| gate.name().to_owned())
        .collect::<Vec<_>>();
    assert_eq!(names, ["alpha", "mid_1", "zeta"]);
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn missing_directory_declares_no_gates() {
    let root = temporary_root();
    fs::remove_dir(root.join(GATE_PROFILE_RELATIVE_PATH)).expect("remove gate directory");
    assert!(
        GateProfileStore::new(&root)
            .list()
            .expect("list")
            .is_empty()
    );
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn malformed_profiles_fail_closed() {
    let root = temporary_root();
    let exe = executable();
    let cases = [
        ("no-version", format!("executable={exe}\n"), "version=1"),
        (
            "bad-version",
            format!("version=2\nexecutable={exe}\n"),
            "version=1",
        ),
        (
            "no-executable",
            "version=1\n".to_owned(),
            "missing executable",
        ),
        (
            "relative",
            "version=1\nexecutable=bin/check\n".to_owned(),
            "absolute",
        ),
        (
            "unknown-key",
            format!("version=1\nexecutable={exe}\nshell=bash\n"),
            "unknown key",
        ),
        (
            "repeat",
            format!("version=1\nexecutable={exe}\nexecutable={exe}\n"),
            "repeats executable",
        ),
        (
            "no-equals",
            format!("version=1\nexecutable={exe}\nargument\n"),
            "key=value",
        ),
        (
            "zero-timeout",
            format!("version=1\nexecutable={exe}\ntimeout_ms=0\n"),
            "timeout_ms",
        ),
        (
            "huge-output",
            format!("version=1\nexecutable={exe}\nmax_output_bytes=999999999999\n"),
            "max_output_bytes",
        ),
        (
            "env-repeat",
            format!("version=1\nexecutable={exe}\nenv.A=1\nenv.A=2\n"),
            "environment key",
        ),
    ];
    let store = GateProfileStore::new(&root);
    for (id, body, expected) in cases {
        write_gate(&root, id, &body);
        let reason = invalid(store.load(id));
        assert!(reason.contains(expected), "{id}: {reason}");
    }
    // One malformed profile makes the whole listing fail closed.
    assert!(store.list().is_err());
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn unsafe_ids_and_oversized_profiles_are_rejected() {
    let root = temporary_root();
    let store = GateProfileStore::new(&root);
    for id in ["", "..", "a/b", "dot.name", &"x".repeat(65)] {
        assert!(invalid(store.load(id)).contains("gate ID"), "{id:?}");
    }
    write_gate(
        &root,
        "big",
        &format!(
            "version=1\nexecutable={}\n{}",
            executable(),
            "# padding\n".repeat(8 * 1024)
        ),
    );
    assert!(invalid(store.load("big")).contains("exceeds"));
    assert!(invalid(store.load("absent")).contains("not found"));
    fs::remove_dir_all(root).expect("cleanup");
}

#[cfg(unix)]
#[test]
fn symlinked_profiles_are_rejected() {
    let root = temporary_root();
    let target = root.join("elsewhere.conf");
    fs::write(&target, format!("version=1\nexecutable={}\n", executable())).expect("target");
    std::os::unix::fs::symlink(
        &target,
        root.join(GATE_PROFILE_RELATIVE_PATH).join("linked.conf"),
    )
    .expect("symlink");
    assert!(invalid(GateProfileStore::new(&root).load("linked")).contains("not a regular file"));
    fs::remove_dir_all(root).expect("cleanup");
}
