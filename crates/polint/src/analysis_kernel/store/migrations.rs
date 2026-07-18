//! Strict, private schema migrations for the durable semantic store.

use std::sync::OnceLock;

#[cfg(test)]
use rusqlite::TransactionBehavior;
use rusqlite::{Connection, ErrorCode, Transaction};
#[cfg(test)]
use std::cell::Cell;
use thiserror::Error;

use super::schema::REQUIRED_V2_TABLES;

pub(super) const CURRENT_SCHEMA_VERSION: i32 = 2;

#[cfg(test)]
thread_local! {
    static FULL_FOREIGN_KEY_CHECK_RUNS: Cell<u64> = const { Cell::new(0) };
    static FULL_HISTORY_LIFECYCLE_CHECK_RUNS: Cell<u64> = const { Cell::new(0) };
}

const BOOTSTRAP_TABLE: &str = "_polint_schema_migrations";
const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        stages: &[V1_BOOTSTRAP_SQL],
    },
    Migration {
        version: 2,
        stages: &[V2_TABLES_SQL, V2_INDEXES_AND_TRIGGERS_SQL, V2_MARKER_SQL],
    },
];

struct Migration {
    version: i32,
    stages: &'static [&'static str],
}

const V1_BOOTSTRAP_SQL: &str = "CREATE TABLE _polint_schema_migrations (\
                                   version INTEGER PRIMARY KEY CHECK (version > 0)\
                               );\
                               INSERT INTO _polint_schema_migrations (version) VALUES (1);";

const V2_TABLES_SQL: &str = r#"
CREATE TABLE store_manifest (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    workspace_kind TEXT,
    workspace_value TEXT,
    active_generation_id INTEGER,
    CHECK ((workspace_kind IS NULL) = (workspace_value IS NULL)),
    CHECK (workspace_kind IS NULL OR workspace_kind = 'workspace'),
    CHECK (workspace_value IS NULL OR workspace_value <> ''),
    CHECK (workspace_kind IS NOT NULL OR active_generation_id IS NULL),
    FOREIGN KEY (active_generation_id) REFERENCES generations(id)
        DEFERRABLE INITIALLY DEFERRED
);

CREATE TABLE generations (
    id INTEGER PRIMARY KEY,
    reservation_ordinal INTEGER NOT NULL CHECK (reservation_ordinal >= 0),
    workspace_kind TEXT NOT NULL CHECK (workspace_kind = 'workspace'),
    workspace_value TEXT NOT NULL CHECK (workspace_value <> ''),
    generation_kind TEXT NOT NULL CHECK (generation_kind = 'generation'),
    generation_value TEXT NOT NULL CHECK (generation_value <> ''),
    run_kind TEXT NOT NULL CHECK (run_kind = 'run'),
    run_value TEXT NOT NULL CHECK (run_value <> ''),
    full_config_kind TEXT NOT NULL CHECK (full_config_kind = 'config'),
    full_config_value TEXT NOT NULL CHECK (full_config_value <> ''),
    input_snapshot_kind TEXT NOT NULL CHECK (input_snapshot_kind = 'input_snapshot'),
    input_snapshot_value TEXT NOT NULL CHECK (input_snapshot_value <> ''),
    provider_manifest_kind TEXT NOT NULL CHECK (provider_manifest_kind = 'provider_manifest'),
    provider_manifest_value TEXT NOT NULL CHECK (provider_manifest_value <> ''),
    provider_output_kind TEXT NOT NULL CHECK (provider_output_kind = 'provider_output'),
    provider_output_value TEXT NOT NULL CHECK (provider_output_value <> ''),
    layer_kind TEXT NOT NULL CHECK (layer_kind = 'layer'),
    layer_value TEXT NOT NULL CHECK (layer_value <> ''),
    summary_kind TEXT NOT NULL CHECK (summary_kind = 'summary'),
    summary_value TEXT NOT NULL CHECK (summary_value <> ''),
    query_kind TEXT NOT NULL CHECK (query_kind = 'query'),
    query_value TEXT NOT NULL CHECK (query_value <> ''),
    fact_kind TEXT NOT NULL CHECK (fact_kind = 'fact_metadata'),
    fact_value TEXT NOT NULL CHECK (fact_value <> ''),
    dependency_kind TEXT NOT NULL CHECK (dependency_kind = 'dependency'),
    dependency_value TEXT NOT NULL CHECK (dependency_value <> ''),
    validation_kind TEXT NOT NULL CHECK (validation_kind = 'validation_event'),
    validation_value TEXT NOT NULL CHECK (validation_value <> ''),
    dependency_schema TEXT NOT NULL CHECK (dependency_schema = 'polint-dependency-index-2'),
    status TEXT NOT NULL CHECK (status IN ('pending', 'complete', 'failed')),
    UNIQUE (workspace_kind, workspace_value, generation_kind, generation_value, reservation_ordinal)
);

CREATE TABLE run_manifest_nodes (
    id INTEGER PRIMARY KEY,
    generation_id INTEGER NOT NULL UNIQUE,
    run_kind TEXT NOT NULL CHECK (run_kind = 'run'),
    run_value TEXT NOT NULL CHECK (run_value <> ''),
    full_config_kind TEXT NOT NULL CHECK (full_config_kind = 'config'),
    full_config_value TEXT NOT NULL CHECK (full_config_value <> ''),
    UNIQUE (generation_id, id),
    FOREIGN KEY (generation_id) REFERENCES generations(id) ON DELETE CASCADE
);

INSERT INTO store_manifest (id, workspace_kind, workspace_value, active_generation_id)
VALUES (1, NULL, NULL, NULL);

CREATE TABLE input_snapshots (
    generation_id INTEGER PRIMARY KEY,
    schema_version TEXT NOT NULL CHECK (schema_version = 'polint-input-snapshot-2'),
    workspace_kind TEXT NOT NULL CHECK (workspace_kind = 'workspace'),
    workspace_value TEXT NOT NULL CHECK (workspace_value <> ''),
    full_config_kind TEXT NOT NULL CHECK (full_config_kind = 'config'),
    full_config_value TEXT NOT NULL CHECK (full_config_value <> ''),
    input_digest_kind TEXT NOT NULL CHECK (input_digest_kind = 'input_snapshot'),
    input_digest_value TEXT NOT NULL CHECK (input_digest_value <> ''),
    requirements_digest_kind TEXT NOT NULL CHECK (requirements_digest_kind = 'analysis_requirements'),
    requirements_digest_value TEXT NOT NULL CHECK (requirements_digest_value <> ''),
    FOREIGN KEY (generation_id) REFERENCES generations(id) ON DELETE CASCADE
);

