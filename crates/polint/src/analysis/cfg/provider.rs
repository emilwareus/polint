use std::borrow::Cow;
use std::collections::BTreeMap;

use crate::analysis::cfg::derived::{
    derive_control_dependence, derive_dominators, derive_postdominators, derive_reachability,
};
use crate::analysis::cfg::facts::CfgView;
use crate::analysis::cfg::ids::{BasicBlockId, CfgEdgeId, CfgFunctionId, CfgNodeId};
use crate::analysis::cfg::lower::lower_cfg;
use crate::analysis::cfg::store::CfgOutput;
use crate::analysis_kernel::ProviderManifest;
use crate::analysis_kernel::incremental::{
    CacheStats, Digest, DigestBuilder, DigestKind, InputComponent, InputComponentStatus,
    InputSnapshot,
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
    let interner_handle = db.stable_key_interner();
    let interner = &interner_handle;
    let mut output = derive_cfg_output(db).normalized(interner);
    append_derived_rows(interner, &mut output, CfgView::NormalControl);
    let output = output.normalized(interner);
    let output_digest = cfg_output_digest(
        manifest,
        input_snapshot,
        &semantic_mir_output_digest,
        &upstream_syntax_output_digests,
        &output,
        interner,
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
    lower_cfg(db)
}

fn append_derived_rows(
    interner: &crate::core::StableKeyInterner,
    output: &mut CfgOutput,
    view: CfgView,
) {
    if output.functions.is_empty() {
        return;
    }
    output.reachability = derive_reachability(interner, output, view);
    output.dominators = derive_dominators(interner, output, view);
    output.postdominators = derive_postdominators(interner, output, view);
    output.control_dependence = derive_control_dependence(interner, output, view);
}

fn cfg_output_digest(
    manifest: &ProviderManifest,
    input_snapshot: &InputSnapshot,
    semantic_mir_output_digest: &Digest,
    upstream_syntax_output_digests: &[Digest],
    output: &CfgOutput,
    interner: &crate::core::StableKeyInterner,
) -> Digest {
    let mut digest = Digest::builder(DigestKind::ProviderOutput, "cfg_output");
    digest.part("provider_id");
    digest.part(manifest.id);
    digest.part("provider_version");
    digest.part(manifest.provider_version());
    digest.part("schema");
    digest.part(&manifest.primary_schema_label());
    digest.part("config");
    digest.part(&input_snapshot.config.digest.to_string());
    digest.part("semantic_mir");
    digest.part(&semantic_mir_output_digest.to_string());
    append_component_digest_parts(
        &mut digest,
        "go_lifecycle",
        &input_snapshot.go_lifecycle.components,
    );
    append_component_digest_parts(
        &mut digest,
        "ts_js_lifecycle",
        &input_snapshot.ts_js_lifecycle.components,
    );
    append_component_digest_parts(&mut digest, "model", &input_snapshot.models);
    append_component_digest_parts(&mut digest, "extension", &input_snapshot.extensions);
    append_component_digest_parts(&mut digest, "tool", &input_snapshot.tool_invocations);

    let mut upstream_syntax_output_digests =
        upstream_syntax_output_digests.iter().collect::<Vec<_>>();
    upstream_syntax_output_digests.sort();
    for upstream in upstream_syntax_output_digests {
        digest.part("upstream_syntax");
        digest.part(&upstream.to_string());
    }

    let function_keys = function_key_map(output);
    let node_keys = node_key_map(output);
    let block_keys = block_key_map(output);
    let edge_keys = edge_key_map(output);

    for row in sorted_refs_by_stable_key(interner, &output.functions) {
        digest.part("cfg_function");
        digest.part(interner.resolve(row.stable_key).as_ref());
        digest.debug_part(row.language);
        digest.part(&span_part(&row.span));
        digest.part(stable_node_key(interner, &node_keys, row.entry_node).as_ref());
        digest.part(stable_node_key(interner, &node_keys, row.normal_exit_node).as_ref());
        digest.part(optional_node_key(interner, &node_keys, row.exceptional_exit_node).as_ref());
        digest.debug_part(row.status);
        digest.debug_part(row.precision);
    }

    for row in sorted_refs_by_stable_key(interner, &output.nodes) {
        digest.part("cfg_node");
        digest.part(interner.resolve(row.stable_key).as_ref());
        digest.part(stable_function_key(interner, &function_keys, row.cfg_function).as_ref());
        digest.part(stable_block_key(interner, &block_keys, row.block).as_ref());
        digest.debug_part(row.kind);
        digest.part(optional_span_part(row.span.as_ref()).as_ref());
        digest.bool_part(row.generated);
        digest.part(&row.operation_ordinal.to_string());
        digest.debug_part(row.status);
        digest.debug_part(row.precision);
    }

    for row in sorted_refs_by_stable_key(interner, &output.blocks) {
        digest.part("basic_block");
        digest.part(interner.resolve(row.stable_key).as_ref());
        digest.part(stable_function_key(interner, &function_keys, row.cfg_function).as_ref());
        digest.debug_part(row.kind);
        digest.part(optional_node_key(interner, &node_keys, row.first_node).as_ref());
        digest.part(optional_node_key(interner, &node_keys, row.last_node).as_ref());
        digest.bool_part(row.reachable);
        digest.part(&row.reverse_postorder.to_string());
        digest.debug_part(row.status);
        digest.debug_part(row.precision);
    }

    for row in sorted_refs_by_stable_key(interner, &output.edges) {
        digest.part("cfg_edge");
        digest.part(interner.resolve(row.stable_key).as_ref());
        digest.part(stable_function_key(interner, &function_keys, row.cfg_function).as_ref());
        digest.debug_part(row.view);
        digest.part(stable_node_key(interner, &node_keys, row.from).as_ref());
        digest.part(stable_node_key(interner, &node_keys, row.to).as_ref());
        digest.part(stable_block_key(interner, &block_keys, row.from_block).as_ref());
        digest.part(stable_block_key(interner, &block_keys, row.to_block).as_ref());
        digest.debug_part(row.kind);
        digest.part(row.label.as_deref().unwrap_or("none"));
        digest.debug_part(row.status);
        digest.debug_part(row.precision);
    }

    for row in sorted_refs_by_stable_key(interner, &output.reachability) {
        digest.part("cfg_reachability");
        digest.part(interner.resolve(row.stable_key).as_ref());
        digest.part(stable_function_key(interner, &function_keys, row.cfg_function).as_ref());
        digest.debug_part(row.view);
        digest.part(stable_block_key(interner, &block_keys, row.block).as_ref());
        digest.bool_part(row.reachable);
        digest.debug_part(row.status);
        digest.debug_part(row.precision);
    }

    for row in sorted_refs_by_stable_key(interner, &output.dominators) {
        digest.part("cfg_dominator");
        digest.part(interner.resolve(row.stable_key).as_ref());
        digest.part(stable_function_key(interner, &function_keys, row.cfg_function).as_ref());
        digest.debug_part(row.view);
        digest.part(stable_block_key(interner, &block_keys, row.dominator).as_ref());
        digest.part(stable_block_key(interner, &block_keys, row.dominated).as_ref());
        digest.bool_part(row.immediate);
        digest.debug_part(row.status);
        digest.debug_part(row.precision);
    }

    for row in sorted_refs_by_stable_key(interner, &output.postdominators) {
        digest.part("cfg_postdominator");
        digest.part(interner.resolve(row.stable_key).as_ref());
        digest.part(stable_function_key(interner, &function_keys, row.cfg_function).as_ref());
        digest.debug_part(row.view);
        digest.part(stable_block_key(interner, &block_keys, row.postdominator).as_ref());
        digest.part(stable_block_key(interner, &block_keys, row.postdominated).as_ref());
        digest.bool_part(row.immediate);
        digest.debug_part(row.status);
        digest.debug_part(row.precision);
    }

    for row in sorted_refs_by_stable_key(interner, &output.control_dependence) {
        digest.part("cfg_control_dependence");
        digest.part(interner.resolve(row.stable_key).as_ref());
        digest.part(stable_function_key(interner, &function_keys, row.cfg_function).as_ref());
        digest.debug_part(row.view);
        digest.part(stable_edge_key(interner, &edge_keys, row.controlling_edge).as_ref());
        digest.debug_part(row.controlling_edge_kind);
        digest.part(stable_block_key(interner, &block_keys, row.controlled_block).as_ref());
        digest.debug_part(row.status);
        digest.debug_part(row.precision);
    }

    for row in sorted_refs_by_stable_key(interner, &output.unsupported) {
        digest.part("unsupported_control_flow");
        digest.part(interner.resolve(row.stable_key).as_ref());
        digest.part(optional_function_key(interner, &function_keys, row.cfg_function).as_ref());
        digest.debug_part(row.language);
        digest.part(&span_part(&row.span));
        digest.part(&row.construct);
        digest.part(&row.source_evidence);
        digest.debug_part(row.conservative_action);
        digest.debug_part(row.status);
        digest.debug_part(row.precision);
    }

    digest.finish()
}

trait StableKeyed {
    fn stable_key(&self) -> crate::core::StableKeyId;
}

macro_rules! impl_stable_keyed {
    ($($ty:ty),+ $(,)?) => {
        $(
            impl StableKeyed for $ty {
                fn stable_key(&self) -> crate::core::StableKeyId {
                    self.stable_key
                }
            }
        )+
    };
}

impl_stable_keyed!(
    crate::analysis::cfg::facts::CfgFunctionFact,
    crate::analysis::cfg::facts::CfgNodeFact,
    crate::analysis::cfg::facts::BasicBlockFact,
    crate::analysis::cfg::facts::CfgEdgeFact,
    crate::analysis::cfg::facts::ReachabilityFact,
    crate::analysis::cfg::facts::DominatorFact,
    crate::analysis::cfg::facts::PostDominatorFact,
    crate::analysis::cfg::facts::ControlDependenceFact,
    crate::analysis::cfg::facts::UnsupportedControlFlowFact,
);

fn sorted_refs_by_stable_key<'a, T: StableKeyed>(
    interner: &crate::core::StableKeyInterner,
    rows: &'a [T],
) -> Vec<&'a T> {
    let mut refs = rows.iter().collect::<Vec<_>>();
    refs.sort_by_cached_key(|row| interner.resolve(row.stable_key()));
    refs
}

