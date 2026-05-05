mod go_import_boundaries;
use go_import_boundaries::GoImportBoundaries;

use std::process::ExitCode;
use std::sync::Arc;

fn main() -> ExitCode {
    polint::runner::run_cli(vec![Arc::new(GoImportBoundaries)])
}