CREATE TABLE input_files (
    generation_id INTEGER NOT NULL,
    relative_path TEXT NOT NULL,
    language TEXT NOT NULL CHECK (language <> ''),
    source_digest_kind TEXT NOT NULL CHECK (source_digest_kind = 'source_text'),
    source_digest_value TEXT NOT NULL CHECK (source_digest_value <> ''),
    size_bytes INTEGER NOT NULL CHECK (size_bytes >= 0),
    PRIMARY KEY (generation_id, relative_path),
    CHECK (relative_path <> ''),
    CHECK (substr(relative_path, 1, 1) <> '/'),
    CHECK (relative_path NOT GLOB '[A-Za-z]:*'),
    CHECK (instr(relative_path, '\') = 0),
    CHECK (relative_path <> '.' AND relative_path <> '..'),
    CHECK (relative_path NOT LIKE './%' AND relative_path NOT LIKE '../%'),
    CHECK (relative_path NOT LIKE '%/./%' AND relative_path NOT LIKE '%/../%'),
    CHECK (relative_path NOT LIKE '%/.' AND relative_path NOT LIKE '%/..'),
    FOREIGN KEY (generation_id) REFERENCES generations(id) ON DELETE CASCADE
);

CREATE TABLE input_components (
    generation_id INTEGER NOT NULL,
    component_group TEXT NOT NULL CHECK (
        component_group IN ('config', 'go_lifecycle', 'ts_js_lifecycle', 'rule', 'model', 'extension', 'tool_invocation')
    ),
    name TEXT NOT NULL CHECK (name <> ''),
    status TEXT NOT NULL CHECK (status <> ''),
    digest_kind TEXT NOT NULL CHECK (digest_kind <> ''),
    digest_value TEXT NOT NULL CHECK (digest_value <> ''),
    detail_count INTEGER NOT NULL CHECK (detail_count >= 0),
    PRIMARY KEY (generation_id, component_group, name),
    FOREIGN KEY (generation_id) REFERENCES generations(id) ON DELETE CASCADE
);

CREATE TABLE input_component_details (
    generation_id INTEGER NOT NULL,
    component_group TEXT NOT NULL,
    component_name TEXT NOT NULL,
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    component_digest_kind TEXT NOT NULL CHECK (component_digest_kind <> ''),
    component_digest_value TEXT NOT NULL CHECK (component_digest_value <> ''),
    detail TEXT NOT NULL,
    PRIMARY KEY (generation_id, component_group, component_name, ordinal),
    FOREIGN KEY (generation_id, component_group, component_name)
        REFERENCES input_components(generation_id, component_group, name) ON DELETE CASCADE
);

CREATE TABLE analysis_settings (
    generation_id INTEGER NOT NULL,
    scope TEXT NOT NULL CHECK (scope <> ''),
    digest_kind TEXT NOT NULL CHECK (digest_kind = 'analysis_settings'),
    digest_value TEXT NOT NULL CHECK (digest_value <> ''),
    PRIMARY KEY (generation_id, scope),
    FOREIGN KEY (generation_id) REFERENCES generations(id) ON DELETE CASCADE
);

CREATE TABLE requested_capabilities (
    generation_id INTEGER NOT NULL,
    capability TEXT NOT NULL CHECK (capability <> ''),
    language TEXT NOT NULL,
    support_status TEXT NOT NULL CHECK (support_status <> ''),
    setup_status TEXT NOT NULL CHECK (setup_status <> ''),
    policy_query_version TEXT,
    rule_behavior_kind TEXT NOT NULL CHECK (rule_behavior_kind = 'rule_options'),
    rule_behavior_value TEXT NOT NULL CHECK (rule_behavior_value <> ''),
    analysis_dependency_kind TEXT NOT NULL CHECK (analysis_dependency_kind = 'analysis_requirements'),
    analysis_dependency_value TEXT NOT NULL CHECK (analysis_dependency_value <> ''),
    requester_count INTEGER NOT NULL CHECK (requester_count >= 0),
    PRIMARY KEY (generation_id, capability, language),
    FOREIGN KEY (generation_id) REFERENCES generations(id) ON DELETE CASCADE
);

CREATE TABLE capability_requesters (
    generation_id INTEGER NOT NULL,
    capability TEXT NOT NULL,
    language TEXT NOT NULL,
    rule_id TEXT NOT NULL CHECK (rule_id <> ''),
    PRIMARY KEY (generation_id, capability, language, rule_id),
    FOREIGN KEY (generation_id, capability, language)
        REFERENCES requested_capabilities(generation_id, capability, language) ON DELETE CASCADE
);

CREATE TABLE provider_schema_snapshots (
    generation_id INTEGER NOT NULL,
    provider_id TEXT NOT NULL CHECK (provider_id <> ''),
    language_scope TEXT NOT NULL CHECK (language_scope <> ''),
    cache_policy TEXT NOT NULL CHECK (cache_policy <> ''),
    precision_ceiling TEXT NOT NULL CHECK (precision_ceiling <> ''),
    manifest_digest_kind TEXT NOT NULL CHECK (manifest_digest_kind = 'provider_manifest'),
    manifest_digest_value TEXT NOT NULL CHECK (manifest_digest_value <> ''),
    version_count INTEGER NOT NULL CHECK (version_count >= 0),
    PRIMARY KEY (generation_id, provider_id),
    FOREIGN KEY (generation_id) REFERENCES generations(id) ON DELETE CASCADE
);

CREATE TABLE provider_schema_versions (
    generation_id INTEGER NOT NULL,
    provider_id TEXT NOT NULL,
    schema_version TEXT NOT NULL CHECK (schema_version <> ''),
    PRIMARY KEY (generation_id, provider_id, schema_version),
    FOREIGN KEY (generation_id, provider_id)
        REFERENCES provider_schema_snapshots(generation_id, provider_id) ON DELETE CASCADE
);

CREATE TABLE provider_manifests (
    generation_id INTEGER NOT NULL,
    provider_id TEXT NOT NULL CHECK (provider_id <> ''),
    provider_version TEXT NOT NULL CHECK (provider_version <> ''),
    provider_kind TEXT NOT NULL CHECK (provider_kind <> ''),
    language_scope TEXT NOT NULL CHECK (language_scope <> ''),
    cache_policy TEXT NOT NULL CHECK (cache_policy <> ''),
    precision_ceiling TEXT NOT NULL CHECK (precision_ceiling <> ''),
    manifest_digest_kind TEXT NOT NULL CHECK (manifest_digest_kind = 'provider_manifest'),
    manifest_digest_value TEXT NOT NULL CHECK (manifest_digest_value <> ''),
    schema_count INTEGER NOT NULL CHECK (schema_count >= 0),
    input_count INTEGER NOT NULL CHECK (input_count >= 0),
    output_count INTEGER NOT NULL CHECK (output_count >= 0),
    PRIMARY KEY (generation_id, provider_id),
    FOREIGN KEY (generation_id) REFERENCES generations(id) ON DELETE CASCADE
);

CREATE TABLE provider_manifest_schemas (
    generation_id INTEGER NOT NULL,
    provider_id TEXT NOT NULL,
    schema_version TEXT NOT NULL CHECK (schema_version <> ''),
    PRIMARY KEY (generation_id, provider_id, schema_version),
    FOREIGN KEY (generation_id, provider_id)
        REFERENCES provider_manifests(generation_id, provider_id) ON DELETE CASCADE
);

CREATE TABLE provider_manifest_inputs (
    generation_id INTEGER NOT NULL,
    provider_id TEXT NOT NULL,
    input_name TEXT NOT NULL CHECK (input_name <> ''),
    PRIMARY KEY (generation_id, provider_id, input_name),
    FOREIGN KEY (generation_id, provider_id)
        REFERENCES provider_manifests(generation_id, provider_id) ON DELETE CASCADE
);

CREATE TABLE provider_manifest_outputs (
    generation_id INTEGER NOT NULL,
    provider_id TEXT NOT NULL,
    output_name TEXT NOT NULL CHECK (output_name <> ''),
    PRIMARY KEY (generation_id, provider_id, output_name),
    FOREIGN KEY (generation_id, provider_id)
        REFERENCES provider_manifests(generation_id, provider_id) ON DELETE CASCADE
);

CREATE TABLE provider_generations (
    id INTEGER PRIMARY KEY,
    generation_id INTEGER NOT NULL,
    provider_id TEXT NOT NULL,
    provider_version TEXT NOT NULL CHECK (provider_version <> ''),
    schema_version TEXT NOT NULL CHECK (schema_version <> ''),
    output_digest_kind TEXT NOT NULL CHECK (output_digest_kind = 'provider_output'),
    output_digest_value TEXT NOT NULL CHECK (output_digest_value <> ''),
    precision TEXT NOT NULL CHECK (precision <> ''),
    validation TEXT NOT NULL CHECK (validation <> ''),
    dependency_count INTEGER NOT NULL CHECK (dependency_count >= 0),
    layer_count INTEGER NOT NULL CHECK (layer_count >= 0),
    UNIQUE (generation_id, id),
    UNIQUE (generation_id, provider_id, provider_version, schema_version, output_digest_kind, output_digest_value),
    FOREIGN KEY (generation_id, provider_id)
        REFERENCES provider_manifests(generation_id, provider_id) ON DELETE CASCADE
);

CREATE TABLE provider_dependencies (
    generation_id INTEGER NOT NULL,
    provider_generation_id INTEGER NOT NULL,
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    dependency_digest_kind TEXT NOT NULL CHECK (dependency_digest_kind <> ''),
    dependency_digest_value TEXT NOT NULL CHECK (dependency_digest_value <> ''),
    PRIMARY KEY (generation_id, provider_generation_id, ordinal),
    UNIQUE (generation_id, provider_generation_id, dependency_digest_kind, dependency_digest_value),
    FOREIGN KEY (generation_id, provider_generation_id)
        REFERENCES provider_generations(generation_id, id) ON DELETE CASCADE
);

CREATE TABLE layers (
    id INTEGER PRIMARY KEY,
    generation_id INTEGER NOT NULL,
    semantic_ordinal INTEGER NOT NULL CHECK (semantic_ordinal >= 0),
    layer_type TEXT NOT NULL CHECK (layer_type <> ''),
    provider_id TEXT NOT NULL CHECK (provider_id <> ''),
    provider_version TEXT NOT NULL CHECK (provider_version <> ''),
    schema_version TEXT NOT NULL CHECK (schema_version <> ''),
    parameter_digest_kind TEXT NOT NULL CHECK (parameter_digest_kind <> ''),
    parameter_digest_value TEXT NOT NULL CHECK (parameter_digest_value <> ''),
    lifecycle_digest_kind TEXT NOT NULL CHECK (lifecycle_digest_kind <> ''),
    lifecycle_digest_value TEXT NOT NULL CHECK (lifecycle_digest_value <> ''),
    settings_digest_kind TEXT NOT NULL CHECK (settings_digest_kind = 'analysis_settings'),
    settings_digest_value TEXT NOT NULL CHECK (settings_digest_value <> ''),
    toolchain_digest_kind TEXT NOT NULL CHECK (toolchain_digest_kind <> ''),
    toolchain_digest_value TEXT NOT NULL CHECK (toolchain_digest_value <> ''),
    output_digest_kind TEXT NOT NULL CHECK (output_digest_kind = 'provider_output'),
    output_digest_value TEXT NOT NULL CHECK (output_digest_value <> ''),
    payload_digest_kind TEXT NOT NULL CHECK (payload_digest_kind = 'layer_output'),
    payload_digest_value TEXT NOT NULL CHECK (payload_digest_value <> ''),
    precision TEXT NOT NULL CHECK (precision <> ''),
    validation TEXT NOT NULL CHECK (validation <> ''),
    input_count INTEGER NOT NULL CHECK (input_count >= 0),
    dependency_layer_count INTEGER NOT NULL CHECK (dependency_layer_count >= 0),
    extension_count INTEGER NOT NULL CHECK (extension_count >= 0),
    edge_count INTEGER NOT NULL CHECK (edge_count >= 0),
    warning_count INTEGER NOT NULL CHECK (warning_count >= 0),
    UNIQUE (generation_id, id),
    UNIQUE (generation_id, semantic_ordinal),
    FOREIGN KEY (generation_id) REFERENCES generations(id) ON DELETE CASCADE
);

CREATE TABLE layer_input_digests (
    generation_id INTEGER NOT NULL,
    layer_id INTEGER NOT NULL,
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    digest_kind TEXT NOT NULL CHECK (digest_kind <> ''),
    digest_value TEXT NOT NULL CHECK (digest_value <> ''),
    PRIMARY KEY (generation_id, layer_id, ordinal),
    UNIQUE (generation_id, layer_id, digest_kind, digest_value),
    FOREIGN KEY (generation_id, layer_id) REFERENCES layers(generation_id, id) ON DELETE CASCADE
);

CREATE TABLE layer_dependency_digests (
    generation_id INTEGER NOT NULL,
    layer_id INTEGER NOT NULL,
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    digest_kind TEXT NOT NULL CHECK (digest_kind = 'dependency_layer'),
    digest_value TEXT NOT NULL CHECK (digest_value <> ''),
    PRIMARY KEY (generation_id, layer_id, ordinal),
    UNIQUE (generation_id, layer_id, digest_kind, digest_value),
    FOREIGN KEY (generation_id, layer_id) REFERENCES layers(generation_id, id) ON DELETE CASCADE
);

CREATE TABLE layer_extension_digests (
    generation_id INTEGER NOT NULL,
    layer_id INTEGER NOT NULL,
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    digest_kind TEXT NOT NULL CHECK (digest_kind = 'extension_code'),
    digest_value TEXT NOT NULL CHECK (digest_value <> ''),
    PRIMARY KEY (generation_id, layer_id, ordinal),
    UNIQUE (generation_id, layer_id, digest_kind, digest_value),
    FOREIGN KEY (generation_id, layer_id) REFERENCES layers(generation_id, id) ON DELETE CASCADE
);

CREATE TABLE layer_warnings (
    generation_id INTEGER NOT NULL,
    layer_id INTEGER NOT NULL,
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    warning_code TEXT NOT NULL CHECK (warning_code <> ''),
    PRIMARY KEY (generation_id, layer_id, ordinal),
    UNIQUE (generation_id, layer_id, warning_code),
    FOREIGN KEY (generation_id, layer_id) REFERENCES layers(generation_id, id) ON DELETE CASCADE
);

CREATE TABLE summaries (
    id INTEGER PRIMARY KEY,
    generation_id INTEGER NOT NULL,
    semantic_ordinal INTEGER NOT NULL CHECK (semantic_ordinal >= 0),
    callable_stable_key TEXT NOT NULL CHECK (callable_stable_key <> ''),
    summary_domain TEXT NOT NULL CHECK (summary_domain <> ''),
    summary_version TEXT NOT NULL CHECK (summary_version <> ''),
    body_shape_digest_kind TEXT NOT NULL CHECK (body_shape_digest_kind <> ''),
    body_shape_digest_value TEXT NOT NULL CHECK (body_shape_digest_value <> ''),
    extension_digest_kind TEXT NOT NULL CHECK (extension_digest_kind = 'extension_code'),
    extension_digest_value TEXT NOT NULL CHECK (extension_digest_value <> ''),
    dependency_count INTEGER NOT NULL CHECK (dependency_count >= 0),
    UNIQUE (generation_id, id),
    UNIQUE (generation_id, semantic_ordinal),
    UNIQUE (generation_id, callable_stable_key, summary_domain, summary_version, body_shape_digest_kind, body_shape_digest_value, extension_digest_kind, extension_digest_value),
    FOREIGN KEY (generation_id) REFERENCES generations(id) ON DELETE CASCADE
);

CREATE TABLE summary_dependency_digests (
    generation_id INTEGER NOT NULL,
    summary_id INTEGER NOT NULL,
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    digest_kind TEXT NOT NULL CHECK (digest_kind = 'summary_dependency'),
    digest_value TEXT NOT NULL CHECK (digest_value <> ''),
    PRIMARY KEY (generation_id, summary_id, ordinal),
    UNIQUE (generation_id, summary_id, digest_kind, digest_value),
    FOREIGN KEY (generation_id, summary_id) REFERENCES summaries(generation_id, id) ON DELETE CASCADE
);

CREATE TABLE queries (
    id INTEGER PRIMARY KEY,
    generation_id INTEGER NOT NULL,
    semantic_ordinal INTEGER NOT NULL CHECK (semantic_ordinal >= 0),
    query_type TEXT NOT NULL CHECK (query_type <> ''),
    query_version TEXT NOT NULL CHECK (query_version <> ''),
    parameter_digest_kind TEXT NOT NULL CHECK (parameter_digest_kind = 'query_parameters'),
    parameter_digest_value TEXT NOT NULL CHECK (parameter_digest_value <> ''),
    budget_digest_kind TEXT NOT NULL CHECK (budget_digest_kind = 'budget'),
    budget_digest_value TEXT NOT NULL CHECK (budget_digest_value <> ''),
    precision_tier TEXT NOT NULL CHECK (precision_tier <> ''),
    result_digest_kind TEXT NOT NULL CHECK (result_digest_kind = 'provider_output'),
    result_digest_value TEXT NOT NULL CHECK (result_digest_value <> ''),
    precision TEXT NOT NULL CHECK (precision <> ''),
    provenance TEXT NOT NULL CHECK (provenance <> ''),
    input_count INTEGER NOT NULL CHECK (input_count >= 0),
    layer_count INTEGER NOT NULL CHECK (layer_count >= 0),
    edge_count INTEGER NOT NULL CHECK (edge_count >= 0),
    UNIQUE (generation_id, id),
    UNIQUE (generation_id, semantic_ordinal),
    FOREIGN KEY (generation_id) REFERENCES generations(id) ON DELETE CASCADE
);

CREATE TABLE query_inputs (
    generation_id INTEGER NOT NULL,
    query_id INTEGER NOT NULL,
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    input_kind TEXT NOT NULL CHECK (input_kind <> ''),
    stable_key TEXT NOT NULL CHECK (stable_key <> ''),
    digest_kind TEXT NOT NULL CHECK (digest_kind <> ''),
    digest_value TEXT NOT NULL CHECK (digest_value <> ''),
    status TEXT NOT NULL CHECK (status <> ''),
    PRIMARY KEY (generation_id, query_id, ordinal),
    UNIQUE (generation_id, query_id, input_kind, stable_key, digest_kind, digest_value, status),
    FOREIGN KEY (generation_id, query_id) REFERENCES queries(generation_id, id) ON DELETE CASCADE
);

CREATE TABLE query_layer_digests (
    generation_id INTEGER NOT NULL,
    query_id INTEGER NOT NULL,
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    digest_kind TEXT NOT NULL CHECK (digest_kind = 'provider_output'),
    digest_value TEXT NOT NULL CHECK (digest_value <> ''),
    PRIMARY KEY (generation_id, query_id, ordinal),
    UNIQUE (generation_id, query_id, digest_kind, digest_value),
    FOREIGN KEY (generation_id, query_id) REFERENCES queries(generation_id, id) ON DELETE CASCADE
);

CREATE TABLE fact_metadata (
    generation_id INTEGER NOT NULL,
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    family TEXT NOT NULL CHECK (family <> ''),
    stable_key TEXT NOT NULL CHECK (stable_key <> ''),
    producer_id TEXT NOT NULL CHECK (producer_id <> ''),
    producer_layer_key TEXT NOT NULL CHECK (producer_layer_key <> ''),
    precision TEXT NOT NULL CHECK (precision <> ''),
    confidence TEXT NOT NULL CHECK (confidence <> ''),
    validation TEXT NOT NULL CHECK (validation <> ''),
    payload_digest TEXT NOT NULL CHECK (payload_digest <> ''),
    PRIMARY KEY (generation_id, ordinal),
    FOREIGN KEY (generation_id) REFERENCES generations(id) ON DELETE CASCADE
) WITHOUT ROWID;

CREATE TABLE diagnostic_nodes (
    id INTEGER PRIMARY KEY,
    generation_id INTEGER NOT NULL,
    semantic_ordinal INTEGER NOT NULL CHECK (semantic_ordinal >= 0),
    rule_id TEXT NOT NULL CHECK (rule_id <> ''),
    rule_version TEXT NOT NULL CHECK (rule_version <> ''),
    rule_code_kind TEXT NOT NULL CHECK (rule_code_kind = 'rule_code'),
    rule_code_value TEXT NOT NULL CHECK (rule_code_value <> ''),
    options_kind TEXT NOT NULL CHECK (options_kind = 'rule_options'),
    options_value TEXT NOT NULL CHECK (options_value <> ''),
    evidence_kind TEXT NOT NULL CHECK (evidence_kind = 'evidence'),
    evidence_value TEXT NOT NULL CHECK (evidence_value <> ''),
    requested_views_digest_kind TEXT NOT NULL CHECK (requested_views_digest_kind = 'dependency'),
    requested_views_digest_value TEXT NOT NULL CHECK (requested_views_digest_value <> ''),
    requested_view_count INTEGER NOT NULL CHECK (requested_view_count >= 0),
    UNIQUE (generation_id, id),
    UNIQUE (generation_id, semantic_ordinal),
    UNIQUE (
        generation_id, rule_id, rule_version,
        rule_code_kind, rule_code_value, options_kind, options_value,
        evidence_kind, evidence_value,
        requested_views_digest_kind, requested_views_digest_value
    ),
    FOREIGN KEY (generation_id) REFERENCES generations(id) ON DELETE CASCADE
);

CREATE TABLE diagnostic_requested_view_digests (
    generation_id INTEGER NOT NULL,
    diagnostic_id INTEGER NOT NULL,
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    digest_kind TEXT NOT NULL CHECK (digest_kind <> ''),
    digest_value TEXT NOT NULL CHECK (digest_value <> ''),
    PRIMARY KEY (generation_id, diagnostic_id, ordinal),
    UNIQUE (generation_id, diagnostic_id, digest_kind, digest_value),
    FOREIGN KEY (generation_id, diagnostic_id)
        REFERENCES diagnostic_nodes(generation_id, id) ON DELETE CASCADE
);

CREATE TABLE dependency_edges (
    generation_id INTEGER NOT NULL,
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    from_node_kind TEXT NOT NULL CHECK (from_node_kind IN ('dependency_input', 'run_manifest', 'layer', 'query', 'summary', 'diagnostic')),
    from_input_kind TEXT,
    from_input_stable_key TEXT,
    from_input_digest_kind TEXT,
    from_input_digest_value TEXT,
    from_input_status TEXT,
    from_layer_id INTEGER,
    from_query_id INTEGER,
    from_summary_id INTEGER,
    from_run_manifest_id INTEGER,
    from_diagnostic_id INTEGER,
    to_node_kind TEXT NOT NULL CHECK (to_node_kind IN ('dependency_input', 'run_manifest', 'layer', 'query', 'summary', 'diagnostic')),
    to_input_kind TEXT,
    to_input_stable_key TEXT,
    to_input_digest_kind TEXT,
    to_input_digest_value TEXT,
    to_input_status TEXT,
    to_layer_id INTEGER,
    to_query_id INTEGER,
    to_summary_id INTEGER,
    to_run_manifest_id INTEGER,
    to_diagnostic_id INTEGER,
    dependency_kind TEXT NOT NULL CHECK (dependency_kind <> ''),
    required_shape TEXT NOT NULL CHECK (required_shape <> ''),
    PRIMARY KEY (generation_id, ordinal),
    CHECK (
        (from_node_kind = 'dependency_input'
            AND from_input_kind IS NOT NULL AND from_input_stable_key IS NOT NULL
            AND from_input_digest_kind IS NOT NULL AND from_input_digest_value IS NOT NULL
            AND from_input_status IS NOT NULL
            AND from_layer_id IS NULL AND from_query_id IS NULL AND from_summary_id IS NULL
            AND from_run_manifest_id IS NULL AND from_diagnostic_id IS NULL)
        OR (from_node_kind = 'run_manifest' AND from_run_manifest_id IS NOT NULL
            AND from_input_kind IS NULL AND from_input_stable_key IS NULL
            AND from_input_digest_kind IS NULL AND from_input_digest_value IS NULL
            AND from_input_status IS NULL AND from_layer_id IS NULL AND from_query_id IS NULL
            AND from_summary_id IS NULL AND from_diagnostic_id IS NULL)
        OR (from_node_kind = 'layer' AND from_layer_id IS NOT NULL
            AND from_input_kind IS NULL AND from_input_stable_key IS NULL
            AND from_input_digest_kind IS NULL AND from_input_digest_value IS NULL
            AND from_input_status IS NULL AND from_query_id IS NULL AND from_summary_id IS NULL
            AND from_run_manifest_id IS NULL AND from_diagnostic_id IS NULL)
        OR (from_node_kind = 'query' AND from_query_id IS NOT NULL
            AND from_input_kind IS NULL AND from_input_stable_key IS NULL
            AND from_input_digest_kind IS NULL AND from_input_digest_value IS NULL
            AND from_input_status IS NULL AND from_layer_id IS NULL AND from_summary_id IS NULL
            AND from_run_manifest_id IS NULL AND from_diagnostic_id IS NULL)
        OR (from_node_kind = 'summary' AND from_summary_id IS NOT NULL
            AND from_input_kind IS NULL AND from_input_stable_key IS NULL
            AND from_input_digest_kind IS NULL AND from_input_digest_value IS NULL
            AND from_input_status IS NULL AND from_layer_id IS NULL AND from_query_id IS NULL
            AND from_run_manifest_id IS NULL AND from_diagnostic_id IS NULL)
        OR (from_node_kind = 'diagnostic' AND from_diagnostic_id IS NOT NULL
            AND from_input_kind IS NULL AND from_input_stable_key IS NULL
            AND from_input_digest_kind IS NULL AND from_input_digest_value IS NULL
            AND from_input_status IS NULL AND from_layer_id IS NULL AND from_query_id IS NULL
            AND from_summary_id IS NULL AND from_run_manifest_id IS NULL)
    ),
    CHECK (
        (to_node_kind = 'dependency_input'
            AND to_input_kind IS NOT NULL AND to_input_stable_key IS NOT NULL
            AND to_input_digest_kind IS NOT NULL AND to_input_digest_value IS NOT NULL
            AND to_input_status IS NOT NULL
            AND to_layer_id IS NULL AND to_query_id IS NULL AND to_summary_id IS NULL
            AND to_run_manifest_id IS NULL AND to_diagnostic_id IS NULL)
        OR (to_node_kind = 'run_manifest' AND to_run_manifest_id IS NOT NULL
            AND to_input_kind IS NULL AND to_input_stable_key IS NULL
            AND to_input_digest_kind IS NULL AND to_input_digest_value IS NULL
            AND to_input_status IS NULL AND to_layer_id IS NULL AND to_query_id IS NULL
            AND to_summary_id IS NULL AND to_diagnostic_id IS NULL)
        OR (to_node_kind = 'layer' AND to_layer_id IS NOT NULL
            AND to_input_kind IS NULL AND to_input_stable_key IS NULL
            AND to_input_digest_kind IS NULL AND to_input_digest_value IS NULL
            AND to_input_status IS NULL AND to_query_id IS NULL AND to_summary_id IS NULL
            AND to_run_manifest_id IS NULL AND to_diagnostic_id IS NULL)
        OR (to_node_kind = 'query' AND to_query_id IS NOT NULL
            AND to_input_kind IS NULL AND to_input_stable_key IS NULL
            AND to_input_digest_kind IS NULL AND to_input_digest_value IS NULL
            AND to_input_status IS NULL AND to_layer_id IS NULL AND to_summary_id IS NULL
            AND to_run_manifest_id IS NULL AND to_diagnostic_id IS NULL)
        OR (to_node_kind = 'summary' AND to_summary_id IS NOT NULL
            AND to_input_kind IS NULL AND to_input_stable_key IS NULL
            AND to_input_digest_kind IS NULL AND to_input_digest_value IS NULL
            AND to_input_status IS NULL AND to_layer_id IS NULL AND to_query_id IS NULL
            AND to_run_manifest_id IS NULL AND to_diagnostic_id IS NULL)
        OR (to_node_kind = 'diagnostic' AND to_diagnostic_id IS NOT NULL
            AND to_input_kind IS NULL AND to_input_stable_key IS NULL
            AND to_input_digest_kind IS NULL AND to_input_digest_value IS NULL
            AND to_input_status IS NULL AND to_layer_id IS NULL AND to_query_id IS NULL
            AND to_summary_id IS NULL AND to_run_manifest_id IS NULL)
    ),
    FOREIGN KEY (generation_id) REFERENCES generations(id) ON DELETE CASCADE,
    FOREIGN KEY (generation_id, from_layer_id) REFERENCES layers(generation_id, id),
    FOREIGN KEY (generation_id, from_query_id) REFERENCES queries(generation_id, id),
    FOREIGN KEY (generation_id, from_summary_id) REFERENCES summaries(generation_id, id),
    FOREIGN KEY (generation_id, from_run_manifest_id) REFERENCES run_manifest_nodes(generation_id, id),
    FOREIGN KEY (generation_id, from_diagnostic_id) REFERENCES diagnostic_nodes(generation_id, id),
    FOREIGN KEY (generation_id, to_layer_id) REFERENCES layers(generation_id, id),
    FOREIGN KEY (generation_id, to_query_id) REFERENCES queries(generation_id, id),
    FOREIGN KEY (generation_id, to_summary_id) REFERENCES summaries(generation_id, id),
    FOREIGN KEY (generation_id, to_run_manifest_id) REFERENCES run_manifest_nodes(generation_id, id),
    FOREIGN KEY (generation_id, to_diagnostic_id) REFERENCES diagnostic_nodes(generation_id, id)
);

CREATE TABLE validation_events (
    generation_id INTEGER NOT NULL,
    event_kind TEXT NOT NULL CHECK (event_kind <> ''),
    status TEXT NOT NULL CHECK (status <> ''),
    issue_count INTEGER NOT NULL CHECK (issue_count >= 0),
    digest_kind TEXT NOT NULL CHECK (digest_kind = 'validation_event'),
    digest_value TEXT NOT NULL CHECK (digest_value <> ''),
    PRIMARY KEY (generation_id, event_kind),
    FOREIGN KEY (generation_id) REFERENCES generations(id) ON DELETE CASCADE
);

CREATE TABLE generation_stats (
    generation_id INTEGER PRIMARY KEY,
    input_file_count INTEGER NOT NULL CHECK (input_file_count >= 0),
    input_component_count INTEGER NOT NULL CHECK (input_component_count >= 0),
    input_detail_count INTEGER NOT NULL CHECK (input_detail_count >= 0),
    analysis_setting_count INTEGER NOT NULL CHECK (analysis_setting_count >= 0),
    capability_count INTEGER NOT NULL CHECK (capability_count >= 0),
    provider_schema_count INTEGER NOT NULL CHECK (provider_schema_count >= 0),
    provider_manifest_count INTEGER NOT NULL CHECK (provider_manifest_count >= 0),
    provider_generation_count INTEGER NOT NULL CHECK (provider_generation_count >= 0),
    layer_count INTEGER NOT NULL CHECK (layer_count >= 0),
    summary_count INTEGER NOT NULL CHECK (summary_count >= 0),
    query_count INTEGER NOT NULL CHECK (query_count >= 0),
    fact_count INTEGER NOT NULL CHECK (fact_count >= 0),
    diagnostic_count INTEGER NOT NULL CHECK (diagnostic_count >= 0),
    dependency_edge_count INTEGER NOT NULL CHECK (dependency_edge_count >= 0),
    validation_event_count INTEGER NOT NULL CHECK (validation_event_count >= 0),
    input_digest_kind TEXT NOT NULL CHECK (input_digest_kind = 'input_snapshot'),
    input_digest_value TEXT NOT NULL CHECK (input_digest_value <> ''),
    provider_manifest_digest_kind TEXT NOT NULL CHECK (provider_manifest_digest_kind = 'provider_manifest'),
    provider_manifest_digest_value TEXT NOT NULL CHECK (provider_manifest_digest_value <> ''),
    provider_output_digest_kind TEXT NOT NULL CHECK (provider_output_digest_kind = 'provider_output'),
    provider_output_digest_value TEXT NOT NULL CHECK (provider_output_digest_value <> ''),
    layer_digest_kind TEXT NOT NULL CHECK (layer_digest_kind = 'layer'),
    layer_digest_value TEXT NOT NULL CHECK (layer_digest_value <> ''),
    summary_digest_kind TEXT NOT NULL CHECK (summary_digest_kind = 'summary'),
    summary_digest_value TEXT NOT NULL CHECK (summary_digest_value <> ''),
    query_digest_kind TEXT NOT NULL CHECK (query_digest_kind = 'query'),
    query_digest_value TEXT NOT NULL CHECK (query_digest_value <> ''),
    fact_digest_kind TEXT NOT NULL CHECK (fact_digest_kind = 'fact_metadata'),
    fact_digest_value TEXT NOT NULL CHECK (fact_digest_value <> ''),
    dependency_digest_kind TEXT NOT NULL CHECK (dependency_digest_kind = 'dependency'),
    dependency_digest_value TEXT NOT NULL CHECK (dependency_digest_value <> ''),
    validation_digest_kind TEXT NOT NULL CHECK (validation_digest_kind = 'validation_event'),
    validation_digest_value TEXT NOT NULL CHECK (validation_digest_value <> ''),
    input_logical_bytes INTEGER NOT NULL CHECK (input_logical_bytes >= 0),
    provider_logical_bytes INTEGER NOT NULL CHECK (provider_logical_bytes >= 0),
    layer_logical_bytes INTEGER NOT NULL CHECK (layer_logical_bytes >= 0),
    summary_logical_bytes INTEGER NOT NULL CHECK (summary_logical_bytes >= 0),
    query_logical_bytes INTEGER NOT NULL CHECK (query_logical_bytes >= 0),
    fact_logical_bytes INTEGER NOT NULL CHECK (fact_logical_bytes >= 0),
    diagnostic_logical_bytes INTEGER NOT NULL CHECK (diagnostic_logical_bytes >= 0),
    dependency_logical_bytes INTEGER NOT NULL CHECK (dependency_logical_bytes >= 0),
    validation_logical_bytes INTEGER NOT NULL CHECK (validation_logical_bytes >= 0),
    semantic_logical_bytes INTEGER NOT NULL CHECK (semantic_logical_bytes >= 0),
    FOREIGN KEY (generation_id) REFERENCES generations(id) ON DELETE CASCADE
);

CREATE TABLE generation_telemetry (
    generation_id INTEGER NOT NULL,
    relative_path TEXT NOT NULL CHECK (relative_path <> ''),
    file_mtime_hint_present INTEGER NOT NULL CHECK (file_mtime_hint_present IN (0, 1)),
    PRIMARY KEY (generation_id, relative_path),
    FOREIGN KEY (generation_id, relative_path)
        REFERENCES input_files(generation_id, relative_path) ON DELETE CASCADE
);

CREATE TABLE generation_failure_events (
    generation_id INTEGER NOT NULL,
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    event_code TEXT NOT NULL CHECK (event_code = 'commit_attempt_failed'),
    reason_code TEXT NOT NULL CHECK (
        reason_code IN ('write_failed', 'post_write_validation_failed', 'publication_commit_failed')
    ),
    stage_code TEXT NOT NULL CHECK (
        stage_code IN (
            'reservation', 'store_run_input', 'providers', 'layers_summaries_queries',
            'fact_metadata', 'dependency_edges', 'validation_events', 'statistics',
            'completion', 'activation', 'transaction_commit'
        )
    ),
    PRIMARY KEY (generation_id, ordinal),
    FOREIGN KEY (generation_id) REFERENCES generations(id) ON DELETE CASCADE
);
"#;

const V2_INDEXES_AND_TRIGGERS_SQL: &str = r#"
CREATE INDEX generations_workspace_status_idx
    ON generations(workspace_kind, workspace_value, status);
CREATE INDEX provider_generations_generation_idx
    ON provider_generations(generation_id, provider_id);
CREATE INDEX dependency_edges_from_endpoint_idx
    ON dependency_edges(
        generation_id, from_node_kind, from_run_manifest_id, from_layer_id, from_query_id,
        from_summary_id, from_diagnostic_id,
        from_input_kind, from_input_stable_key, from_input_digest_kind, from_input_digest_value,
        from_input_status
    );
CREATE INDEX dependency_edges_to_endpoint_idx
    ON dependency_edges(
        generation_id, to_node_kind, to_run_manifest_id, to_layer_id, to_query_id,
        to_summary_id, to_diagnostic_id,
        to_input_kind, to_input_stable_key, to_input_digest_kind, to_input_digest_value,
        to_input_status
    );
CREATE INDEX generation_failure_events_generation_idx
    ON generation_failure_events(generation_id, ordinal);

CREATE TRIGGER generations_require_bound_workspace
BEFORE INSERT ON generations
WHEN NOT EXISTS (
    SELECT 1
    FROM store_manifest
    WHERE id = 1
      AND workspace_kind = NEW.workspace_kind
      AND workspace_value = NEW.workspace_value
)
BEGIN
    SELECT RAISE(ABORT, 'generation workspace is not bound');
END;

CREATE TRIGGER generations_start_pending
BEFORE INSERT ON generations
WHEN NEW.status <> 'pending'
BEGIN
    SELECT RAISE(ABORT, 'generation reservations must start pending');
END;

CREATE TRIGGER generation_reservations_are_contiguous
BEFORE INSERT ON generations
WHEN NEW.reservation_ordinal IS NOT COALESCE((
    SELECT max(prior.reservation_ordinal) + 1
    FROM generations AS prior
    WHERE prior.workspace_kind = NEW.workspace_kind
      AND prior.workspace_value = NEW.workspace_value
      AND prior.generation_kind = NEW.generation_kind
      AND prior.generation_value = NEW.generation_value
), 0)
BEGIN
    SELECT RAISE(ABORT, 'generation reservation ordinal is not contiguous');
END;

CREATE TRIGGER generation_retry_identity_consistent
BEFORE INSERT ON generations
WHEN EXISTS (
    SELECT 1
    FROM generations AS prior
    WHERE prior.workspace_kind = NEW.workspace_kind
      AND prior.workspace_value = NEW.workspace_value
      AND prior.generation_kind = NEW.generation_kind
      AND prior.generation_value = NEW.generation_value
)
 AND NOT EXISTS (
    SELECT 1
    FROM (
        SELECT
            run_kind, run_value,
            full_config_kind, full_config_value,
            input_snapshot_kind, input_snapshot_value,
            provider_manifest_kind, provider_manifest_value,
            provider_output_kind, provider_output_value,
            layer_kind, layer_value,
            summary_kind, summary_value,
            query_kind, query_value,
            fact_kind, fact_value,
            dependency_kind, dependency_value,
            validation_kind, validation_value,
            dependency_schema
        FROM generations AS prior
        WHERE prior.workspace_kind = NEW.workspace_kind
          AND prior.workspace_value = NEW.workspace_value
          AND prior.generation_kind = NEW.generation_kind
          AND prior.generation_value = NEW.generation_value
        ORDER BY prior.reservation_ordinal DESC
        LIMIT 1
    ) AS prior
    WHERE prior.run_kind IS NEW.run_kind
      AND prior.run_value IS NEW.run_value
      AND prior.full_config_kind IS NEW.full_config_kind
      AND prior.full_config_value IS NEW.full_config_value
      AND prior.input_snapshot_kind IS NEW.input_snapshot_kind
      AND prior.input_snapshot_value IS NEW.input_snapshot_value
      AND prior.provider_manifest_kind IS NEW.provider_manifest_kind
      AND prior.provider_manifest_value IS NEW.provider_manifest_value
      AND prior.provider_output_kind IS NEW.provider_output_kind
      AND prior.provider_output_value IS NEW.provider_output_value
      AND prior.layer_kind IS NEW.layer_kind
      AND prior.layer_value IS NEW.layer_value
      AND prior.summary_kind IS NEW.summary_kind
      AND prior.summary_value IS NEW.summary_value
      AND prior.query_kind IS NEW.query_kind
      AND prior.query_value IS NEW.query_value
      AND prior.fact_kind IS NEW.fact_kind
      AND prior.fact_value IS NEW.fact_value
      AND prior.dependency_kind IS NEW.dependency_kind
      AND prior.dependency_value IS NEW.dependency_value
      AND prior.validation_kind IS NEW.validation_kind
      AND prior.validation_value IS NEW.validation_value
      AND prior.dependency_schema IS NEW.dependency_schema
)
BEGIN
    SELECT RAISE(ABORT, 'generation retry identity is inconsistent');
END;

CREATE TRIGGER generation_reservation_identity_immutable
BEFORE UPDATE ON generations
WHEN OLD.id IS NOT NEW.id
  OR OLD.reservation_ordinal IS NOT NEW.reservation_ordinal
  OR OLD.workspace_kind IS NOT NEW.workspace_kind
  OR OLD.workspace_value IS NOT NEW.workspace_value
  OR OLD.generation_kind IS NOT NEW.generation_kind
  OR OLD.generation_value IS NOT NEW.generation_value
  OR OLD.run_kind IS NOT NEW.run_kind
  OR OLD.run_value IS NOT NEW.run_value
  OR OLD.full_config_kind IS NOT NEW.full_config_kind
  OR OLD.full_config_value IS NOT NEW.full_config_value
  OR OLD.input_snapshot_kind IS NOT NEW.input_snapshot_kind
  OR OLD.input_snapshot_value IS NOT NEW.input_snapshot_value
  OR OLD.provider_manifest_kind IS NOT NEW.provider_manifest_kind
  OR OLD.provider_manifest_value IS NOT NEW.provider_manifest_value
  OR OLD.provider_output_kind IS NOT NEW.provider_output_kind
  OR OLD.provider_output_value IS NOT NEW.provider_output_value
  OR OLD.layer_kind IS NOT NEW.layer_kind
  OR OLD.layer_value IS NOT NEW.layer_value
  OR OLD.summary_kind IS NOT NEW.summary_kind
  OR OLD.summary_value IS NOT NEW.summary_value
  OR OLD.query_kind IS NOT NEW.query_kind
  OR OLD.query_value IS NOT NEW.query_value
  OR OLD.fact_kind IS NOT NEW.fact_kind
  OR OLD.fact_value IS NOT NEW.fact_value
  OR OLD.dependency_kind IS NOT NEW.dependency_kind
  OR OLD.dependency_value IS NOT NEW.dependency_value
  OR OLD.validation_kind IS NOT NEW.validation_kind
  OR OLD.validation_value IS NOT NEW.validation_value
  OR OLD.dependency_schema IS NOT NEW.dependency_schema
BEGIN
    SELECT RAISE(ABORT, 'generation reservation identity is immutable');
END;

CREATE TRIGGER generation_status_transitions_are_terminal
BEFORE UPDATE OF status ON generations
WHEN OLD.status IS NOT NEW.status
 AND NOT (OLD.status = 'pending' AND NEW.status IN ('complete', 'failed'))
BEGIN
    SELECT RAISE(ABORT, 'generation status transition is invalid');
END;

CREATE TRIGGER generation_reservations_cannot_be_deleted
BEFORE DELETE ON generations
BEGIN
    SELECT RAISE(ABORT, 'generation reservations cannot be deleted');
END;

CREATE TRIGGER store_manifest_workspace_immutable
BEFORE UPDATE OF workspace_kind, workspace_value ON store_manifest
WHEN OLD.workspace_kind IS NOT NULL
 AND (NEW.workspace_kind IS NOT OLD.workspace_kind OR NEW.workspace_value IS NOT OLD.workspace_value)
BEGIN
    SELECT RAISE(ABORT, 'store workspace binding is immutable');
END;

CREATE TRIGGER store_manifest_active_must_be_complete
BEFORE UPDATE OF active_generation_id ON store_manifest
WHEN NEW.active_generation_id IS NOT NULL
 AND NOT EXISTS (
    SELECT 1
    FROM generations AS generation
    WHERE generation.id = NEW.active_generation_id
      AND generation.workspace_kind = NEW.workspace_kind
      AND generation.workspace_value = NEW.workspace_value
      AND generation.status = 'complete'
      AND NOT EXISTS (
          SELECT 1
          FROM generation_failure_events AS failure
          WHERE failure.generation_id = generation.id
      )
)
BEGIN
    SELECT RAISE(ABORT, 'active generation is not complete for the bound workspace');
END;

CREATE TRIGGER store_manifest_active_cannot_be_cleared
BEFORE UPDATE OF active_generation_id ON store_manifest
WHEN OLD.active_generation_id IS NOT NULL AND NEW.active_generation_id IS NULL
BEGIN
    SELECT RAISE(ABORT, 'active generation cannot be cleared');
END;

CREATE TRIGGER generation_completion_rejects_failures
BEFORE UPDATE OF status ON generations
WHEN NEW.status = 'complete'
 AND EXISTS (
    SELECT 1 FROM generation_failure_events WHERE generation_id = NEW.id
)
BEGIN
    SELECT RAISE(ABORT, 'failed generation cannot become complete');
END;

CREATE TRIGGER active_generation_stays_complete
BEFORE UPDATE OF status ON generations
WHEN NEW.status <> 'complete'
 AND EXISTS (
    SELECT 1 FROM store_manifest WHERE active_generation_id = NEW.id
)
BEGIN
    SELECT RAISE(ABORT, 'active generation must remain complete');
END;

CREATE TRIGGER failure_events_require_failed_generation
BEFORE INSERT ON generation_failure_events
WHEN NOT EXISTS (
    SELECT 1 FROM generations WHERE id = NEW.generation_id AND status = 'failed'
)
 OR EXISTS (
    SELECT 1 FROM store_manifest WHERE active_generation_id = NEW.generation_id
)
BEGIN
    SELECT RAISE(ABORT, 'failure event requires an inactive failed generation');
END;

CREATE TRIGGER failure_event_updates_require_failed_generation
BEFORE UPDATE ON generation_failure_events
WHEN NOT EXISTS (
    SELECT 1 FROM generations WHERE id = NEW.generation_id AND status = 'failed'
)
 OR EXISTS (
    SELECT 1 FROM store_manifest WHERE active_generation_id = NEW.generation_id
)
BEGIN
    SELECT RAISE(ABORT, 'failure event requires an inactive failed generation');
END;
"#;

const V2_MARKER_SQL: &str = "UPDATE _polint_schema_migrations SET version = 2 WHERE version = 1;";

#[derive(Debug, PartialEq, Eq)]
struct SchemaObjectDefinition {
    object_type: String,
    name: String,
    table_name: String,
    sql: Option<String>,
}

static EXPECTED_CURRENT_SCHEMA: OnceLock<Result<Vec<SchemaObjectDefinition>, ()>> = OnceLock::new();

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum MigrationStatus {
    Current,
    Migrated { from: i32, to: i32 },
}

#[derive(Debug, Error)]
pub(super) enum MigrationError {
    #[error("semantic store schema version {found} is newer than supported version {supported}")]
    FutureSchema { found: i32, supported: i32 },
    #[error("semantic store schema version {version} is missing its bootstrap invariant")]
    InvalidSchema { version: i32 },
    #[error("semantic store migration failed: {0}")]
    Sqlite(#[from] rusqlite::Error),
}

#[cfg(test)]
pub(super) fn apply_migrations(
    connection: &mut Connection,
) -> Result<MigrationStatus, MigrationError> {
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let status = apply_migrations_in_transaction(&transaction)?;
    transaction.commit()?;
    Ok(status)
}

pub(super) fn apply_migrations_in_transaction(
    transaction: &Transaction<'_>,
) -> Result<MigrationStatus, MigrationError> {
    let found = schema_version(transaction)?;
    if found > CURRENT_SCHEMA_VERSION {
        return Err(MigrationError::FutureSchema {
            found,
            supported: CURRENT_SCHEMA_VERSION,
        });
    }
    if found > 0 {
        validate_bootstrap_marker(transaction, found)?;
    }

    if found == CURRENT_SCHEMA_VERSION {
        validate_current_schema_shape(transaction)?;
        return Ok(MigrationStatus::Current);
    }

    for migration in MIGRATIONS
        .iter()
        .filter(|migration| migration.version > found)
    {
        for stage in migration.stages {
            transaction.execute_batch(stage)?;
        }
        transaction.pragma_update(None, "user_version", migration.version)?;
    }
    validate_current_schema(transaction)?;

    Ok(MigrationStatus::Migrated {
        from: found,
        to: CURRENT_SCHEMA_VERSION,
    })
}

#[cfg(test)]
pub(super) fn preflight_schema(connection: &Connection) -> Result<(), MigrationError> {
    preflight_schema_shape(connection)?;
    if schema_version(connection)? == CURRENT_SCHEMA_VERSION {
        validate_manifest_lifecycle(connection, CURRENT_SCHEMA_VERSION)?;
        validate_current_foreign_keys(connection)?;
    }
    Ok(())
}

/// Authenticates the exact schema contract without scanning persisted rows.
/// Generation hot paths follow this with a bounded, generation-scoped data
/// preflight and relationship audit.
pub(super) fn preflight_schema_shape(connection: &Connection) -> Result<(), MigrationError> {
    let found = schema_version(connection)?;
    if found > CURRENT_SCHEMA_VERSION {
        return Err(MigrationError::FutureSchema {
            found,
            supported: CURRENT_SCHEMA_VERSION,
        });
    }
    if found > 0 {
        validate_bootstrap_marker(connection, found)?;
    }
    if found == CURRENT_SCHEMA_VERSION {
        validate_current_schema_shape(connection)?;
    }
    Ok(())
}

fn schema_version(connection: &Connection) -> Result<i32, rusqlite::Error> {
    connection.pragma_query_value(None, "user_version", |row| row.get(0))
}

fn validate_bootstrap_marker(
    connection: &Connection,
    expected_version: i32,
) -> Result<(), MigrationError> {
    let table_count: i64 = connection
        .query_row(
            "SELECT count(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
            [BOOTSTRAP_TABLE],
            |row| row.get(0),
        )
        .map_err(|error| classify_invariant_error(error, expected_version))?;
    if table_count != 1 {
        return Err(MigrationError::InvalidSchema {
            version: expected_version,
        });
    }

    let mut column_statement = connection
        .prepare("PRAGMA table_info(_polint_schema_migrations)")
        .map_err(|error| classify_invariant_error(error, expected_version))?;
    let columns = column_statement
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|error| classify_invariant_error(error, expected_version))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| classify_invariant_error(error, expected_version))?;
    if columns != ["version"] {
        return Err(MigrationError::InvalidSchema {
            version: expected_version,
        });
    }

    let marker_count: i64 = connection
        .query_row(
            "SELECT count(*) FROM _polint_schema_migrations WHERE version = ?1",
            [expected_version],
            |row| row.get(0),
        )
        .map_err(|error| classify_invariant_error(error, expected_version))?;
    let total_marker_count: i64 = connection
        .query_row(
            "SELECT count(*) FROM _polint_schema_migrations",
            [],
            |row| row.get(0),
        )
        .map_err(|error| classify_invariant_error(error, expected_version))?;
    if marker_count != 1 || total_marker_count != 1 {
        return Err(MigrationError::InvalidSchema {
            version: expected_version,
        });
    }

    Ok(())
}

fn validate_current_schema(connection: &Connection) -> Result<(), MigrationError> {
    validate_current_schema_shape(connection)?;
    validate_manifest_lifecycle(connection, CURRENT_SCHEMA_VERSION)?;
    validate_current_foreign_keys(connection)
}

fn validate_current_schema_shape(connection: &Connection) -> Result<(), MigrationError> {
    let version = schema_version(connection)?;
    if version != CURRENT_SCHEMA_VERSION {
        return Err(MigrationError::InvalidSchema { version });
    }
    validate_bootstrap_marker(connection, version)?;
    validate_exact_schema_objects(connection, version)?;

    for table in REQUIRED_V2_TABLES {
        let count: i64 = connection
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
                [table],
                |row| row.get(0),
            )
            .map_err(|error| classify_invariant_error(error, version))?;
        if count != 1 {
            return Err(MigrationError::InvalidSchema { version });
        }
    }

    for index in [
        "generations_workspace_status_idx",
        "provider_generations_generation_idx",
        "dependency_edges_from_endpoint_idx",
        "dependency_edges_to_endpoint_idx",
        "generation_failure_events_generation_idx",
    ] {
        let count: i64 = connection
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE type = 'index' AND name = ?1",
                [index],
                |row| row.get(0),
            )
            .map_err(|error| classify_invariant_error(error, version))?;
        if count != 1 {
            return Err(MigrationError::InvalidSchema { version });
        }
    }

    for trigger in [
        "generations_require_bound_workspace",
        "generations_start_pending",
        "generation_reservations_are_contiguous",
        "generation_retry_identity_consistent",
        "generation_reservation_identity_immutable",
        "generation_status_transitions_are_terminal",
        "generation_reservations_cannot_be_deleted",
        "store_manifest_workspace_immutable",
        "store_manifest_active_must_be_complete",
        "store_manifest_active_cannot_be_cleared",
        "generation_completion_rejects_failures",
        "active_generation_stays_complete",
        "failure_events_require_failed_generation",
        "failure_event_updates_require_failed_generation",
    ] {
        let count: i64 = connection
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE type = 'trigger' AND name = ?1",
                [trigger],
                |row| row.get(0),
            )
            .map_err(|error| classify_invariant_error(error, version))?;
        if count != 1 {
            return Err(MigrationError::InvalidSchema { version });
        }
    }

    validate_required_columns(connection, version)?;
    validate_forbidden_schema_names(connection, version)?;

    Ok(())
}