fn function_key_map(output: &CfgOutput) -> BTreeMap<CfgFunctionId, crate::core::StableKeyId> {
    output
        .functions
        .iter()
        .map(|row| (row.id, row.stable_key))
        .collect()
}

fn node_key_map(output: &CfgOutput) -> BTreeMap<CfgNodeId, crate::core::StableKeyId> {
    output
        .nodes
        .iter()
        .map(|row| (row.id, row.stable_key))
        .collect()
}

fn block_key_map(output: &CfgOutput) -> BTreeMap<BasicBlockId, crate::core::StableKeyId> {
    output
        .blocks
        .iter()
        .map(|row| (row.id, row.stable_key))
        .collect()
}

fn edge_key_map(output: &CfgOutput) -> BTreeMap<CfgEdgeId, crate::core::StableKeyId> {
    output
        .edges
        .iter()
        .map(|row| (row.id, row.stable_key))
        .collect()
}

fn stable_function_key<'a>(
    interner: &crate::core::StableKeyInterner,
    keys: &BTreeMap<CfgFunctionId, crate::core::StableKeyId>,
    id: CfgFunctionId,
) -> Cow<'a, str> {
    keys.get(&id)
        .map(|key| Cow::Owned(interner.resolve(*key).to_string()))
        .unwrap_or_else(|| Cow::Owned(format!("<missing-function:{}>", id.0)))
}

