mod no_denied_literals;
use no_denied_literals::NoDeniedLiterals;

use std::process::ExitCode;
use std::sync::Arc;

fn main() -> ExitCode {
    polint_runner::run_cli(vec![Arc::new(NoDeniedLiterals)])
}