fn validate_current_foreign_keys(connection: &Connection) -> Result<(), MigrationError> {
    let version = schema_version(connection)?;
    #[cfg(test)]
    FULL_FOREIGN_KEY_CHECK_RUNS.set(FULL_FOREIGN_KEY_CHECK_RUNS.get().saturating_add(1));
    let mut statement = connection
        .prepare("PRAGMA foreign_key_check")
        .map_err(|error| classify_invariant_error(error, version))?;
    let has_foreign_key_violation = statement
        .exists([])
        .map_err(|error| classify_invariant_error(error, version))?;
    if has_foreign_key_violation {
        return Err(MigrationError::InvalidSchema { version });
    }

    Ok(())
}

#[cfg(test)]
pub(super) fn reset_full_foreign_key_check_runs_for_test() {
    FULL_FOREIGN_KEY_CHECK_RUNS.set(0);
}

#[cfg(test)]
pub(super) fn full_foreign_key_check_runs_for_test() -> u64 {
    FULL_FOREIGN_KEY_CHECK_RUNS.get()
}

#[cfg(test)]
pub(super) fn reset_full_history_lifecycle_check_runs_for_test() {
    FULL_HISTORY_LIFECYCLE_CHECK_RUNS.set(0);
}

#[cfg(test)]
pub(super) fn full_history_lifecycle_check_runs_for_test() -> u64 {
    FULL_HISTORY_LIFECYCLE_CHECK_RUNS.get()
}

