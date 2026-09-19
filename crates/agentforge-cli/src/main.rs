//! `forge` command-line entry point.

use std::process::ExitCode;

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);

    match args.next().as_deref() {
        Some("version" | "--version" | "-V") => {
            println!(
                "{} {}",
                agentforge_core::PRODUCT_NAME,
                agentforge_core::version()
            );
            ExitCode::SUCCESS
        }
        Some("doctor") => {
            println!("AgentForge doctor");
            println!("version: {}", agentforge_core::version());
            println!("rust: managed externally");
            println!("bootstrap: ok");
            ExitCode::SUCCESS
        }
        Some(other) => {
            eprintln!("unknown command: {other}");
            print_usage();
            ExitCode::from(2)
        }
        None => {
            print_usage();
            ExitCode::SUCCESS
        }
    }
}

fn print_usage() {
    println!("usage: forge <version|doctor>");
}
