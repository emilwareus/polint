// This is one rule-pack crate for the whole multiple-rules example repo.
// In a real project, this is the shape to prefer when several policies share
// one lifecycle: one Cargo.toml, one binary entry point, and one module per rule.
mod go_import_boundaries;
mod no_raw_colors;

use std::process::ExitCode;

fn main() -> ExitCode {
    polint::runner::run_cli(vec![
        no_raw_colors::no_raw_colors(),
        go_import_boundaries::go_import_boundaries(),
    ])
}
