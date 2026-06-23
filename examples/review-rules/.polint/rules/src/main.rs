// Review-rule example pack. Both rules are `kind = "review"`, so they fire only
// against the diff computed by `polint review <ref>` (they are inert under
// `polint check`). One is a simple path watcher; the other restricts real
// symbol/reference analysis to changed files.
mod migrations;
mod public_api_change;

use std::process::ExitCode;

fn main() -> ExitCode {
    polint::runner::run_cli(vec![
        migrations::migrations(),
        public_api_change::public_api_change(),
    ])
}
