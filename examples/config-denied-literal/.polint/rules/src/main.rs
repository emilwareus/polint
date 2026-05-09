mod no_denied_literals;

use std::process::ExitCode;

fn main() -> ExitCode {
    polint::runner::run_cli(vec![no_denied_literals::no_denied_literals()])
}