fn stable_node_key<'a>(
    interner: &crate::core::StableKeyInterner,
    keys: &BTreeMap<CfgNodeId, crate::core::StableKeyId>,
    id: CfgNodeId,
) -> Cow<'a, str> {
    keys.get(&id)
        .map(|key| Cow::Owned(interner.resolve(*key).to_string()))
        .unwrap_or_else(|| Cow::Owned(format!("<missing-node:{}>", id.0)))
}

fn stable_block_key<'a>(
    interner: &crate::core::StableKeyInterner,
    keys: &BTreeMap<BasicBlockId, crate::core::StableKeyId>,
    id: BasicBlockId,
) -> Cow<'a, str> {
    keys.get(&id)
        .map(|key| Cow::Owned(interner.resolve(*key).to_string()))
        .unwrap_or_else(|| Cow::Owned(format!("<missing-block:{}>", id.0)))
}

fn stable_edge_key<'a>(
    interner: &crate::core::StableKeyInterner,
    keys: &BTreeMap<CfgEdgeId, crate::core::StableKeyId>,
    id: CfgEdgeId,
) -> Cow<'a, str> {
    keys.get(&id)
        .map(|key| Cow::Owned(interner.resolve(*key).to_string()))
        .unwrap_or_else(|| Cow::Owned(format!("<missing-edge:{}>", id.0)))
}

