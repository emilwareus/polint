use std::collections::BTreeMap;

use crate::analysis::cfg::derived::{
    derive_control_dependence, derive_dominators, derive_postdominators, derive_reachability,
};
use crate::analysis::cfg::facts::CfgView;
use crate::analysis::cfg::ids::{
    BasicBlockId, CfgEdgeId, CfgFunctionId, CfgNodeId, UnsupportedControlFlowId,
};
use crate::analysis::cfg::lower_go::lower_go_cfg;
use crate::analysis::cfg::lower_ts::lower_ts_cfg;
use crate::analysis::cfg::store::CfgOutput;
use crate::analysis_kernel::ProviderManifest;
use crate::analysis_kernel::incremental::{
    CacheStats, Digest, DigestKind, InputComponent, InputSnapshot,
};
use crate::core::AnalysisDb;
use crate::diagnostics::{Diagnostic, TextRange};

#[derive(Debug, Clone, Default)]
pub(crate) struct CfgProviderOutput {
    pub(crate) diagnostics: Vec<Diagnostic>,
    pub(crate) cache_stats: CacheStats,
    pub(crate) output_digest: Option<Digest>,
}

pub(crate) fn derive_cfg_with_cache_stats(
    db: &mut AnalysisDb,
    input_snapshot: &InputSnapshot,
    manifest: &ProviderManifest,
    semantic_mir_output_digest: Digest,
    upstream_syntax_output_digests: Vec<Digest>,
) -> CfgProviderOutput {
    let mut output = derive_cfg_output(db).normalized();
    append_derived_rows(&mut output, CfgView::NormalControl);
    let output = output.normalized();
    let output_digest = cfg_output_digest(
        manifest,
        input_snapshot,
        &semantic_mir_output_digest,
        &upstream_syntax_output_digests,
        &output,
    );
    let mut cache_stats = CacheStats::default();
    cache_stats.record_recompute();

    match db.replace_cfg_facts(output) {
        Ok(()) => CfgProviderOutput {
            diagnostics: Vec::new(),
            cache_stats,
            output_digest: Some(output_digest),
        },
        Err(error) => CfgProviderOutput {
            diagnostics: vec![provider_error_diagnostic(error.to_string())],
            cache_stats,
            output_digest: Some(output_digest),
        },
    }
}

fn derive_cfg_output(db: &AnalysisDb) -> CfgOutput {
    let mut output = lower_go_cfg(db);
    merge_base_output(&mut output, lower_ts_cfg(db));
    output.normalized()
}

fn merge_base_output(left: &mut CfgOutput, mut right: CfgOutput) {
    let function_offset = left.functions.iter().map(|row| row.id.0).max().unwrap_or(0);
    let node_offset = left.nodes.iter().map(|row| row.id.0).max().unwrap_or(0);
    let block_offset = left.blocks.iter().map(|row| row.id.0).max().unwrap_or(0);
    let edge_offset = left.edges.iter().map(|row| row.id.0).max().unwrap_or(0);
    let unsupported_offset = left
        .unsupported
        .iter()
        .map(|row| row.id.0)
        .max()
        .unwrap_or(0);

    for row in &mut right.functions {
        row.id = offset_function(row.id, function_offset);
        row.entry_node = offset_node(row.entry_node, node_offset);
        row.normal_exit_node = offset_node(row.normal_exit_node, node_offset);
        row.exceptional_exit_node = row
            .exceptional_exit_node
            .map(|node| offset_node(node, node_offset));
    }
    for row in &mut right.nodes {
        row.id = offset_node(row.id, node_offset);
        row.cfg_function = offset_function(row.cfg_function, function_offset);
        row.block = offset_block(row.block, block_offset);
    }
    for row in &mut right.blocks {
        row.id = offset_block(row.id, block_offset);
        row.cfg_function = offset_function(row.cfg_function, function_offset);
        row.first_node = row.first_node.map(|node| offset_node(node, node_offset));
        row.last_node = row.last_node.map(|node| offset_node(node, node_offset));
    }
    for row in &mut right.edges {
        row.id = offset_edge(row.id, edge_offset);
        row.cfg_function = offset_function(row.cfg_function, function_offset);
        row.from = offset_node(row.from, node_offset);
        row.to = offset_node(row.to, node_offset);
        row.from_block = offset_block(row.from_block, block_offset);
        row.to_block = offset_block(row.to_block, block_offset);
    }
    for row in &mut right.unsupported {
        row.id = UnsupportedControlFlowId(row.id.0 + unsupported_offset);
        row.cfg_function = row
            .cfg_function
            .map(|function| offset_function(function, function_offset));
    }

    left.functions.extend(right.functions);
    left.nodes.extend(right.nodes);
    left.blocks.extend(right.blocks);
    left.edges.extend(right.edges);
    left.unsupported.extend(right.unsupported);
}

