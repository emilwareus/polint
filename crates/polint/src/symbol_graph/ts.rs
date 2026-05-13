use crate::analysis_plan::AnalysisPlan;
use crate::config::LoadedConfig;
use crate::core::{AnalysisDb, SourceFile};
use crate::symbol_graph::model::SymbolGraphBuilder;
use crate::symbol_graph::{LanguageSymbolOutput, unsupported_language_support};
use std::collections::BTreeSet;

pub(crate) fn derive_ts_symbols(
    builder: &mut SymbolGraphBuilder,
    db: &AnalysisDb,
    _loaded: &LoadedConfig,
    plan: &AnalysisPlan,
) -> LanguageSymbolOutput {
    let files = ts_files(db);
    if files.is_empty() {
        return LanguageSymbolOutput::default();
    }

    let mut output = LanguageSymbolOutput::default();
    let languages = files
        .iter()
        .map(|file| file.language)
        .collect::<BTreeSet<_>>();
    for language in languages {
        output.capability_support.extend(unsupported_language_support(
            plan,
            language,
            "TypeScript/JavaScript symbol and reference extraction is not implemented in this plan.",
            "Semantic TS/JS providers are promoted by the follow-up symbol extraction plan.",
        ));
    }

    if plan.requests_capability("references") {
        for file in files {
            builder.add_unsupported_reference(
                file.language,
                file.id,
                file.relative_path.clone(),
                "<unsupported>",
            );
        }
    }

    output
}

fn ts_files(db: &AnalysisDb) -> Vec<&SourceFile> {
    let mut files = db
        .files()
        .iter()
        .filter(|file| file.language.is_ts_family())
        .collect::<Vec<_>>();
    files.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    files
}
