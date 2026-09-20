//! Portable executable used by the daemon CLI integration test.

use std::fs;

fn main() {
    fs::write("agentforge-fixture-output.txt", "fixture executed\n").expect("write fixture output");
    println!("fixture executed");
}