fn offset_function(id: CfgFunctionId, offset: u64) -> CfgFunctionId {
    CfgFunctionId(id.0 + offset)
}

fn offset_node(id: CfgNodeId, offset: u64) -> CfgNodeId {
    CfgNodeId(id.0 + offset)
}

fn offset_block(id: BasicBlockId, offset: u64) -> BasicBlockId {
    BasicBlockId(id.0 + offset)
}

fn offset_edge(id: CfgEdgeId, offset: u64) -> CfgEdgeId {
    CfgEdgeId(id.0 + offset)
}

fn append_derived_rows(output: &mut CfgOutput, view: CfgView) {
    if output.functions.is_empty() {
        return;
    }
    output.reachability = derive_reachability(output, view);
    output.dominators = derive_dominators(output, view);
    output.postdominators = derive_postdominators(output, view);
    output.control_dependence = derive_control_dependence(output, view);
}

fn cfg_output_digest(
    manifest: &ProviderManifest,
    input_snapshot: &InputSnapshot,
    semantic_mir_output_digest: &Digest,
    upstream_syntax_output_digests: &[Digest],
    output: &CfgOutput,
) -> Digest {
    let mut parts = vec![
        format!("provider_id={}", manifest.id),
        format!("provider_version={}", manifest.provider_version()),
        format!("schema={}", manifest.primary_schema_label()),
        format!("config={}", input_snapshot.config.digest),
        format!("semantic_mir={semantic_mir_output_digest}"),
    ];
    extend_component_parts(
        &mut parts,
        "go_lifecycle",
        &input_snapshot.go_lifecycle.components,
    );
    extend_component_parts(
        &mut parts,
        "ts_js_lifecycle",
        &input_snapshot.ts_js_lifecycle.components,
    );
    extend_component_parts(&mut parts, "model", &input_snapshot.models);
    extend_component_parts(&mut parts, "extension", &input_snapshot.extensions);
    extend_component_parts(&mut parts, "tool", &input_snapshot.tool_invocations);

    parts.extend(
        upstream_syntax_output_digests
            .iter()
            .map(|digest| format!("upstream_syntax={digest}")),
    );
    let function_keys = function_key_map(output);
    let node_keys = node_key_map(output);
    let block_keys = block_key_map(output);
    let edge_keys = edge_key_map(output);

    parts.extend(output.functions.iter().map(|row| {
        format!(
            "cfg_function={} language={:?} span={} entry={} normal_exit={} exceptional_exit={} status={:?} precision={:?}",
            row.stable_key,
            row.language,
            span_part(&row.span),
            stable_node_key(&node_keys, row.entry_node),
            stable_node_key(&node_keys, row.normal_exit_node),
            row.exceptional_exit_node
                .map(|node| stable_node_key(&node_keys, node))
                .unwrap_or_else(|| "none".to_string()),
            row.status,
            row.precision,
        )
    }));
    parts.extend(output.nodes.iter().map(|row| {
        format!(
            "cfg_node={} function={} block={} kind={:?} span={} generated={} ordinal={} status={:?} precision={:?}",
            row.stable_key,
            stable_function_key(&function_keys, row.cfg_function),
            stable_block_key(&block_keys, row.block),
            row.kind,
            row.span
                .as_ref()
                .map(span_part)
                .unwrap_or_else(|| "none".to_string()),
            row.generated,
            row.operation_ordinal,
            row.status,
            row.precision,
        )
    }));
    parts.extend(output.blocks.iter().map(|row| {
        format!(
            "basic_block={} function={} kind={:?} first_node={} last_node={} reachable={} rpo={} status={:?} precision={:?}",
            row.stable_key,
            stable_function_key(&function_keys, row.cfg_function),
            row.kind,
            row.first_node
                .map(|node| stable_node_key(&node_keys, node))
                .unwrap_or_else(|| "none".to_string()),
            row.last_node
                .map(|node| stable_node_key(&node_keys, node))
                .unwrap_or_else(|| "none".to_string()),
            row.reachable,
            row.reverse_postorder,
            row.status,
            row.precision,
        )
    }));
    parts.extend(output.edges.iter().map(|row| {
        format!(
            "cfg_edge={} function={} view={:?} from={} to={} from_block={} to_block={} kind={:?} label={} status={:?} precision={:?}",
            row.stable_key,
            stable_function_key(&function_keys, row.cfg_function),
            row.view,
            stable_node_key(&node_keys, row.from),
            stable_node_key(&node_keys, row.to),
            stable_block_key(&block_keys, row.from_block),
            stable_block_key(&block_keys, row.to_block),
            row.kind,
            row.label.as_deref().unwrap_or("none"),
            row.status,
            row.precision,
        )
    }));
    parts.extend(output.reachability.iter().map(|row| {
        format!(
            "cfg_reachability={} function={} view={:?} block={} reachable={} status={:?} precision={:?}",
            row.stable_key,
            stable_function_key(&function_keys, row.cfg_function),
            row.view,
            stable_block_key(&block_keys, row.block),
            row.reachable,
            row.status,
            row.precision,
        )
    }));
    parts.extend(output.dominators.iter().map(|row| {
        format!(
            "cfg_dominator={} function={} view={:?} dominator={} dominated={} immediate={} status={:?} precision={:?}",
            row.stable_key,
            stable_function_key(&function_keys, row.cfg_function),
            row.view,
            stable_block_key(&block_keys, row.dominator),
            stable_block_key(&block_keys, row.dominated),
            row.immediate,
            row.status,
            row.precision,
        )
    }));
    parts.extend(output.postdominators.iter().map(|row| {
        format!(
            "cfg_postdominator={} function={} view={:?} postdominator={} postdominated={} immediate={} status={:?} precision={:?}",
            row.stable_key,
            stable_function_key(&function_keys, row.cfg_function),
            row.view,
            stable_block_key(&block_keys, row.postdominator),
            stable_block_key(&block_keys, row.postdominated),
            row.immediate,
            row.status,
            row.precision,
        )
    }));
    parts.extend(output.control_dependence.iter().map(|row| {
        format!(
            "cfg_control_dependence={} function={} view={:?} edge={} edge_kind={:?} controlled_block={} status={:?} precision={:?}",
            row.stable_key,
            stable_function_key(&function_keys, row.cfg_function),
            row.view,
            stable_edge_key(&edge_keys, row.controlling_edge),
            row.controlling_edge_kind,
            stable_block_key(&block_keys, row.controlled_block),
            row.status,
            row.precision,
        )
    }));
    parts.extend(output.unsupported.iter().map(|row| {
        format!(
            "unsupported_control_flow={} function={} language={:?} span={} construct={} evidence={} action={:?} status={:?} precision={:?}",
            row.stable_key,
            row.cfg_function
                .map(|function| stable_function_key(&function_keys, function))
                .unwrap_or_else(|| "none".to_string()),
            row.language,
            span_part(&row.span),
            row.construct,
            row.source_evidence,
            row.conservative_action,
            row.status,
            row.precision,
        )
    }));

    parts.sort();
    let refs = parts.iter().map(String::as_str).collect::<Vec<_>>();
    Digest::from_parts(DigestKind::ProviderOutput, "cfg_output", &refs)
}

