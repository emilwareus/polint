use crate::analysis_plan::AnalysisPlan;
use crate::config::LoadedConfig;
use crate::core::{AnalysisDb, Language, SourceFile};
use crate::symbol_graph::model::SymbolGraphBuilder;
use crate::symbol_graph::{LanguageSymbolOutput, unsupported_language_support};

pub(crate) fn derive_go_symbols(
    builder: &mut SymbolGraphBuilder,
    db: &AnalysisDb,
    _loaded: &LoadedConfig,
    plan: &AnalysisPlan,
) -> LanguageSymbolOutput {
    let files = go_files(db);
    if files.is_empty() {
        return LanguageSymbolOutput::default();
    }

    let mut output = LanguageSymbolOutput::default();
    output
        .capability_support
        .extend(unsupported_language_support(
            plan,
            Language::Go,
            "Go symbol and reference extraction is not implemented in this plan.",
            "Typed Go package providers are promoted by the follow-up symbol extraction plan.",
        ));

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

fn go_files(db: &AnalysisDb) -> Vec<&SourceFile> {
    let mut files = db
        .files()
        .iter()
        .filter(|file| file.language == Language::Go)
        .collect::<Vec<_>>();
    files.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    files
}