fn validate_exact_schema_objects(
    connection: &Connection,
    version: i32,
) -> Result<(), MigrationError> {
    let actual = read_schema_object_definitions(connection)
        .map_err(|error| classify_invariant_error(error, version))?;
    let expected = EXPECTED_CURRENT_SCHEMA.get_or_init(build_expected_current_schema);
    let Ok(expected) = expected else {
        return Err(MigrationError::InvalidSchema { version });
    };
    if actual.as_slice() != expected.as_slice() {
        return Err(MigrationError::InvalidSchema { version });
    }
    Ok(())
}

fn build_expected_current_schema() -> Result<Vec<SchemaObjectDefinition>, ()> {
    let connection = Connection::open_in_memory().map_err(|_| ())?;
    connection.execute_batch(V1_BOOTSTRAP_SQL).map_err(|_| ())?;
    connection.execute_batch(V2_TABLES_SQL).map_err(|_| ())?;
    connection
        .execute_batch(V2_INDEXES_AND_TRIGGERS_SQL)
        .map_err(|_| ())?;
    read_schema_object_definitions(&connection).map_err(|_| ())
}

fn read_schema_object_definitions(
    connection: &Connection,
) -> Result<Vec<SchemaObjectDefinition>, rusqlite::Error> {
    let mut statement = connection.prepare(
        "SELECT type, name, tbl_name, sql
         FROM sqlite_schema
         WHERE name NOT LIKE 'sqlite_%'
         ORDER BY type, name",
    )?;
    statement
        .query_map([], |row| {
            let sql = row
                .get::<_, Option<String>>(3)?
                .map(|definition| normalize_schema_definition(&definition));
            Ok(SchemaObjectDefinition {
                object_type: row.get(0)?,
                name: row.get(1)?,
                table_name: row.get(2)?,
                sql,
            })
        })?
        .collect()
}

