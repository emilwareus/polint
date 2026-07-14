use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt;
use std::hash::{Hash, Hasher};

use crate::analysis_kernel::incremental::{Digest, DigestKind};
use crate::core::{ResolutionPrecision, ResolutionStatus, SymbolPrecision};
use rayon::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum FactFamily {
    SourceFile,
    Package,
    Function,
    Import,
    BranchObligation,
    Test,
    Coverage,
    TsComponent,
    TsClass,
    StringLiteral,
    JsxAttribute,
    ResolvedImport,
    ModuleNode,
    ModuleEdge,
    WorkspaceRoot,
    TopologyPackage,
    SourceSet,
    DependencyRequirement,
    ResolvedDependencyEdge,
    ImportToPackage,
    RepoTopologyOverlay,
    Scope,
    SemanticImport,
    Export,
    Alias,
    Resolution,
    GeneratedSymbol,
    StableExport,
    Symbol,
    Definition,
    Reference,
    FileMetric,
    FunctionMetric,
    ComplexityMetric,
    Place,
    MirBody,
    MirOperation,
    CfgFunction,
    CfgNode,
    BasicBlock,
    CfgEdge,
    CfgReachability,
    CfgDominator,
    CfgPostDominator,
    CfgControlDependence,
    UnsupportedControlFlow,
    CallSite,
    CallTarget,
    UnresolvedCall,
    RefinedCallEdge,
    DataFlowNode,
    DataFlowEdge,
    DataFlowModel,
    DataFlowBudget,
    EvidenceNode,
    EvidenceEdge,
    EvidenceBundle,
    EvidencePath,
    EvidenceSlice,
    EvidenceUnknown,
    EvidenceOmittedRegion,
    EvidenceReplayKey,
    DomainObservation,
    DomainEvent,
    SummaryControl,
    SummaryCall,
    SummaryMemory,
    SummaryTito,
    SummaryEvent,
    ExtensionFact,
    Entrypoint,
    TrustBoundary,
    DispatchEdge,
    UnresolvedFramework,
    Type,
    NarrowedType,
    Value,
    AllocationToken,
    AccessPath,
    PointsToConstraint,
    PointsToSet,
    AliasAnswer,
    AdaptationModel,
    /// Unified-solver derived-edge facts. Each row tags a
    /// solver-derived edge whose `DerivedEdgeProvenance` records its contributing
    /// fact IDs (total-ordered by stable ID), producing `ConstraintKind`, and the
    /// monotonic solver step.
    SolverDerivedEdge,
    #[cfg_attr(
        not(test),
        allow(
            dead_code,
            reason = "type/value alias event metadata remains reserved kernel vocabulary"
        )
    )]
    TypeValueAliasEvent,
    #[cfg_attr(
        not(test),
        allow(
            dead_code,
            reason = "statement-level MIR metadata remains reserved kernel vocabulary"
        )
    )]
    MirStatement,
    #[cfg_attr(
        not(test),
        allow(
            dead_code,
            reason = "terminator-level MIR metadata remains reserved kernel vocabulary"
        )
    )]
    MirTerminator,
    UnsupportedSemantic,
}

#[cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "canonical fact-family decoding is consumed by private metadata readers"
    )
)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct UnknownFactFamilyLabel {
    label: String,
}

impl fmt::Display for UnknownFactFamilyLabel {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "unknown fact family label `{}`", self.label)
    }
}

impl std::error::Error for UnknownFactFamilyLabel {}

