mod go_test_quality;

use std::process::ExitCode;

fn main() -> ExitCode {
    polint::runner::run_cli(vec![go_test_quality::go_test_quality()])
}
