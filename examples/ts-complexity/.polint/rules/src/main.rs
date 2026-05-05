mod ts_complexity;
use ts_complexity::TsComplexity;

use std::process::ExitCode;
use std::sync::Arc;

fn main() -> ExitCode {
    polint_runner::run_cli(vec![Arc::new(TsComplexity)])
}
