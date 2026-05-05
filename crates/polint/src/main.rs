use std::process::ExitCode;

fn main() -> ExitCode {
    match polint::run_main() {
        Ok(code) => ExitCode::from(code),
        Err(error) => {
            eprintln!("polint: {error:?}");
            ExitCode::from(2)
        }
    }
}
