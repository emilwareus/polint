mod go_complexity;

use std::process::ExitCode;

fn main() -> ExitCode {
    polint::runner::run_cli(vec![go_complexity::go_complexity()])
}