fn function_key_map(output: &CfgOutput) -> BTreeMap<CfgFunctionId, String> {
    output
        .functions
        .iter()
        .map(|row| (row.id, row.stable_key.clone()))
        .collect()
}

fn node_key_map(output: &CfgOutput) -> BTreeMap<CfgNodeId, String> {
    output
        .nodes
        .iter()
        .map(|row| (row.id, row.stable_key.clone()))
        .collect()
}

fn block_key_map(output: &CfgOutput) -> BTreeMap<BasicBlockId, String> {
    output
        .blocks
        .iter()
        .map(|row| (row.id, row.stable_key.clone()))
        .collect()
}

fn edge_key_map(output: &CfgOutput) -> BTreeMap<CfgEdgeId, String> {
    output
        .edges
        .iter()
        .map(|row| (row.id, row.stable_key.clone()))
        .collect()
}

fn stable_function_key(keys: &BTreeMap<CfgFunctionId, String>, id: CfgFunctionId) -> String {
    keys.get(&id)
        .cloned()
        .unwrap_or_else(|| format!("<missing-function:{}>", id.0))
}

fn stable_node_key(keys: &BTreeMap<CfgNodeId, String>, id: CfgNodeId) -> String {
    keys.get(&id)
        .cloned()
        .unwrap_or_else(|| format!("<missing-node:{}>", id.0))
}

