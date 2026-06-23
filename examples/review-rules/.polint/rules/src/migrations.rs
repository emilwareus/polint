// The SIMPLE review rule: fire when a watched path changes.
//
// "Fire when this exact path changes" is ordinary Rust here — the rule reads
// the diff through the `ChangedFiles<'_>` fact view and checks each changed
// path against a glob. There is nothing in TOML; the policy is the code.
use polint::sdk::prelude::*;

#[polint::rule(
    id = "review/migrations",
    description = "Migrations changed — a database owner must review.",
    severity = "warn",
    kind = "review"
)]
pub(crate) fn migrations(ctx: &mut RuleCtx<'_>, changes: ChangedFiles<'_>) -> RuleResult {
    let rule_id = ctx.rule_id().to_string();
    for changed in changes.iter() {
        if !changed.matches_glob("db/migrations/**") {
            continue;
        }
        // Anchor the diagnostic on the first changed line so it lands inside the
        // diff under the default finding-level gate. (A path-watcher rule that
        // reports at a fixed line 1 would be gated out when line 1 is unchanged;
        // `lines()` gives the real new-side hunks. Deleted files have none, so
        // fall back to line 1.)
        let line = changed.lines().first().map(|&(lo, _)| lo).unwrap_or(1);
        ctx.report(
            Diagnostic::warning(
                rule_id.clone(),
                changed.path().to_string(),
                DiagnosticRange::point(line, 1),
                "Migration changed: a database owner must review this change.",
            )
            .with_help("Repo-local review policy: changes under db/migrations/** need a DB owner."),
        );
    }
    Ok(())
}
