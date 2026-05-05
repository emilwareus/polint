mod go_test_quality;
use go_test_quality::GoTestQuality;

use std::process::ExitCode;
use std::sync::Arc;

fn main() -> ExitCode {
    polint_runner::run_cli(vec![
        Arc::new(GoTestQuality),
    ])
}