fn stable_block_key(keys: &BTreeMap<BasicBlockId, String>, id: BasicBlockId) -> String {
    keys.get(&id)
        .cloned()
        .unwrap_or_else(|| format!("<missing-block:{}>", id.0))
}

fn stable_edge_key(keys: &BTreeMap<CfgEdgeId, String>, id: CfgEdgeId) -> String {
    keys.get(&id)
        .cloned()
        .unwrap_or_else(|| format!("<missing-edge:{}>", id.0))
}

fn span_part(span: &crate::core::Span) -> String {
    format!(
        "{}:{}..{}:{}@{}..{}",
        span.start_line,
        span.start_col,
        span.end_line,
        span.end_col,
        span.start_byte,
        span.end_byte
    )
}

fn extend_component_parts(parts: &mut Vec<String>, prefix: &str, components: &[InputComponent]) {
    parts.extend(components.iter().map(|component| {
        format!(
            "{prefix}:{}:{:?}:{}",
            component.name, component.status, component.digest
        )
    }));
}

fn provider_error_diagnostic(message: String) -> Diagnostic {
    Diagnostic::error(
        "polint/internal",
        "<workspace>",
        TextRange::point(1, 1),
        format!("CFG provider failed: {message}"),
    )
}

#[cfg(test)]
mod cfg_provider {
    use super::*;
    use crate::analysis::cfg::builder::CfgBuilder;
    use crate::analysis::cfg::facts::{BasicBlockKind, CfgEdgeKind, CfgNodeKind};
    use crate::analysis::ids::{MirBodyId, MirOpId, PlaceId};
    use crate::analysis::mir::body::{MirBody, MirStatus};
    use crate::analysis::mir::op::{AssignMode, MirOperation, MirOperationKind, MirValue};
    use crate::analysis_kernel::AnalysisKernel;
    use crate::analysis_kernel::incremental::{Digest, DigestKind, InputSnapshot};
    use crate::analysis_plan::AnalysisPlan;
    use crate::cache::Cache;
    use crate::config::load_config;
    use crate::core::{FileId, FunctionId, Language, Span};
    use std::fs;
    use tempfile::tempdir;

    fn span() -> Span {
        Span {
            file: FileId(1),
            start_byte: 1,
            end_byte: 2,
            start_line: 1,
            start_col: 1,
            end_line: 1,
            end_col: 2,
        }
    }

    fn body() -> MirBody {
        MirBody {
            id: MirBodyId(1),
            language: Language::Go,
            file: FileId(1),
            function: FunctionId(1),
            package: None,
            module: None,
            owner_stable_key: "owner".to_string(),
            span: span(),
            stable_key: "body:one".to_string(),
            status: MirStatus::Resolved,
        }
    }

    fn op(id: u64, ordinal: u32) -> MirOperation {
        MirOperation {
            id: MirOpId(id),
            body: MirBodyId(1),
            ordinal,
            span: span(),
            kind: MirOperationKind::Assign {
                place: PlaceId(1),
                value: MirValue::Place(PlaceId(2)),
                mode: AssignMode::Overwrite,
            },
            stable_key: format!("op:{ordinal}"),
            status: MirStatus::Resolved,
        }
    }

    fn branch_output_with_derived_rows() -> CfgOutput {
        let mut builder = CfgBuilder::new();
        builder.start_function(&body(), false);
        let entry = builder.current_block();
        let condition = builder.start_block(BasicBlockKind::Branch);
        builder.append_operation_node(Some(&op(1, 1)), CfgNodeKind::Condition, Some(span()));
        let then_block = builder.start_block(BasicBlockKind::StraightLine);
        builder.append_operation_node(Some(&op(2, 2)), CfgNodeKind::Operation, Some(span()));
        let else_block = builder.start_block(BasicBlockKind::StraightLine);
        builder.append_operation_node(Some(&op(3, 3)), CfgNodeKind::Operation, Some(span()));
        let join = builder.start_block(BasicBlockKind::Join);
        builder.append_operation_node(Some(&op(4, 4)), CfgNodeKind::Operation, Some(span()));

        builder.add_edge(entry, condition, CfgEdgeKind::Normal);
        builder.add_edge(condition, then_block, CfgEdgeKind::True);
        builder.add_edge(condition, else_block, CfgEdgeKind::False);
        builder.add_edge(then_block, join, CfgEdgeKind::Normal);
        builder.add_edge(else_block, join, CfgEdgeKind::Normal);
        builder.finish_function();

        let mut output = builder.finish();
        append_derived_rows(&mut output, CfgView::NormalControl);
        output.normalized()
    }