impl FactFamily {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::SourceFile => "SourceFile",
            Self::Package => "Package",
            Self::Function => "Function",
            Self::Import => "Import",
            Self::BranchObligation => "BranchObligation",
            Self::Test => "Test",
            Self::Coverage => "Coverage",
            Self::TsComponent => "TsComponent",
            Self::TsClass => "TsClass",
            Self::StringLiteral => "StringLiteral",
            Self::JsxAttribute => "JsxAttribute",
            Self::ResolvedImport => "ResolvedImport",
            Self::ModuleNode => "ModuleNode",
            Self::ModuleEdge => "ModuleEdge",
            Self::WorkspaceRoot => "WorkspaceRoot",
            Self::TopologyPackage => "TopologyPackage",
            Self::SourceSet => "SourceSet",
            Self::DependencyRequirement => "DependencyRequirement",
            Self::ResolvedDependencyEdge => "ResolvedDependencyEdge",
            Self::ImportToPackage => "ImportToPackage",
            Self::RepoTopologyOverlay => "RepoTopologyOverlay",
            Self::Scope => "Scope",
            Self::SemanticImport => "SemanticImport",
            Self::Export => "Export",
            Self::Alias => "Alias",
            Self::Resolution => "Resolution",
            Self::GeneratedSymbol => "GeneratedSymbol",
            Self::StableExport => "StableExport",
            Self::Symbol => "Symbol",
            Self::Definition => "Definition",
            Self::Reference => "Reference",
            Self::FileMetric => "FileMetric",
            Self::FunctionMetric => "FunctionMetric",
            Self::ComplexityMetric => "ComplexityMetric",
            Self::Place => "Place",
            Self::MirBody => "MirBody",
            Self::MirOperation => "MirOperation",
            Self::CfgFunction => "CfgFunction",
            Self::CfgNode => "CfgNode",
            Self::BasicBlock => "BasicBlock",
            Self::CfgEdge => "CfgEdge",
            Self::CfgReachability => "CfgReachability",
            Self::CfgDominator => "CfgDominator",
            Self::CfgPostDominator => "CfgPostDominator",
            Self::CfgControlDependence => "CfgControlDependence",
            Self::UnsupportedControlFlow => "UnsupportedControlFlow",
            Self::CallSite => "CallSite",
            Self::CallTarget => "CallTarget",
            Self::UnresolvedCall => "UnresolvedCall",
            Self::RefinedCallEdge => "RefinedCallEdge",
            Self::DataFlowNode => "DataFlowNode",
            Self::DataFlowEdge => "DataFlowEdge",
            Self::DataFlowModel => "DataFlowModel",
            Self::DataFlowBudget => "DataFlowBudget",
            Self::EvidenceNode => "EvidenceNode",
            Self::EvidenceEdge => "EvidenceEdge",
            Self::EvidenceBundle => "EvidenceBundle",
            Self::EvidencePath => "EvidencePath",
            Self::EvidenceSlice => "EvidenceSlice",
            Self::EvidenceUnknown => "EvidenceUnknown",
            Self::EvidenceOmittedRegion => "EvidenceOmittedRegion",
            Self::EvidenceReplayKey => "EvidenceReplayKey",
            Self::DomainObservation => "DomainObservation",
            Self::DomainEvent => "DomainEvent",
            Self::SummaryControl => "SummaryControl",
            Self::SummaryCall => "SummaryCall",
            Self::SummaryMemory => "SummaryMemory",
            Self::SummaryTito => "SummaryTito",
            Self::SummaryEvent => "SummaryEvent",
            Self::ExtensionFact => "ExtensionFact",
            Self::Entrypoint => "Entrypoint",
            Self::TrustBoundary => "TrustBoundary",
            Self::DispatchEdge => "DispatchEdge",
            Self::UnresolvedFramework => "UnresolvedFramework",
            Self::Type => "Type",
            Self::NarrowedType => "NarrowedType",
            Self::Value => "Value",
            Self::AllocationToken => "AllocationToken",
            Self::AccessPath => "AccessPath",
            Self::PointsToConstraint => "PointsToConstraint",
            Self::PointsToSet => "PointsToSet",
            Self::AliasAnswer => "AliasAnswer",
            Self::AdaptationModel => "AdaptationModel",
            Self::SolverDerivedEdge => "SolverDerivedEdge",
            Self::TypeValueAliasEvent => "TypeValueAliasEvent",
            Self::MirStatement => "MirStatement",
            Self::MirTerminator => "MirTerminator",
            Self::UnsupportedSemantic => "UnsupportedSemantic",
        }
    }

    #[cfg_attr(
        not(test),
        allow(
            dead_code,
            reason = "canonical fact-family codecs stay symmetric for durable row decoding"
        )
    )]
    pub(crate) fn parse_label(label: &str) -> Result<Self, UnknownFactFamilyLabel> {
        match label {
            "SourceFile" => Ok(Self::SourceFile),
            "Package" => Ok(Self::Package),
            "Function" => Ok(Self::Function),
            "Import" => Ok(Self::Import),
            "BranchObligation" => Ok(Self::BranchObligation),
            "Test" => Ok(Self::Test),
            "Coverage" => Ok(Self::Coverage),
            "TsComponent" => Ok(Self::TsComponent),
            "TsClass" => Ok(Self::TsClass),
            "StringLiteral" => Ok(Self::StringLiteral),
            "JsxAttribute" => Ok(Self::JsxAttribute),
            "ResolvedImport" => Ok(Self::ResolvedImport),
            "ModuleNode" => Ok(Self::ModuleNode),
            "ModuleEdge" => Ok(Self::ModuleEdge),
            "WorkspaceRoot" => Ok(Self::WorkspaceRoot),
            "TopologyPackage" => Ok(Self::TopologyPackage),
            "SourceSet" => Ok(Self::SourceSet),
            "DependencyRequirement" => Ok(Self::DependencyRequirement),
            "ResolvedDependencyEdge" => Ok(Self::ResolvedDependencyEdge),
            "ImportToPackage" => Ok(Self::ImportToPackage),
            "RepoTopologyOverlay" => Ok(Self::RepoTopologyOverlay),
            "Scope" => Ok(Self::Scope),
            "SemanticImport" => Ok(Self::SemanticImport),
            "Export" => Ok(Self::Export),
            "Alias" => Ok(Self::Alias),
            "Resolution" => Ok(Self::Resolution),
            "GeneratedSymbol" => Ok(Self::GeneratedSymbol),
            "StableExport" => Ok(Self::StableExport),
            "Symbol" => Ok(Self::Symbol),
            "Definition" => Ok(Self::Definition),
            "Reference" => Ok(Self::Reference),
            "FileMetric" => Ok(Self::FileMetric),
            "FunctionMetric" => Ok(Self::FunctionMetric),
            "ComplexityMetric" => Ok(Self::ComplexityMetric),
            "Place" => Ok(Self::Place),
            "MirBody" => Ok(Self::MirBody),
            "MirOperation" => Ok(Self::MirOperation),
            "CfgFunction" => Ok(Self::CfgFunction),
            "CfgNode" => Ok(Self::CfgNode),
            "BasicBlock" => Ok(Self::BasicBlock),
            "CfgEdge" => Ok(Self::CfgEdge),
            "CfgReachability" => Ok(Self::CfgReachability),
            "CfgDominator" => Ok(Self::CfgDominator),
            "CfgPostDominator" => Ok(Self::CfgPostDominator),
            "CfgControlDependence" => Ok(Self::CfgControlDependence),
            "UnsupportedControlFlow" => Ok(Self::UnsupportedControlFlow),
            "CallSite" => Ok(Self::CallSite),
            "CallTarget" => Ok(Self::CallTarget),
            "UnresolvedCall" => Ok(Self::UnresolvedCall),
            "RefinedCallEdge" => Ok(Self::RefinedCallEdge),
            "DataFlowNode" => Ok(Self::DataFlowNode),
            "DataFlowEdge" => Ok(Self::DataFlowEdge),
            "DataFlowModel" => Ok(Self::DataFlowModel),
            "DataFlowBudget" => Ok(Self::DataFlowBudget),
            "EvidenceNode" => Ok(Self::EvidenceNode),
            "EvidenceEdge" => Ok(Self::EvidenceEdge),
            "EvidenceBundle" => Ok(Self::EvidenceBundle),
            "EvidencePath" => Ok(Self::EvidencePath),
            "EvidenceSlice" => Ok(Self::EvidenceSlice),
            "EvidenceUnknown" => Ok(Self::EvidenceUnknown),
            "EvidenceOmittedRegion" => Ok(Self::EvidenceOmittedRegion),
            "EvidenceReplayKey" => Ok(Self::EvidenceReplayKey),
            "DomainObservation" => Ok(Self::DomainObservation),
            "DomainEvent" => Ok(Self::DomainEvent),
            "SummaryControl" => Ok(Self::SummaryControl),
            "SummaryCall" => Ok(Self::SummaryCall),
            "SummaryMemory" => Ok(Self::SummaryMemory),
            "SummaryTito" => Ok(Self::SummaryTito),
            "SummaryEvent" => Ok(Self::SummaryEvent),
            "ExtensionFact" => Ok(Self::ExtensionFact),
            "Entrypoint" => Ok(Self::Entrypoint),
            "TrustBoundary" => Ok(Self::TrustBoundary),
            "DispatchEdge" => Ok(Self::DispatchEdge),
            "UnresolvedFramework" => Ok(Self::UnresolvedFramework),
            "Type" => Ok(Self::Type),
            "NarrowedType" => Ok(Self::NarrowedType),
            "Value" => Ok(Self::Value),
            "AllocationToken" => Ok(Self::AllocationToken),
            "AccessPath" => Ok(Self::AccessPath),
            "PointsToConstraint" => Ok(Self::PointsToConstraint),
            "PointsToSet" => Ok(Self::PointsToSet),
            "AliasAnswer" => Ok(Self::AliasAnswer),
            "AdaptationModel" => Ok(Self::AdaptationModel),
            "SolverDerivedEdge" => Ok(Self::SolverDerivedEdge),
            "TypeValueAliasEvent" => Ok(Self::TypeValueAliasEvent),
            "MirStatement" => Ok(Self::MirStatement),
            "MirTerminator" => Ok(Self::MirTerminator),
            "UnsupportedSemantic" => Ok(Self::UnsupportedSemantic),
            _ => Err(UnknownFactFamilyLabel {
                label: label.to_string(),
            }),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct FactRef {
    pub(crate) family: FactFamily,
    pub(crate) run_id: u64,
}

impl FactRef {
    pub(crate) fn new(family: FactFamily, run_id: u64) -> Self {
        Self { family, run_id }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct MissingFactMeta {
    pub(crate) family: FactFamily,
    pub(crate) run_id: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FactMeta {
    pub(crate) stable_key: String,
    pub(crate) producer_id: &'static str,
    pub(crate) layer_id: &'static str,
    pub(crate) precision: FactPrecision,
    pub(crate) confidence: FactConfidence,
    pub(crate) validation: ValidationStatus,
    pub(crate) payload_digest: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum FactPrecision {
    Exact,
    Syntax,
    SetupAware,
    Heuristic,
    Unresolved,
    Ambiguous,
    SetupMissing,
    Unsupported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum FactConfidence {
    High,
    Medium,
    Low,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "closed validation metadata retains reader-only statuses"
    )
)]
pub(crate) enum ValidationStatus {
    NativeTrusted,
    SchemaValidated,
    ReferentiallyValidated,
    SpanValidated,
    StableKeyValidated,
    ConflictRejected,
}

macro_rules! define_unknown_label_error {
    ($name:ident, $description:literal) => {
        #[cfg_attr(
            not(test),
            allow(
                dead_code,
                reason = "canonical fact metadata decoding is consumed by private metadata readers"
            )
        )]
        #[derive(Clone, Debug, PartialEq, Eq)]
        pub(crate) struct $name {
            label: String,
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(
                    formatter,
                    concat!("unknown ", $description, " label `{}`"),
                    self.label
                )
            }
        }

        impl std::error::Error for $name {}
    };
}

define_unknown_label_error!(UnknownFactPrecisionLabel, "fact precision");
define_unknown_label_error!(UnknownFactConfidenceLabel, "fact confidence");
define_unknown_label_error!(UnknownValidationStatusLabel, "validation status");

impl FactPrecision {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Exact => "exact",
            Self::Syntax => "syntax",
            Self::SetupAware => "setup_aware",
            Self::Heuristic => "heuristic",
            Self::Unresolved => "unresolved",
            Self::Ambiguous => "ambiguous",
            Self::SetupMissing => "setup_missing",
            Self::Unsupported => "unsupported",
        }
    }

    #[cfg_attr(
        not(test),
        allow(
            dead_code,
            reason = "canonical fact metadata codecs stay symmetric for durable row decoding"
        )
    )]
    pub(crate) fn parse_label(label: &str) -> Result<Self, UnknownFactPrecisionLabel> {
        match label {
            "exact" => Ok(Self::Exact),
            "syntax" => Ok(Self::Syntax),
            "setup_aware" => Ok(Self::SetupAware),
            "heuristic" => Ok(Self::Heuristic),
            "unresolved" => Ok(Self::Unresolved),
            "ambiguous" => Ok(Self::Ambiguous),
            "setup_missing" => Ok(Self::SetupMissing),
            "unsupported" => Ok(Self::Unsupported),
            _ => Err(UnknownFactPrecisionLabel {
                label: label.to_string(),
            }),
        }
    }
}

