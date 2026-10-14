//! The test ownership check (docs/test-ownership.md), run from the repository root by
//!   cargo run --locked --manifest-path tests/test-ownership/Cargo.toml -- <repository root>

use std::env;
use std::io;
use std::path::PathBuf;
use std::process::ExitCode;

use polyspec_ssr_test_ownership::{Cargo, cargo_case, check};

fn main() -> ExitCode {
    let mut arguments = env::args_os().skip(1);
    let (Some(root), None) = (arguments.next(), arguments.next()) else {
        eprintln!("usage: polyspec-ssr-test-ownership <repository root>");
        return ExitCode::from(2);
    };
    if let Err(error) = cargo_case::install_signal_handler() {
        eprintln!("FAIL test ownership: cannot handle signals: {error}");
        return ExitCode::FAILURE;
    }
    match check(&PathBuf::from(root), &Cargo::default(), &mut io::stdout()) {
        Ok(count) => {
            println!("PASS test ownership: {count} tests");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("FAIL test ownership: {error}");
            match cargo_case::interrupted().and_then(|signal| u8::try_from(128 + signal).ok()) {
                Some(code) => ExitCode::from(code),
                None => ExitCode::FAILURE,
            }
        }
    }
}