fn optional_function_key<'a>(
    interner: &crate::core::StableKeyInterner,
    keys: &BTreeMap<CfgFunctionId, crate::core::StableKeyId>,
    id: Option<CfgFunctionId>,
) -> Cow<'a, str> {
    id.map(|id| stable_function_key(interner, keys, id))
        .unwrap_or(Cow::Borrowed("none"))
}

fn optional_node_key<'a>(
    interner: &crate::core::StableKeyInterner,
    keys: &BTreeMap<CfgNodeId, crate::core::StableKeyId>,
    id: Option<CfgNodeId>,
) -> Cow<'a, str> {
    id.map(|id| stable_node_key(interner, keys, id))
        .unwrap_or(Cow::Borrowed("none"))
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

fn optional_span_part(span: Option<&crate::core::Span>) -> Cow<'_, str> {
    span.map(|span| Cow::Owned(span_part(span)))
        .unwrap_or(Cow::Borrowed("none"))
}

fn append_component_digest_parts(
    digest: &mut DigestBuilder,
    prefix: &str,
    components: &[InputComponent],
) {
    let mut components = components.iter().collect::<Vec<_>>();
    components.sort_by(|left, right| {
        (
            left.name.as_str(),
            component_status_rank(left.status),
            &left.digest,
        )
            .cmp(&(
                right.name.as_str(),
                component_status_rank(right.status),
                &right.digest,
            ))
    });
    for component in components {
        digest.part(prefix);
        digest.part(&component.name);
        digest.debug_part(component.status);
        digest.part(&component.digest.to_string());
    }
}