impl FactConfidence {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::High => "high",
            Self::Medium => "medium",
            Self::Low => "low",
        }
    }

    #[cfg_attr(
        not(test),
        allow(
            dead_code,
            reason = "canonical fact metadata codecs stay symmetric for durable row decoding"
        )
    )]
    pub(crate) fn parse_label(label: &str) -> Result<Self, UnknownFactConfidenceLabel> {
        match label {
            "high" => Ok(Self::High),
            "medium" => Ok(Self::Medium),
            "low" => Ok(Self::Low),
            _ => Err(UnknownFactConfidenceLabel {
                label: label.to_string(),
            }),
        }
    }
}

impl ValidationStatus {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::NativeTrusted => "native_trusted",
            Self::SchemaValidated => "schema_validated",
            Self::ReferentiallyValidated => "referentially_validated",
            Self::SpanValidated => "span_validated",
            Self::StableKeyValidated => "stable_key_validated",
            Self::ConflictRejected => "conflict_rejected",
        }
    }

    #[cfg_attr(
        not(test),
        allow(
            dead_code,
            reason = "canonical fact metadata codecs stay symmetric for durable row decoding"
        )
    )]
    pub(crate) fn parse_label(label: &str) -> Result<Self, UnknownValidationStatusLabel> {
        match label {
            "native_trusted" => Ok(Self::NativeTrusted),
            "schema_validated" => Ok(Self::SchemaValidated),
            "referentially_validated" => Ok(Self::ReferentiallyValidated),
            "span_validated" => Ok(Self::SpanValidated),
            "stable_key_validated" => Ok(Self::StableKeyValidated),
            "conflict_rejected" => Ok(Self::ConflictRejected),
            _ => Err(UnknownValidationStatusLabel {
                label: label.to_string(),
            }),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct StableFactMetaRow {
    pub(crate) family: FactFamily,
    pub(crate) stable_key: StableFactKey,
    pub(crate) producer_id: Cow<'static, str>,
    pub(crate) layer_id: Cow<'static, str>,
    pub(crate) precision: FactPrecision,
    pub(crate) confidence: FactConfidence,
    pub(crate) validation: ValidationStatus,
    pub(crate) payload_digest: String,
}

impl serde::Serialize for StableFactMetaRow {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct as _;

        let mut row = serializer.serialize_struct("StableFactMetaRow", 8)?;
        row.serialize_field("family", self.family.label())?;
        row.serialize_field("stable_key", &self.stable_key)?;
        row.serialize_field("producer_id", &self.producer_id)?;
        row.serialize_field("layer_id", &self.layer_id)?;
        row.serialize_field("precision", self.precision.label())?;
        row.serialize_field("confidence", self.confidence.label())?;
        row.serialize_field("validation", self.validation.label())?;
        row.serialize_field("payload_digest", &self.payload_digest)?;
        row.end()
    }
}

impl StableFactMetaRow {
    fn from_metadata(family: FactFamily, metadata: &FactMeta) -> Self {
        Self {
            family,
            stable_key: metadata.stable_key.clone().into(),
            producer_id: Cow::Borrowed(metadata.producer_id),
            layer_id: Cow::Borrowed(metadata.layer_id),
            precision: metadata.precision,
            confidence: metadata.confidence,
            validation: metadata.validation,
            payload_digest: metadata.payload_digest.clone(),
        }
    }

    #[cfg(test)]
    fn from_metadata_compact(family: FactFamily, metadata: &FactMeta) -> Self {
        Self {
            family,
            stable_key: StableFactKey::compressed(&metadata.stable_key),
            producer_id: Cow::Borrowed(metadata.producer_id),
            layer_id: Cow::Borrowed(metadata.layer_id),
            precision: metadata.precision,
            confidence: metadata.confidence,
            validation: metadata.validation,
            payload_digest: metadata.payload_digest.clone(),
        }
    }

    fn has_same_identity(&self, other: &Self) -> bool {
        self.family == other.family && self.stable_key == other.stable_key
    }
}

const COMPRESSED_STABLE_KEY_PREFIX: &[u8] = b"polint-lz4-v1\0";
const MAX_STABLE_KEY_BYTES: usize = 64 * 1024 * 1024;

#[derive(Clone)]
pub(crate) enum StableFactKey {
    Plain(String),
    Compressed { encoded: Vec<u8>, json_bytes: u64 },
}

impl StableFactKey {
    pub(crate) fn compressed(stable_key: &str) -> Self {
        let compressed = lz4_flex::compress_prepend_size(stable_key.as_bytes());
        let mut encoded = Vec::with_capacity(COMPRESSED_STABLE_KEY_PREFIX.len() + compressed.len());
        encoded.extend_from_slice(COMPRESSED_STABLE_KEY_PREFIX);
        encoded.extend_from_slice(&compressed);
        Self::Compressed {
            encoded,
            json_bytes: serialized_string_bytes(stable_key),
        }
    }

    fn compressed_with_scratch(
        stable_key: &str,
        scratch: &mut Vec<u8>,
        table: &mut lz4_flex::block::CompressTable,
    ) -> Self {
        let input = stable_key.as_bytes();
        let maximum_output = lz4_flex::block::get_maximum_output_size(input.len());
        scratch.resize(maximum_output, 0);
        let compressed_len = lz4_flex::block::compress_into_with_table(input, scratch, table)
            .expect("maximum-size LZ4 scratch buffer accepts compressed stable key");
        let input_len = u32::try_from(input.len())
            .expect("validated stable keys fit the LZ4 size-prefixed representation");
        let mut encoded =
            Vec::with_capacity(COMPRESSED_STABLE_KEY_PREFIX.len() + 4 + compressed_len);
        encoded.extend_from_slice(COMPRESSED_STABLE_KEY_PREFIX);
        encoded.extend_from_slice(&input_len.to_le_bytes());
        encoded.extend_from_slice(&scratch[..compressed_len]);
        Self::Compressed {
            encoded,
            json_bytes: serialized_string_bytes(stable_key),
        }
    }

    pub(crate) fn from_storage(encoded: Vec<u8>) -> Result<Self, ()> {
        let Some(compressed) = encoded.strip_prefix(COMPRESSED_STABLE_KEY_PREFIX) else {
            let stable_key = String::from_utf8(encoded).map_err(|_| ())?;
            return Ok(Self::Plain(stable_key));
        };
        let declared_size = compressed
            .get(..4)
            .and_then(|bytes| <[u8; 4]>::try_from(bytes).ok())
            .map(u32::from_le_bytes)
            .and_then(|size| usize::try_from(size).ok())
            .ok_or(())?;
        if declared_size == 0 || declared_size > MAX_STABLE_KEY_BYTES {
            return Err(());
        }
        let decoded = lz4_flex::decompress_size_prepended(compressed).map_err(|_| ())?;
        let stable_key = std::str::from_utf8(&decoded).map_err(|_| ())?;
        Ok(Self::Compressed {
            encoded,
            json_bytes: serialized_string_bytes(stable_key),
        })
    }

    pub(crate) fn decoded(&self) -> Cow<'_, str> {
        match self {
            Self::Plain(stable_key) => Cow::Borrowed(stable_key),
            Self::Compressed { encoded, .. } => {
                let compressed = encoded
                    .strip_prefix(COMPRESSED_STABLE_KEY_PREFIX)
                    .expect("compressed stable-key representation has its codec prefix");
                let decoded = lz4_flex::decompress_size_prepended(compressed)
                    .expect("compressed stable-key representation was validated");
                Cow::Owned(
                    String::from_utf8(decoded)
                        .expect("compressed stable-key representation is valid UTF-8"),
                )
            }
        }
    }

    pub(crate) fn storage_bytes(&self) -> Cow<'_, [u8]> {
        match self {
            Self::Plain(stable_key) => match Self::compressed(stable_key) {
                Self::Compressed { encoded, .. } => Cow::Owned(encoded),
                Self::Plain(_) => unreachable!("compression always returns encoded bytes"),
            },
            Self::Compressed { encoded, .. } => Cow::Borrowed(encoded),
        }
    }

    pub(crate) fn json_bytes(&self) -> u64 {
        match self {
            Self::Plain(stable_key) => serialized_string_bytes(stable_key),
            Self::Compressed { json_bytes, .. } => *json_bytes,
        }
    }

    pub(crate) fn is_empty(&self) -> bool {
        match self {
            Self::Plain(stable_key) => stable_key.is_empty(),
            Self::Compressed { encoded, .. } => {
                encoded
                    .get(COMPRESSED_STABLE_KEY_PREFIX.len()..COMPRESSED_STABLE_KEY_PREFIX.len() + 4)
                    .and_then(|bytes| <[u8; 4]>::try_from(bytes).ok())
                    .map(u32::from_le_bytes)
                    == Some(0)
            }
        }
    }
}

