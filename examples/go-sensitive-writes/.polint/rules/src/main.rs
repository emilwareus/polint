mod no_sensitive_balance_writes;

use std::process::ExitCode;

fn main() -> ExitCode {
    polint::runner::run_cli(vec![
        no_sensitive_balance_writes::no_sensitive_balance_writes(),
    ])
}
