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
