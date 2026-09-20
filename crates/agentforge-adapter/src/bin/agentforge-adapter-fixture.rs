//! Deterministic local executable used by adapter integration tests.

use std::env;
use std::io::{self, Read, Write};
use std::thread;
use std::time::Duration;

fn main() {
    let mode = env::var("AGENTFORGE_FIXTURE_MODE").unwrap_or_else(|_| "echo".to_owned());
    match mode.as_str() {
        "echo" => {
            let mut input = Vec::new();
            io::stdin().read_to_end(&mut input).unwrap();
            println!("cwd={}", env::current_dir().unwrap().display());
            println!(
                "value={}",
                env::var("AGENTFORGE_TEST_VALUE").unwrap_or_default()
            );
            println!("argument={}", env::args().nth(1).unwrap_or_default());
            println!(
                "git_index={}",
                env::var("GIT_INDEX_FILE").unwrap_or_default()
            );
            io::stdout().write_all(&input).unwrap();
            eprintln!("fixture stderr");
        }
        "fail" => std::process::exit(23),
        "flood" => loop {
            println!("stdout flood");
            eprintln!("stderr flood");
        },
        "sleep" => thread::sleep(Duration::from_secs(30)),
        _ => std::process::exit(2),
    }
}
