# Quick Task 260507-rap

Harden the rule-authoring platform after reviewing whether examples prove the
public SDK contract.

The review found that example rule code uses `polint::sdk::prelude::*` and
`polint::runner::run_cli`, but the proof was too workspace-coupled and the
public contract needed stronger external tests, arbitrary rule settings, more
honest capabilities documentation, fact docs, and AGENTS guidance.