fn normalize_schema_definition(definition: &str) -> String {
    let mut normalized = String::with_capacity(definition.len());
    let mut pending_space = false;
    let mut quote = None;
    let mut characters = definition.chars().peekable();
    while let Some(character) = characters.next() {
        if let Some(terminator) = quote {
            normalized.push(character);
            if character == terminator {
                if terminator != ']' && characters.peek() == Some(&terminator) {
                    if let Some(escaped) = characters.next() {
                        normalized.push(escaped);
                    }
                } else {
                    quote = None;
                }
            }
        } else if character.is_whitespace() {
            pending_space = !normalized.is_empty();
        } else {
            if pending_space {
                normalized.push(' ');
                pending_space = false;
            }
            normalized.push(character);
            quote = match character {
                '\'' | '"' | '`' => Some(character),
                '[' => Some(']'),
                _ => None,
            };
        }
    }
    normalized
}

fn validate_forbidden_schema_names(
    connection: &Connection,
    version: i32,
) -> Result<(), MigrationError> {
    const FORBIDDEN_NAMES: [&str; 11] = [
        "source_text",
        "source_bytes",
        "fact_payload",
        "payload_blob",
        "ast_blob",
        "mir_blob",
        "cfg_blob",
        "summary_body",
        "summary_blob",
        "graph_nodes",
        "graph_edges",
    ];

    let mut table_statement = connection
        .prepare("SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name")
        .map_err(|error| classify_invariant_error(error, version))?;
    let tables = table_statement
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|error| classify_invariant_error(error, version))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| classify_invariant_error(error, version))?;

    for forbidden in FORBIDDEN_NAMES {
        if tables.iter().any(|table| table == forbidden) {
            return Err(MigrationError::InvalidSchema { version });
        }
        for table in &tables {
            let count: i64 = connection
                .query_row(
                    "SELECT count(*) FROM pragma_table_info(?1) WHERE name = ?2",
                    rusqlite::params![table, forbidden],
                    |row| row.get(0),
                )
                .map_err(|error| classify_invariant_error(error, version))?;
            if count != 0 {
                return Err(MigrationError::InvalidSchema { version });
            }
        }
    }

    Ok(())
}