fn serialized_string_bytes(value: &str) -> u64 {
    value.as_bytes().iter().fold(2_u64, |total, byte| {
        total.saturating_add(match byte {
            b'"' | b'\\' | b'\x08' | b'\x0c' | b'\n' | b'\r' | b'\t' => 2,
            0x00..=0x1f => 6,
            _ => 1,
        })
    })
}

impl From<String> for StableFactKey {
    fn from(value: String) -> Self {
        Self::Plain(value)
    }
}

impl From<&str> for StableFactKey {
    fn from(value: &str) -> Self {
        Self::Plain(value.to_string())
    }
}

impl fmt::Debug for StableFactKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("StableFactKey")
            .field(&self.decoded())
            .finish()
    }
}

impl fmt::Display for StableFactKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.decoded())
    }
}

impl PartialEq for StableFactKey {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Plain(left), Self::Plain(right)) => left == right,
            (Self::Compressed { encoded: left, .. }, Self::Compressed { encoded: right, .. }) => {
                left == right
            }
            _ => self.decoded() == other.decoded(),
        }
    }
}

impl Eq for StableFactKey {}

impl PartialOrd for StableFactKey {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for StableFactKey {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.decoded().cmp(&other.decoded())
    }
}

impl Hash for StableFactKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.decoded().hash(state);
    }
}

impl serde::Serialize for StableFactKey {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.decoded())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StableFactMetaConflict {
    pub(crate) family: FactFamily,
    pub(crate) stable_key: String,
    pub(crate) existing: Box<StableFactMetaRow>,
    pub(crate) incoming: Box<StableFactMetaRow>,
}

impl fmt::Display for StableFactMetaConflict {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "conflicting stable fact metadata for {} `{}`",
            self.family.label(),
            self.stable_key
        )
    }
}

