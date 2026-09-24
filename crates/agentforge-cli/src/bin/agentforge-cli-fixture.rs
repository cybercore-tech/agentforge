//! Portable executable used by the daemon CLI integration test.

use std::fs;
use std::io::{self, Read, Write};

fn main() {
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