fn validate_required_columns(connection: &Connection, version: i32) -> Result<(), MigrationError> {
    const EXPECTED_COLUMNS: &[(&str, &str)] = &[
        (
            "store_manifest",
            "id workspace_kind workspace_value active_generation_id",
        ),
        (
            "generations",
            "id reservation_ordinal workspace_kind workspace_value generation_kind generation_value \
             run_kind run_value full_config_kind full_config_value input_snapshot_kind input_snapshot_value \
             provider_manifest_kind provider_manifest_value provider_output_kind provider_output_value \
             layer_kind layer_value summary_kind summary_value query_kind query_value fact_kind fact_value \
             dependency_kind dependency_value validation_kind validation_value dependency_schema status",
        ),
        (
            "run_manifest_nodes",
            "id generation_id run_kind run_value full_config_kind full_config_value",
        ),
        (
            "input_snapshots",
            "generation_id schema_version workspace_kind workspace_value full_config_kind full_config_value \
             input_digest_kind input_digest_value requirements_digest_kind requirements_digest_value",
        ),
        (
            "input_files",
            "generation_id relative_path language source_digest_kind source_digest_value size_bytes",
        ),
        (
            "input_components",
            "generation_id component_group name status digest_kind digest_value detail_count",
        ),
        (
            "input_component_details",
            "generation_id component_group component_name ordinal component_digest_kind \
             component_digest_value detail",
        ),
        (
            "analysis_settings",
            "generation_id scope digest_kind digest_value",
        ),
        (
            "requested_capabilities",
            "generation_id capability language support_status setup_status policy_query_version \
             rule_behavior_kind rule_behavior_value analysis_dependency_kind analysis_dependency_value \
             requester_count",
        ),
        (
            "capability_requesters",
            "generation_id capability language rule_id",
        ),
        (
            "provider_schema_snapshots",
            "generation_id provider_id language_scope cache_policy precision_ceiling manifest_digest_kind \
             manifest_digest_value version_count",
        ),
        (
            "provider_schema_versions",
            "generation_id provider_id schema_version",
        ),
        (
            "provider_manifests",
            "generation_id provider_id provider_version provider_kind language_scope cache_policy \
             precision_ceiling manifest_digest_kind manifest_digest_value schema_count input_count output_count",
        ),
        (
            "provider_manifest_schemas",
            "generation_id provider_id schema_version",
        ),
        (
            "provider_manifest_inputs",
            "generation_id provider_id input_name",
        ),
        (
            "provider_manifest_outputs",
            "generation_id provider_id output_name",
        ),
        (
            "provider_generations",
            "id generation_id provider_id provider_version schema_version output_digest_kind \
             output_digest_value precision validation dependency_count layer_count",
        ),
        (
            "provider_dependencies",
            "generation_id provider_generation_id ordinal dependency_digest_kind dependency_digest_value",
        ),
        (
            "layers",
            "id generation_id semantic_ordinal layer_type provider_id provider_version schema_version \
             parameter_digest_kind parameter_digest_value lifecycle_digest_kind lifecycle_digest_value \
             settings_digest_kind settings_digest_value toolchain_digest_kind toolchain_digest_value \
             output_digest_kind output_digest_value payload_digest_kind payload_digest_value precision validation \
             input_count dependency_layer_count extension_count edge_count warning_count",
        ),
        (
            "layer_input_digests",
            "generation_id layer_id ordinal digest_kind digest_value",
        ),
        (
            "layer_dependency_digests",
            "generation_id layer_id ordinal digest_kind digest_value",
        ),
        (
            "layer_extension_digests",
            "generation_id layer_id ordinal digest_kind digest_value",
        ),
        (
            "layer_warnings",
            "generation_id layer_id ordinal warning_code",
        ),
        (
            "summaries",
            "id generation_id semantic_ordinal callable_stable_key summary_domain summary_version \
             body_shape_digest_kind body_shape_digest_value extension_digest_kind extension_digest_value \
             dependency_count",
        ),
        (
            "summary_dependency_digests",
            "generation_id summary_id ordinal digest_kind digest_value",
        ),
        (
            "queries",
            "id generation_id semantic_ordinal query_type query_version parameter_digest_kind \
             parameter_digest_value budget_digest_kind budget_digest_value precision_tier result_digest_kind \
             result_digest_value precision provenance input_count layer_count edge_count",
        ),
        (
            "query_inputs",
            "generation_id query_id ordinal input_kind stable_key digest_kind digest_value status",
        ),
        (
            "query_layer_digests",
            "generation_id query_id ordinal digest_kind digest_value",
        ),
        (
            "fact_metadata",
            "generation_id ordinal family stable_key producer_id producer_layer_key precision confidence validation \
             payload_digest",
        ),
        (
            "diagnostic_nodes",
            "id generation_id semantic_ordinal rule_id rule_version rule_code_kind rule_code_value \
             options_kind options_value evidence_kind evidence_value requested_views_digest_kind \
             requested_views_digest_value requested_view_count",
        ),
        (
            "diagnostic_requested_view_digests",
            "generation_id diagnostic_id ordinal digest_kind digest_value",
        ),
        (
            "dependency_edges",
            "generation_id ordinal from_node_kind from_input_kind from_input_stable_key \
             from_input_digest_kind from_input_digest_value from_input_status from_layer_id from_query_id \
             from_summary_id from_run_manifest_id from_diagnostic_id to_node_kind to_input_kind \
             to_input_stable_key to_input_digest_kind to_input_digest_value to_input_status \
             to_layer_id to_query_id to_summary_id to_run_manifest_id to_diagnostic_id dependency_kind \
             required_shape",
        ),
        (
            "validation_events",
            "generation_id event_kind status issue_count digest_kind digest_value",
        ),
        (
            "generation_stats",
            "generation_id input_file_count input_component_count input_detail_count analysis_setting_count \
             capability_count provider_schema_count provider_manifest_count provider_generation_count layer_count \
             summary_count query_count fact_count diagnostic_count dependency_edge_count validation_event_count input_digest_kind \
             input_digest_value provider_manifest_digest_kind provider_manifest_digest_value \
             provider_output_digest_kind provider_output_digest_value layer_digest_kind layer_digest_value \
             summary_digest_kind summary_digest_value query_digest_kind query_digest_value fact_digest_kind \
             fact_digest_value dependency_digest_kind dependency_digest_value validation_digest_kind \
             validation_digest_value input_logical_bytes provider_logical_bytes layer_logical_bytes \
             summary_logical_bytes query_logical_bytes fact_logical_bytes diagnostic_logical_bytes dependency_logical_bytes \
             validation_logical_bytes semantic_logical_bytes",
        ),
        (
            "generation_telemetry",
            "generation_id relative_path file_mtime_hint_present",
        ),
        (
            "generation_failure_events",
            "generation_id ordinal event_code reason_code stage_code",
        ),
    ];

    for (table, expected) in EXPECTED_COLUMNS {
        let query = format!("PRAGMA table_info({table})");
        let mut statement = connection
            .prepare(&query)
            .map_err(|error| classify_invariant_error(error, version))?;
        let columns = statement
            .query_map([], |row| row.get::<_, String>(1))
            .map_err(|error| classify_invariant_error(error, version))?
            .collect::<Result<std::collections::BTreeSet<_>, _>>()
            .map_err(|error| classify_invariant_error(error, version))?;
        let expected = expected
            .split_ascii_whitespace()
            .map(str::to_owned)
            .collect::<std::collections::BTreeSet<_>>();
        if columns != expected {
            return Err(MigrationError::InvalidSchema { version });
        }
    }

    Ok(())
}

