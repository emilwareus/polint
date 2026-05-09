mod no_product_hex_colors;

use std::process::ExitCode;

fn main() -> ExitCode {
    polint::runner::run_cli(vec![no_product_hex_colors::no_product_hex_colors()])
}
