mod gorm_model_read_indexes;

use std::process::ExitCode;

fn main() -> ExitCode {
    polint::runner::run_cli(vec![gorm_model_read_indexes::gorm_model_read_indexes()])
}