fn validate_manifest_lifecycle(
    connection: &Connection,
    version: i32,
) -> Result<(), MigrationError> {
    #[cfg(test)]
    FULL_HISTORY_LIFECYCLE_CHECK_RUNS
        .set(FULL_HISTORY_LIFECYCLE_CHECK_RUNS.get().saturating_add(1));
    let manifest_count: i64 = connection
        .query_row(
            "SELECT count(*) FROM store_manifest WHERE id = 1",
            [],
            |row| row.get(0),
        )
        .map_err(|error| classify_invariant_error(error, version))?;
    if manifest_count != 1 {
        return Err(MigrationError::InvalidSchema { version });
    }

    let (workspace_kind, workspace_value, active_generation): (
        Option<String>,
        Option<String>,
        Option<i64>,
    ) = connection
        .query_row(
            "SELECT workspace_kind, workspace_value, active_generation_id FROM store_manifest WHERE id = 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .map_err(|error| classify_invariant_error(error, version))?;
    let generation_count: i64 = connection
        .query_row("SELECT count(*) FROM generations", [], |row| row.get(0))
        .map_err(|error| classify_invariant_error(error, version))?;

    match (workspace_kind.as_deref(), workspace_value.as_deref()) {
        (None, None) if active_generation.is_none() && generation_count == 0 => return Ok(()),
        (Some("workspace"), Some(value)) if !value.is_empty() && generation_count > 0 => {}
        _ => return Err(MigrationError::InvalidSchema { version }),
    }

    let mismatched_workspace_count: i64 = connection
        .query_row(
            "SELECT count(*) FROM generations
             WHERE workspace_kind <> ?1 OR workspace_value <> ?2",
            [
                workspace_kind.as_deref().unwrap_or_default(),
                workspace_value.as_deref().unwrap_or_default(),
            ],
            |row| row.get(0),
        )
        .map_err(|error| classify_invariant_error(error, version))?;
    let invalid_failure_owner_count: i64 = connection
        .query_row(
            "SELECT count(*)
             FROM generation_failure_events AS failure
             JOIN generations AS generation ON generation.id = failure.generation_id
             WHERE generation.status <> 'failed'",
            [],
            |row| row.get(0),
        )
        .map_err(|error| classify_invariant_error(error, version))?;
    if mismatched_workspace_count != 0 || invalid_failure_owner_count != 0 {
        return Err(MigrationError::InvalidSchema { version });
    }

    if let Some(active_generation) = active_generation {
        let valid_active_count: i64 = connection
            .query_row(
                "SELECT count(*)
                 FROM generations AS generation
                 WHERE generation.id = ?1
                   AND generation.workspace_kind = ?2
                   AND generation.workspace_value = ?3
                   AND generation.status = 'complete'
                   AND NOT EXISTS (
                       SELECT 1 FROM generation_failure_events AS failure
                       WHERE failure.generation_id = generation.id
                   )",
                rusqlite::params![
                    active_generation,
                    workspace_kind.as_deref().unwrap_or_default(),
                    workspace_value.as_deref().unwrap_or_default()
                ],
                |row| row.get(0),
            )
            .map_err(|error| classify_invariant_error(error, version))?;
        if valid_active_count != 1 {
            return Err(MigrationError::InvalidSchema { version });
        }
    } else {
        let complete_count: i64 = connection
            .query_row(
                "SELECT count(*) FROM generations WHERE status = 'complete'",
                [],
                |row| row.get(0),
            )
            .map_err(|error| classify_invariant_error(error, version))?;
        if complete_count != 0 {
            return Err(MigrationError::InvalidSchema { version });
        }
    }

    Ok(())
}

fn classify_invariant_error(error: rusqlite::Error, version: i32) -> MigrationError {
    match error.sqlite_error_code() {
        Some(
            ErrorCode::DatabaseBusy
            | ErrorCode::DatabaseLocked
            | ErrorCode::DatabaseCorrupt
            | ErrorCode::NotADatabase
            | ErrorCode::SystemIoFailure
            | ErrorCode::CannotOpen,
        ) => MigrationError::Sqlite(error),
        _ => MigrationError::InvalidSchema { version },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Transaction;
    use tempfile::TempDir;

    fn database_path(temp: &TempDir) -> std::path::PathBuf {
        temp.path().join("store.sqlite3")
    }

    fn open_temp_database() -> (TempDir, Connection) {
        let temp = TempDir::new().expect("temp directory");
        let connection = Connection::open(database_path(&temp)).expect("open database");
        (temp, connection)
    }

    fn install_v1_fixture(connection: &Connection) {
        connection
            .execute_batch(V1_BOOTSTRAP_SQL)
            .expect("install v1 bootstrap");
        connection
            .pragma_update(None, "user_version", 1)
            .expect("set v1 version");
    }

    fn schema_snapshot(connection: &Connection) -> Vec<(String, String, Option<String>)> {
        let mut statement = connection
            .prepare(
                "SELECT type, name, sql
                 FROM sqlite_master
                 WHERE name NOT LIKE 'sqlite_%'
                 ORDER BY type, name",
            )
            .expect("prepare schema snapshot");
        statement
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
            .expect("query schema snapshot")
            .collect::<Result<_, _>>()
            .expect("collect schema snapshot")
    }

    fn assert_current_schema_tamper_is_rejected(tamper: &str) {
        let (_temp, mut connection) = open_temp_database();
        apply_migrations(&mut connection).expect("migration succeeds");
        connection.execute_batch(tamper).expect("tamper schema");
        let before = schema_snapshot(&connection);

        let error = apply_migrations(&mut connection).expect_err("schema tamper refused");

        assert!(matches!(
            error,
            MigrationError::InvalidSchema {
                version: CURRENT_SCHEMA_VERSION
            }
        ));
        assert_eq!(schema_snapshot(&connection), before);
    }

    fn bind_workspace(connection: &Connection, workspace_value: &str) {
        connection
            .execute(
                "UPDATE store_manifest
                 SET workspace_kind = 'workspace', workspace_value = ?1
                 WHERE id = 1",
                [workspace_value],
            )
            .expect("bind workspace");
    }

    fn reserve_generation(
        connection: &Connection,
        handle: i64,
        reservation_ordinal: i64,
        workspace_value: &str,
        generation_value: &str,
    ) {
        connection
            .execute(
                "INSERT INTO generations (
                    id, reservation_ordinal,
                    workspace_kind, workspace_value,
                    generation_kind, generation_value,
                    run_kind, run_value,
                    full_config_kind, full_config_value,
                    input_snapshot_kind, input_snapshot_value,
                    provider_manifest_kind, provider_manifest_value,
                    provider_output_kind, provider_output_value,
                    layer_kind, layer_value,
                    summary_kind, summary_value,
                    query_kind, query_value,
                    fact_kind, fact_value,
                    dependency_kind, dependency_value,
                    validation_kind, validation_value,
                    dependency_schema,
                    status
                 ) VALUES (
                    ?1, ?2,
                    'workspace', ?3,
                    'generation', ?4,
                    'run', 'run-value',
                    'config', 'config-value',
                    'input_snapshot', 'input-value',
                    'provider_manifest', 'manifest-value',
                    'provider_output', 'provider-value',
                    'layer', 'layer-value',
                    'summary', 'summary-value',
                    'query', 'query-value',
                    'fact_metadata', 'fact-value',
                    'dependency', 'dependency-value',
                    'validation_event', 'validation-value',
                    'polint-dependency-index-2',
                    'pending'
                 )",
                rusqlite::params![
                    handle,
                    reservation_ordinal,
                    workspace_value,
                    generation_value
                ],
            )
            .expect("reserve generation");
    }

    #[test]
    fn empty_database_migrates_to_current_schema() {
        let (_temp, mut connection) = open_temp_database();

        let status = apply_migrations(&mut connection).expect("migration succeeds");

        assert_eq!(
            status,
            MigrationStatus::Migrated {
                from: 0,
                to: CURRENT_SCHEMA_VERSION
            }
        );
        assert_eq!(
            schema_version(&connection).expect("schema version"),
            CURRENT_SCHEMA_VERSION
        );
        let marker: i32 = connection
            .query_row("SELECT version FROM _polint_schema_migrations", [], |row| {
                row.get(0)
            })
            .expect("migration marker");
        assert_eq!(marker, CURRENT_SCHEMA_VERSION);
        for table in REQUIRED_V2_TABLES {
            let count: i64 = connection
                .query_row(
                    "SELECT count(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
                    [table],
                    |row| row.get(0),
                )
                .expect("table inventory");
            assert_eq!(count, 1, "missing table {table}");
        }
    }

    #[test]
    fn v1_database_migrates_by_replacing_the_only_marker() {
        let (_temp, mut connection) = open_temp_database();
        install_v1_fixture(&connection);

        let status = apply_migrations(&mut connection).expect("migration succeeds");

        assert_eq!(
            status,
            MigrationStatus::Migrated {
                from: 1,
                to: CURRENT_SCHEMA_VERSION
            }
        );
        let markers: Vec<i32> = {
            let mut statement = connection
                .prepare("SELECT version FROM _polint_schema_migrations ORDER BY version")
                .expect("prepare markers");
            statement
                .query_map([], |row| row.get(0))
                .expect("query markers")
                .collect::<Result<_, _>>()
                .expect("collect markers")
        };
        assert_eq!(markers, vec![CURRENT_SCHEMA_VERSION]);
    }

    #[test]
    fn version_zero_with_unknown_schema_is_rejected_without_losing_existing_data() {
        let (_temp, mut connection) = open_temp_database();
        connection
            .execute_batch(
                "CREATE TABLE sentinel (value TEXT NOT NULL);\
                 INSERT INTO sentinel (value) VALUES ('preserve-me');\
                 PRAGMA user_version = 0;",
            )
            .expect("create prior fixture");
        let before = schema_snapshot(&connection);

        let error = apply_migrations(&mut connection).expect_err("unknown schema refused");

        assert!(matches!(
            error,
            MigrationError::InvalidSchema {
                version: CURRENT_SCHEMA_VERSION
            }
        ));
        assert_eq!(schema_snapshot(&connection), before);
        assert_eq!(schema_version(&connection).expect("schema version"), 0);
        let value: String = connection
            .query_row("SELECT value FROM sentinel", [], |row| row.get(0))
            .expect("sentinel row");
        assert_eq!(value, "preserve-me");
    }

    #[test]
    fn reopening_current_schema_is_idempotent() {
        let (temp, mut connection) = open_temp_database();
        apply_migrations(&mut connection).expect("initial migration");
        drop(connection);

        let mut reopened = Connection::open(database_path(&temp)).expect("reopen database");
        let status = apply_migrations(&mut reopened).expect("current schema accepted");

        assert_eq!(status, MigrationStatus::Current);
        let marker_count: i64 = reopened
            .query_row(
                "SELECT count(*) FROM _polint_schema_migrations",
                [],
                |row| row.get(0),
            )
            .expect("marker count");
        let table_count: i64 = reopened
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE type = 'table'",
                [],
                |row| row.get(0),
            )
            .expect("table count");
        assert_eq!(marker_count, 1);
        assert_eq!(table_count, 1 + REQUIRED_V2_TABLES.len() as i64);
    }

    #[test]
    fn injected_v2_ddl_failure_rolls_back_to_the_exact_v1_schema() {
        let (_temp, mut connection) = open_temp_database();
        install_v1_fixture(&connection);
        let before = schema_snapshot(&connection);

        {
            let transaction = connection
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .expect("begin migration transaction");
            transaction
                .execute_batch(V2_TABLES_SQL)
                .expect("first v2 stage succeeds");
            assert!(
                transaction
                    .execute_batch("this is deliberately invalid ddl")
                    .is_err()
            );
        }

        assert_eq!(schema_snapshot(&connection), before);
        assert_eq!(schema_version(&connection).expect("schema version"), 1);
        let marker: i32 = connection
            .query_row("SELECT version FROM _polint_schema_migrations", [], |row| {
                row.get(0)
            })
            .expect("migration marker");
        assert_eq!(marker, 1);
    }

    #[test]
    fn future_schema_is_refused_without_mutation() {
        let (_temp, mut connection) = open_temp_database();
        connection
            .execute_batch(
                "CREATE TABLE sentinel (value TEXT NOT NULL);\
                 INSERT INTO sentinel (value) VALUES ('future-data');",
            )
            .expect("create future fixture");
        let future = CURRENT_SCHEMA_VERSION + 1;
        connection
            .pragma_update(None, "user_version", future)
            .expect("set future version");
        let before = schema_snapshot(&connection);

        let error = apply_migrations(&mut connection).expect_err("future schema refused");

        assert!(matches!(
            error,
            MigrationError::FutureSchema {
                found,
                supported: CURRENT_SCHEMA_VERSION
            } if found == future
        ));
        assert_eq!(schema_version(&connection).expect("schema version"), future);
        assert_eq!(schema_snapshot(&connection), before);
        let value: String = connection
            .query_row("SELECT value FROM sentinel", [], |row| row.get(0))
            .expect("sentinel row");
        assert_eq!(value, "future-data");
    }

    #[test]
    fn current_version_without_bootstrap_invariant_is_invalid() {
        let (_temp, mut connection) = open_temp_database();
        connection
            .pragma_update(None, "user_version", CURRENT_SCHEMA_VERSION)
            .expect("set current version");

        let error = apply_migrations(&mut connection).expect_err("invalid schema refused");

        assert!(matches!(
            error,
            MigrationError::InvalidSchema {
                version: CURRENT_SCHEMA_VERSION
            }
        ));
        let table_count: i64 = connection
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE type = 'table'",
                [],
                |row| row.get(0),
            )
            .expect("table count");
        assert_eq!(table_count, 0);
    }

    #[test]
    fn prior_version_with_wrong_bootstrap_shape_is_invalid_without_mutation() {
        let (_temp, mut connection) = open_temp_database();
        connection
            .execute_batch(
                "CREATE TABLE _polint_schema_migrations (wrong_column INTEGER);\
                 INSERT INTO _polint_schema_migrations (wrong_column) VALUES (1);\
                 PRAGMA user_version = 1;",
            )
            .expect("create wrong-shape fixture");
        let before = schema_snapshot(&connection);

        let error = apply_migrations(&mut connection).expect_err("wrong shape refused");

        assert!(matches!(
            error,
            MigrationError::InvalidSchema { version: 1 }
        ));
        assert_eq!(schema_snapshot(&connection), before);
        assert_eq!(schema_version(&connection).expect("schema version"), 1);
    }

    #[test]
    fn current_version_with_extra_marker_row_is_invalid() {
        let (_temp, mut connection) = open_temp_database();
        connection
            .execute_batch(
                "CREATE TABLE _polint_schema_migrations (\
                     version INTEGER PRIMARY KEY CHECK (version > 0)\
                 );\
                 INSERT INTO _polint_schema_migrations (version) VALUES (1), (2);\
                 PRAGMA user_version = 1;",
            )
            .expect("create extra-marker fixture");

        let error = apply_migrations(&mut connection).expect_err("extra marker refused");

        assert!(matches!(
            error,
            MigrationError::InvalidSchema { version: 1 }
        ));
    }

    #[test]
    fn migration_error_display_is_actionable() {
        let future = CURRENT_SCHEMA_VERSION + 1;
        assert_eq!(
            MigrationError::FutureSchema {
                found: future,
                supported: CURRENT_SCHEMA_VERSION,
            }
            .to_string(),
            format!(
                "semantic store schema version {future} is newer than supported version {CURRENT_SCHEMA_VERSION}"
            )
        );
    }

    #[test]
    fn schema_keeps_semantic_metadata_only_and_indexes_both_edge_endpoints() {
        let (_temp, mut connection) = open_temp_database();
        apply_migrations(&mut connection).expect("migration succeeds");

        let forbidden = [
            "source_text",
            "source_bytes",
            "fact_payload",
            "payload_blob",
            "ast_blob",
            "mir_blob",
            "cfg_blob",
            "summary_body",
            "summary_blob",
            "graph_nodes",
            "graph_edges",
        ];
        let mut statement = connection
            .prepare(
                "SELECT name FROM pragma_table_info(?1)
                 UNION ALL
                 SELECT name FROM sqlite_master WHERE type = 'table' AND name = ?1",
            )
            .expect("prepare forbidden-name query");
        for name in forbidden {
            assert!(
                !statement.exists([name]).expect("query forbidden name"),
                "found {name}"
            );
        }

        let payload_not_null: i64 = connection
            .query_row(
                "SELECT [notnull] FROM pragma_table_info('fact_metadata') WHERE name = 'payload_digest'",
                [],
                |row| row.get(0),
            )
            .expect("payload digest shape");
        assert_eq!(payload_not_null, 1);

        for index in [
            "dependency_edges_from_endpoint_idx",
            "dependency_edges_to_endpoint_idx",
        ] {
            let count: i64 = connection
                .query_row(
                    "SELECT count(*) FROM sqlite_master WHERE type = 'index' AND name = ?1",
                    [index],
                    |row| row.get(0),
                )
                .expect("edge index");
            assert_eq!(count, 1);
        }

        let query_result_kind_check: String = connection
            .query_row(
                "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = 'queries'",
                [],
                |row| row.get(0),
            )
            .expect("query table definition");
        assert!(query_result_kind_check.contains("result_digest_kind = 'provider_output'"));

        let layer_definition: String = connection
            .query_row(
                "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = 'layers'",
                [],
                |row| row.get(0),
            )
            .expect("layer table definition");
        assert!(layer_definition.contains("output_digest_kind = 'provider_output'"));
        assert!(layer_definition.contains("payload_digest_kind = 'layer_output'"));

        let query_layer_definition: String = connection
            .query_row(
                "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = 'query_layer_digests'",
                [],
                |row| row.get(0),
            )
            .expect("query layer table definition");
        assert!(query_layer_definition.contains("digest_kind = 'provider_output'"));
    }

    #[test]
    fn current_schema_missing_a_first_class_column_fails_closed() {
        let (_temp, mut connection) = open_temp_database();
        apply_migrations(&mut connection).expect("migration succeeds");
        connection
            .execute_batch("ALTER TABLE queries DROP COLUMN provenance;")
            .expect("remove required column");
        let before = schema_snapshot(&connection);

        let error = apply_migrations(&mut connection).expect_err("shape mismatch refused");

        assert!(matches!(
            error,
            MigrationError::InvalidSchema {
                version: CURRENT_SCHEMA_VERSION
            }
        ));
        assert_eq!(schema_snapshot(&connection), before);
    }

    #[test]
    fn current_schema_rejects_required_table_with_weakened_constraints() {
        assert_current_schema_tamper_is_rejected(
            "DROP TABLE input_files;
             CREATE TABLE input_files (
                 generation_id INTEGER,
                 relative_path TEXT,
                 language TEXT,
                 source_digest_kind TEXT,
                 source_digest_value TEXT,
                 size_bytes INTEGER
             );",
        );
    }

    #[test]
    fn current_schema_rejects_same_name_trigger_with_wrong_program() {
        assert_current_schema_tamper_is_rejected(
            "DROP TRIGGER store_manifest_workspace_immutable;
             CREATE TRIGGER store_manifest_workspace_immutable
             BEFORE UPDATE OF workspace_kind, workspace_value ON store_manifest
             BEGIN
                 SELECT 1;
             END;",
        );
    }

    #[test]
    fn current_schema_rejects_same_name_index_with_wrong_columns() {
        assert_current_schema_tamper_is_rejected(
            "DROP INDEX dependency_edges_to_endpoint_idx;
             CREATE INDEX dependency_edges_to_endpoint_idx
             ON dependency_edges (generation_id, from_node_kind, ordinal);",
        );
    }

    #[test]
    fn current_schema_rejects_unrecognized_payload_bearing_table() {
        assert_current_schema_tamper_is_rejected(
            "CREATE TABLE cached_sources (contents BLOB NOT NULL);",
        );
    }

    #[test]
    fn current_schema_rejects_forbidden_columns_and_tables() {
        for tamper in [
            "ALTER TABLE fact_metadata ADD COLUMN payload_blob TEXT;",
            "CREATE TABLE graph_nodes (id INTEGER PRIMARY KEY);",
        ] {
            let (_temp, mut connection) = open_temp_database();
            apply_migrations(&mut connection).expect("migration succeeds");
            connection.execute_batch(tamper).expect("tamper schema");
            let before = schema_snapshot(&connection);

            let error = apply_migrations(&mut connection).expect_err("forbidden shape refused");

            assert!(matches!(
                error,
                MigrationError::InvalidSchema {
                    version: CURRENT_SCHEMA_VERSION
                }
            ));
            assert_eq!(schema_snapshot(&connection), before);
        }
    }

    #[test]
    fn relational_lifecycle_accepts_recovery_and_selects_active_by_identity_only() {
        let (_temp, mut connection) = open_temp_database();
        apply_migrations(&mut connection).expect("migration succeeds");
        bind_workspace(&connection, "workspace-a");

        reserve_generation(&connection, 90, 0, "workspace-a", "repeatable-generation");
        connection
            .execute(
                "UPDATE generations SET status = 'complete' WHERE id = 90",
                [],
            )
            .expect("complete selected generation");
        connection
            .execute(
                "UPDATE store_manifest SET active_generation_id = 90 WHERE id = 1",
                [],
            )
            .expect("activate selected generation");

        reserve_generation(&connection, 999, 1, "workspace-a", "repeatable-generation");
        reserve_generation(&connection, 2, 2, "workspace-a", "repeatable-generation");
        validate_current_schema(&connection).expect("active lifecycle remains valid");

        let active: i64 = connection
            .query_row(
                "SELECT active_generation_id FROM store_manifest WHERE id = 1",
                [],
                |row| row.get(0),
            )
            .expect("active generation");
        assert_eq!(active, 90);
        assert_ne!(active, 999, "largest handle must not select the active row");
        assert_ne!(
            active, 2,
            "last inserted row must not select the active row"
        );
        assert_eq!(
            connection
                .query_row(
                    "SELECT count(*) FROM generations WHERE generation_value = 'repeatable-generation'",
                    [],
                    |row| row.get::<_, i64>(0),
                )
                .expect("retry count"),
            3
        );
    }

    #[test]
    fn relational_lifecycle_allows_sanitized_failures_only_on_failed_attempts() {
        let (_temp, mut connection) = open_temp_database();
        apply_migrations(&mut connection).expect("migration succeeds");
        bind_workspace(&connection, "workspace-a");
        reserve_generation(&connection, 1, 0, "workspace-a", "generation-a");

        assert!(
            connection
                .execute(
                    "INSERT INTO generation_failure_events
                 (generation_id, ordinal, event_code, reason_code, stage_code)
                 VALUES (1, 0, 'commit_attempt_failed', 'write_failed', 'providers')",
                    [],
                )
                .is_err()
        );

        connection
            .execute("UPDATE generations SET status = 'failed' WHERE id = 1", [])
            .expect("fail reservation");
        connection
            .execute(
                "INSERT INTO generation_failure_events
                 (generation_id, ordinal, event_code, reason_code, stage_code)
                 VALUES (1, 0, 'commit_attempt_failed', 'write_failed', 'providers')",
                [],
            )
            .expect("record sanitized failure");
        validate_current_schema(&connection).expect("failed attempt is recoverable");

        assert!(
            connection
                .execute(
                    "UPDATE generations SET status = 'complete' WHERE id = 1",
                    []
                )
                .is_err()
        );
        assert!(
            connection
                .execute(
                    "UPDATE store_manifest SET active_generation_id = 1 WHERE id = 1",
                    [],
                )
                .is_err()
        );
        for (event, reason, stage) in [
            ("unknown", "write_failed", "providers"),
            ("commit_attempt_failed", "unknown", "providers"),
            ("commit_attempt_failed", "write_failed", "unknown"),
        ] {
            assert!(
                connection
                    .execute(
                        "INSERT INTO generation_failure_events
                     (generation_id, ordinal, event_code, reason_code, stage_code)
                     VALUES (1, 1, ?1, ?2, ?3)",
                        rusqlite::params![event, reason, stage],
                    )
                    .is_err()
            );
        }
    }

    #[test]
    fn relational_schema_rejects_absolute_paths_and_wrong_fixed_digest_kinds() {
        let (_temp, mut connection) = open_temp_database();
        apply_migrations(&mut connection).expect("migration succeeds");
        bind_workspace(&connection, "workspace-a");
        reserve_generation(&connection, 1, 0, "workspace-a", "generation-a");

        for (path, digest_kind) in [
            ("/tmp/source.go", "source_text"),
            ("src/source.go", "config"),
        ] {
            assert!(connection
                .execute(
                    "INSERT INTO input_files
                     (generation_id, relative_path, language, source_digest_kind, source_digest_value, size_bytes)
                     VALUES (1, ?1, 'go', ?2, 'digest-value', 1)",
                    rusqlite::params![path, digest_kind],
                )
                .is_err());
        }
    }

    #[test]
    fn deterministic_stats_are_separate_from_nondeterministic_telemetry() {
        let (_temp, mut connection) = open_temp_database();
        apply_migrations(&mut connection).expect("migration succeeds");

        let columns = |table: &str| {
            let mut statement = connection
                .prepare("SELECT name FROM pragma_table_info(?1) ORDER BY cid")
                .expect("prepare columns");
            statement
                .query_map([table], |row| row.get::<_, String>(0))
                .expect("query columns")
                .collect::<Result<Vec<_>, _>>()
                .expect("collect columns")
        };
        let stats = columns("generation_stats");
        assert!(stats.iter().all(|column| {
            !column.contains("cache")
                && !column.contains("duration")
                && !column.contains("timestamp")
                && !column.contains("mtime")
        }));
        assert_eq!(
            columns("generation_telemetry"),
            vec!["generation_id", "relative_path", "file_mtime_hint_present"]
        );
    }

    #[test]
    fn transaction_type_remains_private_to_store_module() {
        fn accepts_private_transaction(_transaction: &Transaction<'_>) {}
        let _ = accepts_private_transaction;
    }
}
