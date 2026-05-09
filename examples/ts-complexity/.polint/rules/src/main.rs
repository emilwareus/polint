mod ts_complexity;

use std::process::ExitCode;

fn main() -> ExitCode {
    polint::runner::run_cli(vec![ts_complexity::ts_complexity()])
}
