mod go_branch_obligations;

use std::process::ExitCode;

fn main() -> ExitCode {
    polint::runner::run_cli(vec![go_branch_obligations::go_branch_obligations()])
}
