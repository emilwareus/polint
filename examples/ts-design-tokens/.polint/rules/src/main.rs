mod no_raw_colors;
use no_raw_colors::NoRawColors;

use std::process::ExitCode;
use std::sync::Arc;

fn main() -> ExitCode {
    polint::runner::run_cli(vec![Arc::new(NoRawColors)])
}