fn component_status_rank(status: InputComponentStatus) -> u8 {
    match status {
        InputComponentStatus::Present => 0,
        InputComponentStatus::Absent => 1,
        InputComponentStatus::Unsupported => 2,
        InputComponentStatus::SetupMissing => 3,
    }
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
    use crate::analysis_kernel::incremental::{Digest, DigestKind};
    use crate::analysis_plan::AnalysisPlan;
    use crate::cache::Cache;
    use crate::config::load_config;
    use crate::core::{FileId, FunctionId, Language, Span};
    use std::fs;
    use tempfile::tempdir;

    fn span() -> Span {
        Span::new(FileId(1), 1, 2, 1, 1, 1, 2)
    }

    fn body(interner: &crate::core::StableKeyInterner) -> MirBody {
        MirBody {
            id: MirBodyId(1),
            language: Language::Go,
            file: FileId(1),
            function: FunctionId(1),
            package: None,
            module: None,
            owner_stable_key: interner.intern("owner"),
            span: span(),
            stable_key: interner.intern("body:one"),
            status: MirStatus::Resolved,
        }
    }

    fn op(interner: &crate::core::StableKeyInterner, id: u64, ordinal: u32) -> MirOperation {
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
            stable_key: interner.intern(format!("op:{ordinal}")),
            status: MirStatus::Resolved,
        }
    }

    fn branch_output_with_derived_rows(interner: &crate::core::StableKeyInterner) -> CfgOutput {
        let mut builder = CfgBuilder::new();
        builder.start_function(interner, &body(interner), false);
        let entry = builder.current_block();
        let condition = builder.start_block(interner, BasicBlockKind::Branch);
        builder.append_operation_node(
            interner,
            Some(&op(interner, 1, 1)),
            CfgNodeKind::Condition,
            Some(span()),
        );
        let then_block = builder.start_block(interner, BasicBlockKind::StraightLine);
        builder.append_operation_node(
            interner,
            Some(&op(interner, 2, 2)),
            CfgNodeKind::Operation,
            Some(span()),
        );
        let else_block = builder.start_block(interner, BasicBlockKind::StraightLine);
        builder.append_operation_node(
            interner,
            Some(&op(interner, 3, 3)),
            CfgNodeKind::Operation,
            Some(span()),
        );
        let join = builder.start_block(interner, BasicBlockKind::Join);
        builder.append_operation_node(
            interner,
            Some(&op(interner, 4, 4)),
            CfgNodeKind::Operation,
            Some(span()),
        );

        builder.add_edge(interner, entry, condition, CfgEdgeKind::Normal);
        builder.add_edge(interner, condition, then_block, CfgEdgeKind::True);
        builder.add_edge(interner, condition, else_block, CfgEdgeKind::False);
        builder.add_edge(interner, then_block, join, CfgEdgeKind::Normal);
        builder.add_edge(interner, else_block, join, CfgEdgeKind::Normal);
        builder.finish_function();

        let mut output = builder.finish(interner);
        append_derived_rows(interner, &mut output, CfgView::NormalControl);
        output.normalized(interner)
    }

    fn digest_for_output(output: &CfgOutput, interner: &crate::core::StableKeyInterner) -> Digest {
        let temp = tempdir().expect("tempdir");
        fs::write(temp.path().join(".polint.toml"), "").expect("config");
        let loaded = load_config(temp.path()).expect("config loads");
        let db = AnalysisDb::new();
        let snapshot = crate::analysis_kernel::incremental::input_snapshot_from_run_inputs(
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
            interner,
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
        let snapshot = crate::analysis_kernel::incremental::input_snapshot_from_run_inputs(
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
        let interner = db.stable_key_interner();

        let first = cfg_output_digest(
            manifest,
            &snapshot,
            &Digest::from_parts(DigestKind::ProviderOutput, "mir", &["a"]),
            &[],
            &CfgOutput::empty(),
            &interner,
        );
        let second = cfg_output_digest(
            manifest,
            &snapshot,
            &Digest::from_parts(DigestKind::ProviderOutput, "mir", &["b"]),
            &[],
            &CfgOutput::empty(),
            &interner,
        );

        assert_ne!(first, second);
    }

    #[test]
    fn cfg_output_digest_is_order_insensitive_for_upstream_syntax_digests() {
        let temp = tempdir().expect("tempdir");
        fs::write(temp.path().join(".polint.toml"), "").expect("config");
        let loaded = load_config(temp.path()).expect("config loads");
        let db = AnalysisDb::new();
        let snapshot = crate::analysis_kernel::incremental::input_snapshot_from_run_inputs(
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
        let interner = db.stable_key_interner();
        let semantic_mir = Digest::from_parts(DigestKind::ProviderOutput, "mir", &["a"]);
        let go_syntax = Digest::from_parts(DigestKind::ProviderOutput, "go", &["syntax"]);
        let ts_syntax = Digest::from_parts(DigestKind::ProviderOutput, "ts", &["syntax"]);

        let first = cfg_output_digest(
            manifest,
            &snapshot,
            &semantic_mir,
            &[go_syntax.clone(), ts_syntax.clone()],
            &CfgOutput::empty(),
            &interner,
        );
        let second = cfg_output_digest(
            manifest,
            &snapshot,
            &semantic_mir,
            &[ts_syntax, go_syntax],
            &CfgOutput::empty(),
            &interner,
        );

        assert_eq!(first, second);
    }

    #[test]
    fn cfg_output_digest_changes_when_block_reachability_changes() {
        let interner = crate::core::StableKeyInterner::default();
        let output = branch_output_with_derived_rows(&interner);
        let mut changed = output.clone();
        let block = changed
            .blocks
            .iter_mut()
            .find(|block| block.kind == BasicBlockKind::Join)
            .expect("join block");
        block.reachable = !block.reachable;

        assert_ne!(
            digest_for_output(&output, &interner),
            digest_for_output(&changed, &interner)
        );
    }

    #[test]
    fn cfg_output_digest_changes_when_derived_immediate_flag_changes() {
        let interner = crate::core::StableKeyInterner::default();
        let output = branch_output_with_derived_rows(&interner);
        let mut changed = output.clone();
        let dominator = changed
            .dominators
            .iter_mut()
            .find(|row| row.immediate)
            .expect("immediate dominator row");
        dominator.immediate = false;

        assert_ne!(
            digest_for_output(&output, &interner),
            digest_for_output(&changed, &interner)
        );
    }

    #[test]
    fn cfg_output_digest_is_stable_across_dense_id_shifts() {
        let interner = crate::core::StableKeyInterner::default();
        let output = branch_output_with_derived_rows(&interner);
        let shifted = shift_dense_ids(output.clone());

        assert_eq!(
            digest_for_output(&output, &interner),
            digest_for_output(&shifted, &interner)
        );
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
