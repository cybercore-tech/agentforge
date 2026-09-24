//! Portable executable used by the daemon CLI integration test.

use std::fs;
use std::io::{self, Read, Write};

fn main() {
    if let Ok(mode) = std::env::var("AGENTFORGE_CLI_FIXTURE_MODE") {
        if let Some(scenario) = mode.strip_prefix("ci-") {
            ci_provider(scenario);
            return;
        }
    }
    if std::env::var("AGENTFORGE_CLI_FIXTURE_MODE").as_deref() == Ok("sleep") {
        let millis: u64 = std::env::var("AGENTFORGE_FIXTURE_SLEEP_MS")
            .expect("AGENTFORGE_FIXTURE_SLEEP_MS")
            .parse()
            .expect("sleep milliseconds");
        std::thread::sleep(std::time::Duration::from_millis(millis));
        println!("fixture slept {millis} ms");
        return;
    }
    if std::env::var("AGENTFORGE_CLI_FIXTURE_MODE").as_deref() == Ok("rendezvous") {
        rendezvous();
        return;
    }
    if std::env::var("AGENTFORGE_CLI_FIXTURE_MODE").as_deref() == Ok("fail") {
        eprintln!("fixture failing by request");
        std::process::exit(3);
    }
    if std::env::var("AGENTFORGE_CLI_FIXTURE_MODE").as_deref() == Ok("interactive") {
        let mut input = Vec::new();
        io::stdin()
            .read_to_end(&mut input)
            .expect("read fixture input");
        io::stdout()
            .write_all(b"fixture interactive start\n")
            .expect("write fixture output");
        io::stdout().write_all(&input).expect("write fixture input");
        io::stdout().flush().expect("flush fixture output");
        return;
    }
    fs::write("agentforge-fixture-output.txt", "fixture executed\n").expect("write fixture output");
    println!("fixture executed");
}

/// Emits `agentforge-ci-v1` protocol for the requested SHA, as a CI provider would.
fn ci_provider(scenario: &str) {
    let sha = std::env::var("AGENTFORGE_CI_SHA").expect("AGENTFORGE_CI_SHA");
    let other = "0".repeat(40);
    println!("agentforge-ci-v1");
    match scenario {
        "success" => {
            println!("run\t101\t{other}\tcompleted\tfailure");
            println!("run\t102\t{sha}\tcompleted\tsuccess");
            println!("job\t102\t1\tStable code gate\tcompleted\tsuccess\t");
        }
        "failure" => {
            println!("run\t201\t{sha}\tcompleted\tfailure");
            println!("job\t201\t1\tRepository policy\tcompleted\tsuccess\t");
            println!(
                "job\t201\t2\tStable code gate\tcompleted\tfailure\tthread 'x' panicked at src/lib.rs:1:1"
            );
            println!("job\t201\t3\tMSRV\tcompleted\tfailure\terror[E0308]: mismatched types");
        }
        "pending" => {
            println!("run\t301\t{sha}\tin_progress\t-");
            println!("job\t301\t1\tStable code gate\tin_progress\t-\t");
        }
        "ambiguous" => {
            println!("run\t401\t{sha}\tcompleted\tsuccess");
            println!("run\t402\t{sha}\tcompleted\tfailure");
        }
        _ => {
            println!("run\t501\t{other}\tcompleted\tsuccess");
        }
    }
}

/// Registers this process in a shared directory, then waits (bounded) until the expected number
/// of fixture agents have registered. Serial execution can never satisfy the wait.
fn rendezvous() {
    let directory = std::path::PathBuf::from(
        std::env::var("AGENTFORGE_RENDEZVOUS_DIR").expect("AGENTFORGE_RENDEZVOUS_DIR"),
    );
    let expected: usize = std::env::var("AGENTFORGE_RENDEZVOUS_COUNT")
        .expect("AGENTFORGE_RENDEZVOUS_COUNT")
        .parse()
        .expect("rendezvous count");
    fs::write(
        directory.join(format!("arrived-{}", std::process::id())),
        b"",
    )
    .expect("register arrival");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    loop {
        let arrived = fs::read_dir(&directory)
            .expect("rendezvous directory")
            .filter(|entry| {
                entry
                    .as_ref()
                    .is_ok_and(|entry| entry.file_name().to_string_lossy().starts_with("arrived-"))
            })
            .count();
        if arrived >= expected {
            println!("rendezvous complete");
            return;
        }
        if std::time::Instant::now() >= deadline {
            eprintln!("rendezvous timed out with {arrived}/{expected} agents");
            std::process::exit(4);
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}
