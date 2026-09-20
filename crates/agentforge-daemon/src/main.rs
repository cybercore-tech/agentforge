//! `forged` daemon entry point.

use agentforge_daemon::{DEFAULT_BIND, serve};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut arguments = std::env::args().skip(1);
    match arguments.next().as_deref() {
        Some("--version" | "-V" | "version") => {
            println!(
                "{} daemon {}",
                agentforge_core::PRODUCT_NAME,
                agentforge_core::version()
            );
            ExitCode::SUCCESS
        }
        Some("serve") => serve_command(arguments.collect()),
        Some(other) => {
            eprintln!("unknown forged command: {other}");
            print_usage();
            ExitCode::from(2)
        }
        None => {
            print_usage();
            ExitCode::from(2)
        }
    }
}

fn print_usage() {
    println!("usage: forged <version|serve --root <root> [--bind 127.0.0.1:0]>");
}

fn serve_command(arguments: Vec<String>) -> ExitCode {
    let mut root = None;
    let mut bind = DEFAULT_BIND;
    let mut index = 0;
    while index < arguments.len() {
        let option = &arguments[index];
        let Some(value) = arguments.get(index + 1) else {
            eprintln!("{option} requires a value");
            return ExitCode::from(2);
        };
        match option.as_str() {
            "--root" => root = Some(PathBuf::from(value)),
            "--bind" => match value.parse::<SocketAddr>() {
                Ok(address) => bind = address,
                Err(error) => {
                    eprintln!("invalid bind address: {error}");
                    return ExitCode::from(2);
                }
            },
            _ => {
                eprintln!("unknown serve option: {option}");
                return ExitCode::from(2);
            }
        }
        index += 2;
    }
    let Some(root) = root else {
        eprintln!("serve requires --root <root>");
        return ExitCode::from(2);
    };
    match serve(root, bind) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("forged failed: {error}");
            ExitCode::from(1)
        }
    }
}