    fn digest_for_output(output: &CfgOutput) -> Digest {
        let temp = tempdir().expect("tempdir");
        fs::write(temp.path().join(".polint.toml"), "").expect("config");
        let loaded = load_config(temp.path()).expect("config loads");
        let db = AnalysisDb::new();
        let snapshot = InputSnapshot::from_run_inputs(
            &loaded,
            &db,
            "config-a",
            "rules-a",
            "plan-a",
            AnalysisKernel::provider_manifests(),
        );
        let manifest = AnalysisKernel::provider_manifests()
            .iter()
            .find(|manifest| manifest.id == "polint.cfg")
            .expect("cfg manifest");

        cfg_output_digest(
            manifest,
            &snapshot,
            &Digest::from_parts(DigestKind::ProviderOutput, "mir", &["a"]),
            &[],
            output,
        )
    }

    #[test]
    fn cfg_provider_accepts_empty_output_with_deterministic_digest() {
        let temp = tempdir().expect("tempdir");
        fs::write(temp.path().join(".polint.toml"), "").expect("config");
        let loaded = load_config(temp.path()).expect("config loads");
        let cache = Cache::new(temp.path().join(".polint/cache"), false);
        let plan = AnalysisPlan::empty();
        let first = AnalysisKernel::run(crate::analysis_kernel::KernelInput {
            loaded: &loaded,
            cache: &cache,
            config_digest: "cfg-provider-config",
            rule_digest: "cfg-provider-rules",
            plan: &plan,
            parallel: false,
        })
        .expect("kernel run");
        let second = AnalysisKernel::run(crate::analysis_kernel::KernelInput {
            loaded: &loaded,
            cache: &cache,
            config_digest: "cfg-provider-config",
            rule_digest: "cfg-provider-rules",
            plan: &plan,
            parallel: false,
        })
        .expect("kernel run");

        let first_cfg = AnalysisKernel::provider_output_report_for_test(&first)
            .into_iter()
            .find(|row| row.provider_id == "polint.cfg")
            .expect("cfg provider row");
        let second_cfg = AnalysisKernel::provider_output_report_for_test(&second)
            .into_iter()
            .find(|row| row.provider_id == "polint.cfg")
            .expect("cfg provider row");

        assert_eq!(first_cfg.output_digest, second_cfg.output_digest);
        assert_eq!(first_cfg.schema_version, "cfg-facts-1:1");
    }

    #[test]
    fn cfg_output_digest_changes_when_semantic_mir_digest_changes() {
        let temp = tempdir().expect("tempdir");
        fs::write(temp.path().join(".polint.toml"), "").expect("config");
        let loaded = load_config(temp.path()).expect("config loads");
        let db = AnalysisDb::new();
        let snapshot = InputSnapshot::from_run_inputs(
            &loaded,
            &db,
            "config-a",
            "rules-a",
            "plan-a",
            AnalysisKernel::provider_manifests(),
        );
        let manifest = AnalysisKernel::provider_manifests()
            .iter()
            .find(|manifest| manifest.id == "polint.cfg")
            .expect("cfg manifest");

        let first = cfg_output_digest(
            manifest,
            &snapshot,
            &Digest::from_parts(DigestKind::ProviderOutput, "mir", &["a"]),
            &[],
            &CfgOutput::empty(),
        );
        let second = cfg_output_digest(
            manifest,
            &snapshot,
            &Digest::from_parts(DigestKind::ProviderOutput, "mir", &["b"]),
            &[],
            &CfgOutput::empty(),
        );

        assert_ne!(first, second);
    }

