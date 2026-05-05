mod go_branch_obligations;
use go_branch_obligations::GoBranchObligations;

use std::process::ExitCode;
use std::sync::Arc;

fn main() -> ExitCode {
    polint_runner::run_cli(vec![
        Arc::new(GoBranchObligations),
    ])
}
