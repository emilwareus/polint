# Sandbox Forbidden Paths Fixture

This fixture documents the Phase 51 adaptation-agent input boundary. Source/config
inputs may enter the adaptation sandbox; benchmark oracle files, expected-label
files, suite manifests with answers, Jelly oracles, and answer-key paths must stay
outside the prompt and sandbox.

The executable guard lives in `eval::adaptation::sandbox_forbidden_paths_are_filtered_from_agent_inputs`.
