mod go_import_boundaries;

use std::process::ExitCode;

fn main() -> ExitCode {
    polint::runner::run_cli(vec![go_import_boundaries::go_import_boundaries()])
}
