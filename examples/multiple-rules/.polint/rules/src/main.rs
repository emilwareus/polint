// This is one rule-pack crate for the whole multiple-rules example repo.
// In a real project, this is the shape to prefer when several policies share
// one lifecycle: one Cargo.toml, one binary entry point, and one module per rule.
mod glob;
mod go_import_boundaries;
mod no_raw_colors;

use go_import_boundaries::GoImportBoundaries;
use no_raw_colors::NoRawColors;
use std::process::ExitCode;
use std::sync::Arc;

fn main() -> ExitCode {
    polint_runner::run_cli(vec![Arc::new(NoRawColors), Arc::new(GoImportBoundaries)])
}