impl std::error::Error for StableFactMetaConflict {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StableKeyOwner {
    pub(crate) reference: FactRef,
    pub(crate) payload_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct StableKeyConflict {
    pub(crate) family: FactFamily,
    pub(crate) stable_key: String,
    pub(crate) existing: FactRef,
    pub(crate) incoming: FactRef,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum FactMetaInsert {
    Inserted,
    Idempotent,
    Conflict {
        family: FactFamily,
        stable_key: String,
        existing: FactRef,
        incoming: FactRef,
    },
}

#[derive(Debug, Default, Clone)]
struct FactFamilyRows {
    dense: Vec<Option<FactMeta>>,
    sparse: BTreeMap<u64, FactMeta>,
}

impl FactFamilyRows {
    const DENSE_GAP_LIMIT: usize = 1024;

    fn insert(&mut self, run_id: u64, meta: FactMeta) {
        if let Ok(index) = usize::try_from(run_id)
            && index <= self.dense.len().saturating_add(Self::DENSE_GAP_LIMIT)
        {
            if self.dense.len() <= index {
                self.dense.resize_with(index + 1, || None);
            }
            self.dense[index] = Some(meta);
            self.sparse.remove(&run_id);
        } else {
            self.sparse.insert(run_id, meta);
        }
    }

    #[cfg(test)]
    fn remove(&mut self, run_id: u64) -> Option<FactMeta> {
        if let Ok(index) = usize::try_from(run_id)
            && let Some(row) = self.dense.get_mut(index)
        {
            return row.take();
        }
        self.sparse.remove(&run_id)
    }

    fn get(&self, run_id: u64) -> Option<&FactMeta> {
        if let Ok(index) = usize::try_from(run_id)
            && let Some(row) = self.dense.get(index)
        {
            return row.as_ref();
        }
        self.sparse.get(&run_id)
    }

    fn rows(&self) -> impl Iterator<Item = (u64, &FactMeta)> {
        self.dense
            .iter()
            .enumerate()
            .filter_map(|(run_id, metadata)| {
                metadata.as_ref().map(|metadata| (run_id as u64, metadata))
            })
            .chain(
                self.sparse
                    .iter()
                    .map(|(run_id, metadata)| (*run_id, metadata)),
            )
    }

    fn into_rows(self) -> impl Iterator<Item = (u64, FactMeta)> {
        self.dense
            .into_iter()
            .enumerate()
            .filter_map(|(run_id, metadata)| metadata.map(|metadata| (run_id as u64, metadata)))
            .chain(self.sparse)
    }
}

#[derive(Debug, Default, Clone)]
pub(crate) struct FactMetaStore {
    rows: BTreeMap<FactFamily, FactFamilyRows>,
    stable_key_owners: BTreeMap<FactFamily, HashMap<String, StableKeyOwner>>,
    stable_key_conflicts: BTreeSet<StableKeyConflict>,
}

impl FactMetaStore {
    pub(crate) fn insert(&mut self, reference: FactRef, meta: FactMeta) -> FactMetaInsert {
        let family_owners = self.stable_key_owners.entry(reference.family).or_default();
        let result = if let Some(owner) = family_owners.get(&meta.stable_key) {
            if owner.reference == reference && owner.payload_digest == meta.payload_digest {
                FactMetaInsert::Idempotent
            } else {
                let conflict = StableKeyConflict {
                    family: reference.family,
                    stable_key: meta.stable_key.clone(),
                    existing: owner.reference,
                    incoming: reference,
                };
                self.stable_key_conflicts.insert(conflict.clone());
                FactMetaInsert::Conflict {
                    family: conflict.family,
                    stable_key: conflict.stable_key,
                    existing: conflict.existing,
                    incoming: conflict.incoming,
                }
            }
        } else {
            family_owners.insert(
                meta.stable_key.clone(),
                StableKeyOwner {
                    reference,
                    payload_digest: meta.payload_digest.clone(),
                },
            );
            FactMetaInsert::Inserted
        };

        self.rows
            .entry(reference.family)
            .or_default()
            .insert(reference.run_id, meta);
        result
    }

    pub(crate) fn remove_family(&mut self, family: FactFamily) {
        self.rows.remove(&family);
        self.stable_key_owners.remove(&family);

        let conflict_start = StableKeyConflict {
            family,
            stable_key: String::new(),
            existing: FactRef::new(FactFamily::SourceFile, 0),
            incoming: FactRef::new(FactFamily::SourceFile, 0),
        };
        let conflicts = self
            .stable_key_conflicts
            .range(conflict_start..)
            .take_while(|conflict| conflict.family == family)
            .cloned()
            .collect::<Vec<_>>();
        for conflict in conflicts {
            self.stable_key_conflicts.remove(&conflict);
        }
    }

    pub(crate) fn finish_family_insertions(&mut self, family: FactFamily) {
        self.stable_key_owners.remove(&family);
    }

    pub(crate) fn finish_all_insertions(&mut self) {
        self.stable_key_owners.clear();
    }

    #[cfg(test)]
    pub(crate) fn remove_for_test(&mut self, reference: FactRef) -> Option<FactMeta> {
        self.rows
            .get_mut(&reference.family)
            .and_then(|rows| rows.remove(reference.run_id))
    }

    pub(crate) fn get(&self, reference: FactRef) -> Option<&FactMeta> {
        self.rows
            .get(&reference.family)
            .and_then(|rows| rows.get(reference.run_id))
    }

    pub(crate) fn stable_key_conflicts(&self) -> impl Iterator<Item = &StableKeyConflict> {
        self.stable_key_conflicts.iter()
    }

    #[cfg(test)]
    pub(crate) fn stable_key_owner(
        &self,
        family: FactFamily,
        stable_key: &str,
    ) -> Option<&StableKeyOwner> {
        self.stable_key_owners
            .get(&family)
            .and_then(|owners| owners.get(stable_key))
    }

    pub(crate) fn rows(&self) -> impl Iterator<Item = (FactRef, &FactMeta)> {
        self.rows.iter().flat_map(|(family, rows)| {
            rows.rows()
                .map(move |(run_id, metadata)| (FactRef::new(*family, run_id), metadata))
        })
    }

    pub(crate) fn stable_rows(&self) -> Result<Vec<StableFactMetaRow>, StableFactMetaConflict> {
        canonical_stable_rows(self.rows().map(|(reference, metadata)| {
            StableFactMetaRow::from_metadata(reference.family, metadata)
        }))
    }

    #[cfg(test)]
    pub(crate) fn compact_stable_rows(
        &self,
    ) -> Result<(Vec<StableFactMetaRow>, Digest), StableFactMetaConflict> {
        let mut rows = self.rows().collect::<Vec<_>>();
        rows.sort_by(|(left_ref, left), (right_ref, right)| {
            left_ref
                .family
                .cmp(&right_ref.family)
                .then_with(|| left.stable_key.cmp(&right.stable_key))
                .then_with(|| left.producer_id.cmp(right.producer_id))
                .then_with(|| left.layer_id.cmp(right.layer_id))
                .then_with(|| left.precision.cmp(&right.precision))
                .then_with(|| left.confidence.cmp(&right.confidence))
                .then_with(|| left.validation.cmp(&right.validation))
                .then_with(|| left.payload_digest.cmp(&right.payload_digest))
        });
        let mut canonical_rows = Vec::with_capacity(rows.len());
        let mut prior: Option<(FactRef, &FactMeta)> = None;
        for (reference, metadata) in rows {
            if let Some((prior_ref, prior_metadata)) = prior
                && prior_ref.family == reference.family
                && prior_metadata.stable_key == metadata.stable_key
            {
                if prior_metadata == metadata {
                    continue;
                }
                return Err(StableFactMetaConflict {
                    family: reference.family,
                    stable_key: metadata.stable_key.clone(),
                    existing: Box::new(StableFactMetaRow::from_metadata(
                        prior_ref.family,
                        prior_metadata,
                    )),
                    incoming: Box::new(StableFactMetaRow::from_metadata(
                        reference.family,
                        metadata,
                    )),
                });
            }
            canonical_rows.push((reference, metadata));
            prior = Some((reference, metadata));
        }
        let (canonical, row_digests): (Vec<_>, Vec<_>) = canonical_rows
            .par_iter()
            .map(|(reference, metadata)| {
                let digest = Digest::from_parts(
                    DigestKind::FactMetadata,
                    "fact_metadata_row",
                    &[
                        reference.family.label(),
                        &metadata.stable_key,
                        metadata.producer_id,
                        metadata.layer_id,
                        metadata.precision.label(),
                        metadata.confidence.label(),
                        metadata.validation.label(),
                        &metadata.payload_digest,
                    ],
                );
                (
                    StableFactMetaRow::from_metadata_compact(reference.family, metadata),
                    digest,
                )
            })
            .unzip();
        Ok((
            canonical,
            Digest::from_unordered(DigestKind::FactMetadata, "fact_metadata_rows", row_digests),
        ))
    }

    #[cfg(test)]
    pub(crate) fn into_compact_stable_rows(
        self,
    ) -> Result<(Vec<StableFactMetaRow>, Digest), StableFactMetaConflict> {
        Ok(self.prepare_compact_stable_rows()?.finish())
    }

    pub(crate) fn prepare_compact_stable_rows(
        self,
    ) -> Result<PreparedCompactStableRows, StableFactMetaConflict> {
        let Self {
            rows,
            stable_key_owners,
            stable_key_conflicts,
        } = self;
        drop(stable_key_owners);
        drop(stable_key_conflicts);

        let mut rows = rows
            .into_iter()
            .flat_map(|(family, rows)| {
                rows.into_rows()
                    .map(move |(_, metadata)| StableFactMetaRow {
                        family,
                        stable_key: StableFactKey::Plain(metadata.stable_key),
                        producer_id: Cow::Borrowed(metadata.producer_id),
                        layer_id: Cow::Borrowed(metadata.layer_id),
                        precision: metadata.precision,
                        confidence: metadata.confidence,
                        validation: metadata.validation,
                        payload_digest: metadata.payload_digest,
                    })
            })
            .collect::<Vec<_>>();
        rows.par_sort_unstable_by(|left, right| {
            let StableFactKey::Plain(left_stable_key) = &left.stable_key else {
                unreachable!("newly materialized fact rows retain plain stable keys")
            };
            let StableFactKey::Plain(right_stable_key) = &right.stable_key else {
                unreachable!("newly materialized fact rows retain plain stable keys")
            };
            left.family
                .cmp(&right.family)
                .then_with(|| left_stable_key.cmp(right_stable_key))
                .then_with(|| left.producer_id.cmp(&right.producer_id))
                .then_with(|| left.layer_id.cmp(&right.layer_id))
                .then_with(|| left.precision.cmp(&right.precision))
                .then_with(|| left.confidence.cmp(&right.confidence))
                .then_with(|| left.validation.cmp(&right.validation))
                .then_with(|| left.payload_digest.cmp(&right.payload_digest))
        });

        for pair in rows.windows(2) {
            let prior = &pair[0];
            let incoming = &pair[1];
            if prior.has_same_identity(incoming) && prior != incoming {
                return Err(StableFactMetaConflict {
                    family: incoming.family,
                    stable_key: incoming.stable_key.decoded().into_owned(),
                    existing: Box::new(prior.clone()),
                    incoming: Box::new(incoming.clone()),
                });
            }
        }
        rows.dedup_by(|right, left| right.has_same_identity(left));

        Ok(PreparedCompactStableRows { rows })
    }
}

pub(crate) struct PreparedCompactStableRows {
    rows: Vec<StableFactMetaRow>,
}

pub(in crate::analysis_kernel) struct FinalizedCanonicalFactRows {
    rows: Vec<StableFactMetaRow>,
    digest: Digest,
}

impl FinalizedCanonicalFactRows {
    pub(in crate::analysis_kernel) fn into_parts(self) -> (Vec<StableFactMetaRow>, Digest) {
        (self.rows, self.digest)
    }
}

impl PreparedCompactStableRows {
    #[cfg(test)]
    pub(crate) fn finish(self) -> (Vec<StableFactMetaRow>, Digest) {
        self.finish_parts()
    }

    pub(in crate::analysis_kernel) fn finish_validated(self) -> FinalizedCanonicalFactRows {
        let (rows, digest) = self.finish_parts();
        FinalizedCanonicalFactRows { rows, digest }
    }

    fn finish_parts(mut self) -> (Vec<StableFactMetaRow>, Digest) {
        let row_fingerprints = self
            .rows
            .par_iter_mut()
            .map_init(
                || (Vec::new(), lz4_flex::block::CompressTable::default()),
                |(compression_scratch, compression_table), row| {
                    let StableFactKey::Plain(stable_key) = &row.stable_key else {
                        unreachable!("newly materialized fact rows retain plain stable keys")
                    };
                    let fingerprint = Digest::fingerprint_from_parts(
                        DigestKind::FactMetadata,
                        "fact_metadata_row",
                        &[
                            row.family.label(),
                            stable_key,
                            &row.producer_id,
                            &row.layer_id,
                            row.precision.label(),
                            row.confidence.label(),
                            row.validation.label(),
                            &row.payload_digest,
                        ],
                    );
                    let compressed = StableFactKey::compressed_with_scratch(
                        stable_key,
                        compression_scratch,
                        compression_table,
                    );
                    row.stable_key = compressed;
                    fingerprint
                },
            )
            .collect();
        let digest = Digest::from_unordered_same_kind_fingerprints(
            DigestKind::FactMetadata,
            "fact_metadata_rows",
            DigestKind::FactMetadata,
            row_fingerprints,
        );
        (self.rows, digest)
    }
}

fn canonical_stable_rows(
    rows: impl IntoIterator<Item = StableFactMetaRow>,
) -> Result<Vec<StableFactMetaRow>, StableFactMetaConflict> {
    let mut rows = rows.into_iter().collect::<Vec<_>>();
    rows.sort();
    let mut canonical = Vec::<StableFactMetaRow>::with_capacity(rows.len());
    for row in rows {
        if let Some(existing) = canonical.last()
            && existing.has_same_identity(&row)
        {
            if existing == &row {
                continue;
            }
            return Err(StableFactMetaConflict {
                family: row.family,
                stable_key: row.stable_key.decoded().into_owned(),
                existing: Box::new(existing.clone()),
                incoming: Box::new(row),
            });
        }
        canonical.push(row);
    }
    Ok(canonical)
}

pub(crate) fn stable_key_from_parts(family: FactFamily, parts: &[(&str, String)]) -> String {
    let mut normalized = parts
        .iter()
        .map(|(label, value)| (*label, value.replace('\\', "/")))
        .collect::<Vec<_>>();
    normalized.sort_by(|left, right| left.0.cmp(right.0));

    let mut key = length_prefixed(family.label());
    for (label, value) in normalized {
        key.push('|');
        key.push_str(&length_prefixed(label));
        key.push('=');
        key.push_str(&length_prefixed(&value));
    }
    key
}

pub(crate) fn resolution_metadata(
    precision: ResolutionPrecision,
    status: ResolutionStatus,
) -> (FactPrecision, FactConfidence) {
    if matches!(status, ResolutionStatus::Dynamic) {
        return (FactPrecision::Heuristic, FactConfidence::Low);
    }

    match precision {
        ResolutionPrecision::ExactFile => (FactPrecision::SetupAware, FactConfidence::High),
        ResolutionPrecision::Package | ResolutionPrecision::ExternalPackage => {
            (FactPrecision::SetupAware, FactConfidence::High)
        }
        ResolutionPrecision::Heuristic => (FactPrecision::Heuristic, FactConfidence::Medium),
        ResolutionPrecision::None => resolution_status_metadata(status),
    }
}

pub(crate) fn resolution_status_metadata(
    status: ResolutionStatus,
) -> (FactPrecision, FactConfidence) {
    match status {
        ResolutionStatus::Resolved | ResolutionStatus::External => {
            (FactPrecision::SetupAware, FactConfidence::High)
        }
        ResolutionStatus::Unresolved => (FactPrecision::Unresolved, FactConfidence::Low),
        ResolutionStatus::SetupMissing => (FactPrecision::SetupMissing, FactConfidence::Low),
        ResolutionStatus::Dynamic => (FactPrecision::Heuristic, FactConfidence::Low),
        ResolutionStatus::Unsupported => (FactPrecision::Unsupported, FactConfidence::Low),
    }
}

pub(crate) fn symbol_metadata(precision: SymbolPrecision) -> (FactPrecision, FactConfidence) {
    match precision {
        SymbolPrecision::ExactSemantic => (FactPrecision::SetupAware, FactConfidence::High),
        SymbolPrecision::ExactLocal => (FactPrecision::Syntax, FactConfidence::High),
        SymbolPrecision::ModuleLinked => (FactPrecision::SetupAware, FactConfidence::High),
        SymbolPrecision::Heuristic => (FactPrecision::Heuristic, FactConfidence::Medium),
        SymbolPrecision::Unresolved => (FactPrecision::Unresolved, FactConfidence::Low),
        SymbolPrecision::Ambiguous => (FactPrecision::Ambiguous, FactConfidence::Low),
        SymbolPrecision::SetupMissing => (FactPrecision::SetupMissing, FactConfidence::Low),
        SymbolPrecision::Unsupported => (FactPrecision::Unsupported, FactConfidence::Low),
    }
}

fn length_prefixed(value: &str) -> String {
    format!("{}:{value}", value.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL_FACT_FAMILIES: &[FactFamily] = &[
        FactFamily::SourceFile,
        FactFamily::Package,
        FactFamily::Function,
        FactFamily::Import,
        FactFamily::BranchObligation,
        FactFamily::Test,
        FactFamily::Coverage,
        FactFamily::TsComponent,
        FactFamily::TsClass,
        FactFamily::StringLiteral,
        FactFamily::JsxAttribute,
        FactFamily::ResolvedImport,
        FactFamily::ModuleNode,
        FactFamily::ModuleEdge,
        FactFamily::WorkspaceRoot,
        FactFamily::TopologyPackage,
        FactFamily::SourceSet,
        FactFamily::DependencyRequirement,
        FactFamily::ResolvedDependencyEdge,
        FactFamily::ImportToPackage,
        FactFamily::RepoTopologyOverlay,
        FactFamily::Scope,
        FactFamily::SemanticImport,
        FactFamily::Export,
        FactFamily::Alias,
        FactFamily::Resolution,
        FactFamily::GeneratedSymbol,
        FactFamily::StableExport,
        FactFamily::Symbol,
        FactFamily::Definition,
        FactFamily::Reference,
        FactFamily::FileMetric,
        FactFamily::FunctionMetric,
        FactFamily::ComplexityMetric,
        FactFamily::Place,
        FactFamily::MirBody,
        FactFamily::MirOperation,
        FactFamily::CfgFunction,
        FactFamily::CfgNode,
        FactFamily::BasicBlock,
        FactFamily::CfgEdge,
        FactFamily::CfgReachability,
        FactFamily::CfgDominator,
        FactFamily::CfgPostDominator,
        FactFamily::CfgControlDependence,
        FactFamily::UnsupportedControlFlow,
        FactFamily::CallSite,
        FactFamily::CallTarget,
        FactFamily::UnresolvedCall,
        FactFamily::RefinedCallEdge,
        FactFamily::DataFlowNode,
        FactFamily::DataFlowEdge,
        FactFamily::DataFlowModel,
        FactFamily::DataFlowBudget,
        FactFamily::EvidenceNode,
        FactFamily::EvidenceEdge,
        FactFamily::EvidenceBundle,
        FactFamily::EvidencePath,
        FactFamily::EvidenceSlice,
        FactFamily::EvidenceUnknown,
        FactFamily::EvidenceOmittedRegion,
        FactFamily::EvidenceReplayKey,
        FactFamily::DomainObservation,
        FactFamily::DomainEvent,
        FactFamily::SummaryControl,
        FactFamily::SummaryCall,
        FactFamily::SummaryMemory,
        FactFamily::SummaryTito,
        FactFamily::SummaryEvent,
        FactFamily::ExtensionFact,
        FactFamily::Entrypoint,
        FactFamily::TrustBoundary,
        FactFamily::DispatchEdge,
        FactFamily::UnresolvedFramework,
        FactFamily::Type,
        FactFamily::NarrowedType,
        FactFamily::Value,
        FactFamily::AllocationToken,
        FactFamily::AccessPath,
        FactFamily::PointsToConstraint,
        FactFamily::PointsToSet,
        FactFamily::AliasAnswer,
        FactFamily::AdaptationModel,
        FactFamily::SolverDerivedEdge,
        FactFamily::TypeValueAliasEvent,
        FactFamily::MirStatement,
        FactFamily::MirTerminator,
        FactFamily::UnsupportedSemantic,
    ];

    #[test]
    fn fact_metadata_codecs_round_trip_every_variant() {
        for family in ALL_FACT_FAMILIES {
            assert_eq!(FactFamily::parse_label(family.label()), Ok(*family));
        }
        for precision in [
            FactPrecision::Exact,
            FactPrecision::Syntax,
            FactPrecision::SetupAware,
            FactPrecision::Heuristic,
            FactPrecision::Unresolved,
            FactPrecision::Ambiguous,
            FactPrecision::SetupMissing,
            FactPrecision::Unsupported,
        ] {
            assert_eq!(FactPrecision::parse_label(precision.label()), Ok(precision));
        }
        for confidence in [
            FactConfidence::High,
            FactConfidence::Medium,
            FactConfidence::Low,
        ] {
            assert_eq!(
                FactConfidence::parse_label(confidence.label()),
                Ok(confidence)
            );
        }
        for status in [
            ValidationStatus::NativeTrusted,
            ValidationStatus::SchemaValidated,
            ValidationStatus::ReferentiallyValidated,
            ValidationStatus::SpanValidated,
            ValidationStatus::StableKeyValidated,
            ValidationStatus::ConflictRejected,
        ] {
            assert_eq!(ValidationStatus::parse_label(status.label()), Ok(status));
        }
    }

    #[test]
    fn fact_metadata_codecs_reject_unknown_labels() {
        assert!(FactFamily::parse_label("source_file").is_err());
        assert!(FactPrecision::parse_label("setup-aware").is_err());
        assert!(FactConfidence::parse_label("certain").is_err());
        assert!(ValidationStatus::parse_label("trusted").is_err());
    }

    #[test]
    fn fact_ref_keeps_run_local_id_separate_from_stable_key() {
        let reference = FactRef::new(FactFamily::Import, 7);
        let metadata = FactMeta {
            stable_key: "import:stable".to_string(),
            producer_id: "polint.go.syntax",
            layer_id: "polint.go.syntax",
            precision: FactPrecision::Syntax,
            confidence: FactConfidence::High,
            validation: ValidationStatus::NativeTrusted,
            payload_digest: "payload".to_string(),
        };

        assert_eq!(reference.run_id, 7);
        assert_eq!(metadata.stable_key, "import:stable");
    }

    #[test]
    fn stable_key_from_parts_sorts_and_normalizes_length_prefixed_parts() {
        let first = stable_key_from_parts(
            FactFamily::Import,
            &[
                ("path", "src\\main.go".to_string()),
                ("import_path", "fmt".to_string()),
            ],
        );
        let second = stable_key_from_parts(
            FactFamily::Import,
            &[
                ("import_path", "fmt".to_string()),
                ("path", "src/main.go".to_string()),
            ],
        );

        assert_eq!(first, second);
        assert!(first.contains("6:Import"));
        assert!(first.contains("4:path=11:src/main.go"));
    }

    #[test]
    fn fact_meta_store_rows_are_deterministically_ordered() {
        let mut store = FactMetaStore::default();
        let later = FactRef::new(FactFamily::Import, 10);
        let earlier = FactRef::new(FactFamily::Import, 2);

        store.insert(later, test_meta("function", "function"));
        store.insert(earlier, test_meta("import", "import"));

        let rows = store.rows().collect::<Vec<_>>();

        assert_eq!(rows[0].0, earlier);
        assert_eq!(rows[1].0, later);
    }

    #[test]
    fn stable_rows_ignore_run_id_and_insertion_permutations() {
        let mut first = FactMetaStore::default();
        first.insert(
            FactRef::new(FactFamily::Import, 10),
            test_meta("import:b", "payload:b"),
        );
        first.insert(
            FactRef::new(FactFamily::Import, 2),
            test_meta("import:a", "payload:a"),
        );

        let mut second = FactMetaStore::default();
        second.insert(
            FactRef::new(FactFamily::Import, 90),
            test_meta("import:a", "payload:a"),
        );
        second.insert(
            FactRef::new(FactFamily::Import, 1),
            test_meta("import:b", "payload:b"),
        );

        assert_eq!(
            first.stable_rows().expect("first stable rows"),
            second.stable_rows().expect("second stable rows")
        );
    }

    #[test]
    fn compact_stable_rows_preserve_semantics_and_round_trip_storage() {
        let mut store = FactMetaStore::default();
        let stable_key = format!("import:{}", "repeated/segment/".repeat(64));
        store.insert(
            FactRef::new(FactFamily::Import, 7),
            test_meta(&stable_key, "payload:a"),
        );

        let plain = store.stable_rows().expect("plain stable rows");
        let (compact, compact_digest) = store.compact_stable_rows().expect("compact stable rows");
        let (staged, staged_digest) = store
            .clone()
            .prepare_compact_stable_rows()
            .expect("prepared compact stable rows")
            .finish();
        let (consumed, consumed_digest) = store
            .into_compact_stable_rows()
            .expect("consumed compact stable rows");
        let expected_digest = Digest::from_unordered(
            DigestKind::FactMetadata,
            "fact_metadata_rows",
            plain
                .iter()
                .map(|row| {
                    let decoded = row.stable_key.decoded();
                    Digest::from_parts(
                        DigestKind::FactMetadata,
                        "fact_metadata_row",
                        &[
                            row.family.label(),
                            &decoded,
                            &row.producer_id,
                            &row.layer_id,
                            row.precision.label(),
                            row.confidence.label(),
                            row.validation.label(),
                            &row.payload_digest,
                        ],
                    )
                })
                .collect(),
        );

        assert_eq!(compact, plain);
        assert_eq!(compact_digest, expected_digest);
        assert_eq!(staged, compact);
        assert_eq!(staged_digest, compact_digest);
        assert_eq!(consumed, compact);
        assert_eq!(consumed_digest, compact_digest);
        let StableFactKey::Compressed { encoded, .. } = &compact[0].stable_key else {
            panic!("compact rows must retain the encoded stable key")
        };
        assert!(encoded.starts_with(COMPRESSED_STABLE_KEY_PREFIX));
        assert!(encoded.len() < stable_key.len());
        assert_eq!(
            StableFactKey::from_storage(encoded.clone()).expect("encoded key round trips"),
            plain[0].stable_key
        );
    }

    #[test]
    fn scratch_compression_matches_size_prefixed_codec_byte_for_byte() {
        let table_boundary = (0..usize::from(u16::MAX) + 257)
            .map(|index| char::from(b' ' + (index % 95) as u8))
            .collect::<String>();
        let cases = [
            String::new(),
            "x".to_string(),
            "quote:\" slash:\\ newline:\n control:\u{1f}".to_string(),
            "repeated/segment/".repeat(256),
            table_boundary,
            "small key after table upgrade".to_string(),
        ];
        let mut scratch = Vec::new();
        let mut table = lz4_flex::block::CompressTable::default();

        for stable_key in cases {
            assert_eq!(
                StableFactKey::compressed_with_scratch(&stable_key, &mut scratch, &mut table,),
                StableFactKey::compressed(&stable_key),
                "stable key length {}",
                stable_key.len()
            );
        }
    }

    #[test]
    fn stable_rows_deduplicate_identical_semantic_rows() {
        let mut store = FactMetaStore::default();
        store.insert(
            FactRef::new(FactFamily::Import, 1),
            test_meta("import:key", "payload:a"),
        );
        store.insert(
            FactRef::new(FactFamily::Import, 99),
            test_meta("import:key", "payload:a"),
        );

        let rows = store.stable_rows().expect("identical rows deduplicate");

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].payload_digest, "payload:a");
    }

    #[test]
    fn stable_rows_reject_conflicting_semantic_rows() {
        let mut store = FactMetaStore::default();
        store.insert(
            FactRef::new(FactFamily::Import, 1),
            test_meta("import:key", "payload:a"),
        );
        store.insert(
            FactRef::new(FactFamily::Import, 2),
            test_meta("import:key", "payload:b"),
        );

        let conflict = store
            .stable_rows()
            .expect_err("different payload digests conflict");

        assert_eq!(conflict.family, FactFamily::Import);
        assert_eq!(conflict.stable_key, "import:key");
        assert_ne!(
            conflict.existing.payload_digest,
            conflict.incoming.payload_digest
        );

        let consumed_conflict = store
            .into_compact_stable_rows()
            .expect_err("consumed rows preserve the conflict");
        assert_eq!(consumed_conflict, conflict);
    }

    #[test]
    fn stable_key_conflict_insert_is_idempotent_when_same_reference_payload_repeats() {
        let mut store = FactMetaStore::default();
        let reference = FactRef::new(FactFamily::Import, 7);

        let first = store.insert(reference, test_meta("import:key", "payload:a"));
        let second = store.insert(reference, test_meta("import:key", "payload:a"));

        assert_eq!(first, FactMetaInsert::Inserted);
        assert_eq!(second, FactMetaInsert::Idempotent);
    }

    #[test]
    fn stable_key_conflict_insert_records_duplicate_reference_for_same_payload() {
        let mut store = FactMetaStore::default();
        let first_ref = FactRef::new(FactFamily::Import, 1);
        let second_ref = FactRef::new(FactFamily::Import, 2);

        store.insert(first_ref, test_meta("import:key", "payload:a"));
        let result = store.insert(second_ref, test_meta("import:key", "payload:a"));

        assert_eq!(
            result,
            FactMetaInsert::Conflict {
                family: FactFamily::Import,
                stable_key: "import:key".to_string(),
                existing: first_ref,
                incoming: second_ref,
            }
        );
        assert_eq!(store.stable_key_conflicts().count(), 1);
        assert_eq!(
            store
                .stable_key_owner(FactFamily::Import, "import:key")
                .map(|owner| owner.reference),
            Some(first_ref)
        );
    }

    #[test]
    fn finish_family_insertions_keeps_rows_and_conflicts_but_drops_owner_index() {
        let mut store = FactMetaStore::default();
        let first_ref = FactRef::new(FactFamily::Import, 1);
        let second_ref = FactRef::new(FactFamily::Import, 2);

        store.insert(first_ref, test_meta("import:key", "payload:a"));
        store.insert(second_ref, test_meta("import:key", "payload:b"));
        store.finish_family_insertions(FactFamily::Import);

        assert!(store.get(first_ref).is_some());
        assert!(store.get(second_ref).is_some());
        assert_eq!(store.stable_key_conflicts().count(), 1);
        assert!(
            store
                .stable_key_owner(FactFamily::Import, "import:key")
                .is_none()
        );
    }

    #[test]
    fn remove_family_drops_rows_owners_and_conflicts_for_that_family_only() {
        let mut store = FactMetaStore::default();
        let import_ref = FactRef::new(FactFamily::Import, 1);
        let duplicate_import_ref = FactRef::new(FactFamily::Import, 2);
        let function_ref = FactRef::new(FactFamily::Function, 1);

        store.insert(import_ref, test_meta("shared:key", "payload:a"));
        store.insert(duplicate_import_ref, test_meta("shared:key", "payload:b"));
        store.insert(function_ref, test_meta("function:key", "payload:c"));

        store.remove_family(FactFamily::Import);

        assert!(store.get(import_ref).is_none());
        assert!(store.get(duplicate_import_ref).is_none());
        assert!(
            store
                .stable_key_owner(FactFamily::Import, "shared:key")
                .is_none()
        );
        assert_eq!(store.stable_key_conflicts().count(), 0);
        assert!(store.get(function_ref).is_some());
        assert!(
            store
                .stable_key_owner(FactFamily::Function, "function:key")
                .is_some()
        );
    }

    #[test]
    fn fact_meta_store_accepts_sparse_high_run_ids_without_dense_resize() {
        let mut store = FactMetaStore::default();
        let high_ref = FactRef::new(FactFamily::Import, u64::MAX);
        let low_ref = FactRef::new(FactFamily::Import, 0);

        store.insert(high_ref, test_meta("import:high", "payload:high"));
        store.insert(low_ref, test_meta("import:low", "payload:low"));

        assert_eq!(store.get(high_ref).unwrap().stable_key, "import:high");
        assert_eq!(store.get(low_ref).unwrap().stable_key, "import:low");
        assert_eq!(
            store
                .rows()
                .map(|(reference, metadata)| (reference, metadata.stable_key.as_str()))
                .collect::<Vec<_>>(),
            vec![(low_ref, "import:low"), (high_ref, "import:high")]
        );
    }

    #[test]
    fn stable_key_conflict_insert_records_conflicts_deterministically() {
        let mut store = FactMetaStore::default();
        let import_owner_ref = FactRef::new(FactFamily::Import, 8);
        let import_conflict_ref = FactRef::new(FactFamily::Import, 3);

        store.insert(import_owner_ref, test_meta("import:key", "payload:a"));
        let result = store.insert(import_conflict_ref, test_meta("import:key", "payload:b"));
        store.insert(import_conflict_ref, test_meta("import:key", "payload:b"));

        let conflicts = store.stable_key_conflicts().collect::<Vec<_>>();

        assert_eq!(
            result,
            FactMetaInsert::Conflict {
                family: FactFamily::Import,
                stable_key: "import:key".to_string(),
                existing: import_owner_ref,
                incoming: import_conflict_ref,
            }
        );
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].family, FactFamily::Import);
        assert_eq!(conflicts[0].stable_key, "import:key");
        assert_eq!(conflicts[0].existing, import_owner_ref);
        assert_eq!(conflicts[0].incoming, import_conflict_ref);
    }

    fn test_meta(stable_key: &str, payload_digest: &str) -> FactMeta {
        FactMeta {
            stable_key: stable_key.to_string(),
            producer_id: "polint.go.syntax",
            layer_id: "polint.go.syntax",
            precision: FactPrecision::Syntax,
            confidence: FactConfidence::High,
            validation: ValidationStatus::NativeTrusted,
            payload_digest: payload_digest.to_string(),
        }
    }
}