    #[test]
    fn cfg_output_digest_changes_when_block_reachability_changes() {
        let output = branch_output_with_derived_rows();
        let mut changed = output.clone();
        let block = changed
            .blocks
            .iter_mut()
            .find(|block| block.kind == BasicBlockKind::Join)
            .expect("join block");
        block.reachable = !block.reachable;

        assert_ne!(digest_for_output(&output), digest_for_output(&changed));
    }

    #[test]
    fn cfg_output_digest_changes_when_derived_immediate_flag_changes() {
        let output = branch_output_with_derived_rows();
        let mut changed = output.clone();
        let dominator = changed
            .dominators
            .iter_mut()
            .find(|row| row.immediate)
            .expect("immediate dominator row");
        dominator.immediate = false;

        assert_ne!(digest_for_output(&output), digest_for_output(&changed));
    }

    #[test]
    fn cfg_output_digest_is_stable_across_dense_id_shifts() {
        let output = branch_output_with_derived_rows();
        let shifted = shift_dense_ids(output.clone());

        assert_eq!(digest_for_output(&output), digest_for_output(&shifted));
    }

    fn shift_dense_ids(mut output: CfgOutput) -> CfgOutput {
        for function in &mut output.functions {
            function.id = CfgFunctionId(function.id.0 + 100);
            function.body = MirBodyId(function.body.0 + 1_000);
            function.function = FunctionId(function.function.0 + 10_000);
            function.file = FileId(function.file.0 + 20_000);
            function.entry_node = CfgNodeId(function.entry_node.0 + 2_000);
            function.normal_exit_node = CfgNodeId(function.normal_exit_node.0 + 2_000);
            function.exceptional_exit_node = function
                .exceptional_exit_node
                .map(|node| CfgNodeId(node.0 + 2_000));
        }
        for node in &mut output.nodes {
            node.id = CfgNodeId(node.id.0 + 2_000);
            node.cfg_function = CfgFunctionId(node.cfg_function.0 + 100);
            node.body = MirBodyId(node.body.0 + 1_000);
            node.operation = node.operation.map(|operation| MirOpId(operation.0 + 3_000));
            node.block = BasicBlockId(node.block.0 + 4_000);
        }
        for block in &mut output.blocks {
            block.id = BasicBlockId(block.id.0 + 4_000);
            block.cfg_function = CfgFunctionId(block.cfg_function.0 + 100);
            block.first_node = block.first_node.map(|node| CfgNodeId(node.0 + 2_000));
            block.last_node = block.last_node.map(|node| CfgNodeId(node.0 + 2_000));
        }
        for edge in &mut output.edges {
            edge.id = CfgEdgeId(edge.id.0 + 5_000);
            edge.cfg_function = CfgFunctionId(edge.cfg_function.0 + 100);
            edge.from = CfgNodeId(edge.from.0 + 2_000);
            edge.to = CfgNodeId(edge.to.0 + 2_000);
            edge.from_block = BasicBlockId(edge.from_block.0 + 4_000);
            edge.to_block = BasicBlockId(edge.to_block.0 + 4_000);
        }
        for row in &mut output.reachability {
            row.cfg_function = CfgFunctionId(row.cfg_function.0 + 100);
            row.block = BasicBlockId(row.block.0 + 4_000);
        }
        for row in &mut output.dominators {
            row.cfg_function = CfgFunctionId(row.cfg_function.0 + 100);
            row.dominator = BasicBlockId(row.dominator.0 + 4_000);
            row.dominated = BasicBlockId(row.dominated.0 + 4_000);
        }
        for row in &mut output.postdominators {
            row.cfg_function = CfgFunctionId(row.cfg_function.0 + 100);
            row.postdominator = BasicBlockId(row.postdominator.0 + 4_000);
            row.postdominated = BasicBlockId(row.postdominated.0 + 4_000);
        }
        for row in &mut output.control_dependence {
            row.cfg_function = CfgFunctionId(row.cfg_function.0 + 100);
            row.controlling_edge = CfgEdgeId(row.controlling_edge.0 + 5_000);
            row.controlled_block = BasicBlockId(row.controlled_block.0 + 4_000);
        }
        for row in &mut output.unsupported {
            row.cfg_function = row
                .cfg_function
                .map(|function| CfgFunctionId(function.0 + 100));
            row.body = row.body.map(|body| MirBodyId(body.0 + 1_000));
            row.operation = row.operation.map(|operation| MirOpId(operation.0 + 3_000));
            row.file = FileId(row.file.0 + 20_000);
        }
        output
    }
}
