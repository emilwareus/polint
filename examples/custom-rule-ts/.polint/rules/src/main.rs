mod no_product_hex_colors;
use no_product_hex_colors::NoProductHexColors;

use std::process::ExitCode;
use std::sync::Arc;

fn main() -> ExitCode {
    polint::runner::run_cli(vec![Arc::new(NoProductHexColors)])
}
