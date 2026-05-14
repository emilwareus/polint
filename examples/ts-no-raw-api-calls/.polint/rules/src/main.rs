mod no_raw_api_calls;

use std::process::ExitCode;

fn main() -> ExitCode {
    polint::runner::run_cli(vec![no_raw_api_calls::no_raw_api_calls()])
}
