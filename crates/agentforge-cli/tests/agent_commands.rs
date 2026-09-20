#![allow(missing_docs)]

use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn agent_profile_commands_are_deterministic_and_non_mutating() {
    let root = temporary_root();
    let directory = root.join(".forge/agents");
    fs::create_dir_all(&directory).expect("profile directory");
    let executable = PathBuf::from(env!("CARGO_BIN_EXE_forge"));
    fs::write(
        directory.join("local.conf"),
        format!(
            "version=1\nexecutable={}\nargument=--version\nenv.AGENTFORGE_TEST=literal=value\ntimeout_ms=1000\nmax_output_bytes=4096\n",
            executable.display()
        ),
    )
    .expect("profile");

    let listed = forge(&root, &["agent", "list", root.to_str().unwrap()]);
    assert!(listed.status.success(), "{listed:?}");
    assert!(String::from_utf8_lossy(&listed.stdout).contains("id=local"));

    let validated = forge(
        &root,
        &["agent", "validate", root.to_str().unwrap(), "local"],
    );
    assert!(validated.status.success(), "{validated:?}");

    let inspected = forge(
        &root,
        &["agent", "inspect", root.to_str().unwrap(), "local"],
    );
    assert!(inspected.status.success(), "{inspected:?}");
    assert!(String::from_utf8_lossy(&inspected.stdout).contains("arguments=1"));

    fs::write(
        directory.join("broken.conf"),
        format!(
            "version=1\nversion=1\nexecutable={}\n",
            executable.display()
        ),
    )
    .expect("broken profile");
    let rejected = forge(
        &root,
        &["agent", "validate", root.to_str().unwrap(), "broken"],
    );
    assert!(!rejected.status.success(), "{rejected:?}");
    assert!(String::from_utf8_lossy(&rejected.stderr).contains("repeats version"));
    fs::remove_dir_all(root).expect("cleanup");
}

fn forge(root: &std::path::Path, arguments: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_forge"))
        .args(arguments)
        .current_dir(root)
        .output()
        .expect("forge command")
}

fn temporary_root() -> PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "agentforge-cli-agent-profile-{}-{suffix}",
        std::process::id()
    ));
    fs::create_dir(&root).expect("temporary root");
    root
}
