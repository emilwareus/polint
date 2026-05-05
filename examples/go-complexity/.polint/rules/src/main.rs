mod go_complexity;
use go_complexity::GoComplexity;

use std::process::ExitCode;
use std::sync::Arc;

fn main() -> ExitCode {
    polint_runner::run_cli(vec![Arc::new(GoComplexity)])
}
