mod code_quality_score;
mod large_files;
mod large_functions;

use std::process::ExitCode;

fn main() -> ExitCode {
    polint::runner::run_cli(vec![
        large_files::large_file(),
        large_functions::large_function(),
        code_quality_score::code_quality_score(),
    ])
}
