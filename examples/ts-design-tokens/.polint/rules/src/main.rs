mod no_raw_colors;

use std::process::ExitCode;

fn main() -> ExitCode {
    polint::runner::run_cli(vec![no_raw_colors::no_raw_colors()])
}
