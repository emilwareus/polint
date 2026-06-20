//! Preview policy-query vocabulary for repo-local rules.
//!
//! These types define the public shape for v1.4 policy queries. They are
//! intentionally plain data structs with `new(required...)` constructors and
//! named option fields. Runtime query behavior is backed only for documented
//! preview scopes; unsupported patterns return no policy matches rather than
//! exposing internal graph APIs.

use crate::cache::stable_hash;
use crate::diagnostics::{Diagnostic, TextRange};

pub(crate) const POLICY_QUERY_VERSION: &str = "policy-query-preview-v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PolicyOperation {
    EventsMatching,
    CallsForbiddenReachable,
    ControlFlowMissingGuard,
    ControlFlowMissingCleanup,
    DataFlowForbidden,
}

impl PolicyOperation {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::EventsMatching => "events.matching",
            Self::CallsForbiddenReachable => "calls.forbidden_reachable",
            Self::ControlFlowMissingGuard => "control_flow.missing_guard",
            Self::ControlFlowMissingCleanup => "control_flow.missing_cleanup",
            Self::DataFlowForbidden => "data_flow.forbidden",
        }
    }
}

/// Result status for a policy-query match.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolicyStatus {
    /// The query result is exact for the supported scope.
    Exact,
    /// The query result is heuristic and must be described as such.
    Heuristic,
    /// The query could not be answered with the available analysis support.
    Unknown,
    /// The query family is known but not supported in the current phase.
    Unsupported,
    /// The query exceeded its configured budget.
    BudgetExceeded,
}

/// Precision level attached to a policy-query result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolicyPrecision {
    /// Exact semantic evidence.
    Exact,
    /// Setup-aware evidence that may depend on local lifecycle configuration.
    SetupAware,
    /// Syntax-level evidence.
    Syntax,
    /// Conservative evidence.
    Conservative,
    /// Heuristic evidence.
    Heuristic,
    /// Unknown precision.
    Unknown,
}

/// Confidence level attached to a policy-query result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolicyConfidence {
    /// High-confidence evidence from resolved or validated facts.
    High,
    /// Medium-confidence evidence from setup-aware or conservative facts.
    Medium,
    /// Low-confidence evidence from heuristic or unknown facts.
    Low,
}

/// Preview violation returned by policy-query views.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyViolation {
    query: PolicyOperation,
    query_digest: String,
    file: String,
    range: TextRange,
    status: PolicyStatus,
    precision: PolicyPrecision,
    evidence: Vec<(String, String)>,
}

impl PolicyViolation {
    pub(crate) fn new(
        query: PolicyOperation,
        query_digest: impl Into<String>,
        file: impl Into<String>,
        range: TextRange,
        status: PolicyStatus,
        precision: PolicyPrecision,
        evidence: Vec<(String, String)>,
    ) -> Self {
        Self {
            query,
            query_digest: query_digest.into(),
            file: file.into(),
            range,
            status,
            precision,
            evidence,
        }
    }

    pub(crate) fn set_status(&mut self, status: PolicyStatus) {
        self.status = status;
    }

    pub(crate) fn push_evidence(&mut self, label: impl Into<String>, value: impl Into<String>) {
        self.evidence.push((label.into(), value.into()));
    }

    pub(crate) fn stable_key(&self) -> String {
        let mut parts = vec![
            format!("query={}", encode_str(self.query.label())),
            format!("query_digest={}", encode_str(&self.query_digest)),
            format!("file={}", encode_str(&self.file)),
            format!("start_line={}", self.range.start_line),
            format!("start_col={}", self.range.start_col),
            format!("end_line={}", self.range.end_line),
            format!("end_col={}", self.range.end_col),
            format!("status={}", policy_status_label(self.status)),
            format!("precision={}", policy_precision_label(self.precision)),
        ];
        for (label, value) in &self.evidence {
            parts.push(format!(
                "evidence:{}={}",
                encode_str(label),
                encode_str(value)
            ));
        }
        let refs = parts.iter().map(String::as_str).collect::<Vec<_>>();
        stable_hash(&refs)
    }

    /// Returns the result status.
    pub fn status(&self) -> PolicyStatus {
        self.status
    }

    /// Returns the result precision.
    pub fn precision(&self) -> PolicyPrecision {
        self.precision
    }

    /// Builds a diagnostic for this violation using the current rule ID.
    pub fn diagnostic(&self, rule_id: &str, message: impl Into<String>) -> Diagnostic {
        let mut diagnostic =
            Diagnostic::error(rule_id.to_string(), self.file.clone(), self.range, message)
                .with_evidence("policy_query", self.query.label())
                .with_evidence("policy_query_version", POLICY_QUERY_VERSION)
                .with_evidence("query_digest", self.query_digest.clone())
                .with_evidence("policy_status", policy_status_label(self.status))
                .with_evidence("policy_precision", policy_precision_label(self.precision));
        for (label, value) in &self.evidence {
            diagnostic = diagnostic.with_evidence(label.clone(), value.clone());
        }
        diagnostic
    }
}

/// Query for reachable event/call policies.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReachQuery {
    /// Target event that must not be reachable.
    pub target: EventPattern,
    /// Optional root events to constrain reachability.
    pub roots: Vec<EventPattern>,
    /// Whether tests are included in the query.
    pub include_tests: bool,
    /// Maximum call depth explored by the query.
    pub max_depth: usize,
    /// Maximum result paths returned by the query.
    pub max_paths: usize,
    /// Minimum acceptable precision.
    pub minimum_precision: PolicyPrecision,
    /// Minimum acceptable confidence.
    pub minimum_confidence: PolicyConfidence,
}

impl ReachQuery {
    /// Creates a reachability query for a required target event.
    pub fn new(target: EventPattern) -> Self {
        Self {
            target,
            roots: Vec::new(),
            include_tests: false,
            max_depth: 20,
            max_paths: 20,
            minimum_precision: PolicyPrecision::Conservative,
            minimum_confidence: PolicyConfidence::Low,
        }
    }

    pub(crate) fn query_digest(&self) -> String {
        policy_query_digest([
            encode_str(PolicyOperation::CallsForbiddenReachable.label()),
            encode_event_pattern("target", &self.target),
            encode_event_pattern_list("roots", &self.roots),
            format!("include_tests={}", self.include_tests),
            format!("max_depth={}", self.max_depth),
            format!("max_paths={}", self.max_paths),
            format!(
                "minimum_precision={}",
                policy_precision_label(self.minimum_precision)
            ),
            format!(
                "minimum_confidence={}",
                policy_confidence_label(self.minimum_confidence)
            ),
        ])
    }
}

/// Query for missing guard policies.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuardQuery {
    /// Sensitive event that requires a guard.
    pub event: EventPattern,
    /// Required guard pattern.
    pub guard: GuardPattern,
    /// Maximum interprocedural depth.
    pub max_depth: usize,
    /// Maximum uncovered paths returned by the query.
    pub max_paths: usize,
    /// Minimum acceptable precision.
    pub minimum_precision: PolicyPrecision,
}

impl GuardQuery {
    /// Creates a guard query for an event and required guard.
    pub fn new(event: EventPattern, guard: GuardPattern) -> Self {
        Self {
            event,
            guard,
            max_depth: 4,
            max_paths: 20,
            minimum_precision: PolicyPrecision::Conservative,
        }
    }

    pub(crate) fn query_digest(&self) -> String {
        policy_query_digest([
            encode_str(PolicyOperation::ControlFlowMissingGuard.label()),
            encode_event_pattern("event", &self.event),
            encode_guard_pattern("guard", &self.guard),
            format!("max_depth={}", self.max_depth),
            format!("max_paths={}", self.max_paths),
            format!(
                "minimum_precision={}",
                policy_precision_label(self.minimum_precision)
            ),
        ])
    }
}

/// Query for resource lifecycle policies.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LifecycleQuery {
    /// Event that acquires or opens a resource.
    pub start: EventPattern,
    /// Event that must close, release, commit, rollback, or otherwise clean up.
    pub cleanup: EventPattern,
    /// Whether cleanup is required on error exits.
    pub require_error_cleanup: bool,
    /// Maximum interprocedural depth.
    pub max_depth: usize,
    /// Maximum uncovered paths returned by the query.
    pub max_paths: usize,
    /// Minimum acceptable precision.
    pub minimum_precision: PolicyPrecision,
}

impl LifecycleQuery {
    /// Creates a lifecycle query from a start event and required cleanup event.
    pub fn new(start: EventPattern, cleanup: EventPattern) -> Self {
        Self {
            start,
            cleanup,
            require_error_cleanup: true,
            max_depth: 4,
            max_paths: 20,
            minimum_precision: PolicyPrecision::Conservative,
        }
    }

    pub(crate) fn query_digest(&self) -> String {
        policy_query_digest([
            encode_str(PolicyOperation::ControlFlowMissingCleanup.label()),
            encode_event_pattern("start", &self.start),
            encode_event_pattern("cleanup", &self.cleanup),
            format!("require_error_cleanup={}", self.require_error_cleanup),
            format!("max_depth={}", self.max_depth),
            format!("max_paths={}", self.max_paths),
            format!(
                "minimum_precision={}",
                policy_precision_label(self.minimum_precision)
            ),
        ])
    }
}

/// Query for source-to-sink data-flow policies.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlowQuery {
    /// Required source pattern.
    pub source: SourcePattern,
    /// Required sink pattern.
    pub sink: SinkPattern,
    /// Optional barrier/sanitizer pattern.
    pub barriers: BarrierPattern,
    /// Maximum interprocedural path depth.
    pub max_depth: usize,
    /// Maximum paths returned by the query.
    pub max_paths: usize,
    /// Minimum acceptable precision.
    pub minimum_precision: PolicyPrecision,
}

impl FlowQuery {
    /// Creates a data-flow query for a required source and sink.
    pub fn new(source: SourcePattern, sink: SinkPattern) -> Self {
        Self {
            source,
            sink,
            barriers: BarrierPattern::none(),
            max_depth: 8,
            max_paths: 20,
            minimum_precision: PolicyPrecision::Conservative,
        }
    }

    pub(crate) fn query_digest(&self) -> String {
        policy_query_digest([
            encode_str(PolicyOperation::DataFlowForbidden.label()),
            encode_source_pattern("source", &self.source),
            encode_sink_pattern("sink", &self.sink),
            encode_barrier_pattern("barriers", &self.barriers),
            format!("max_depth={}", self.max_depth),
            format!("max_paths={}", self.max_paths),
            format!(
                "minimum_precision={}",
                policy_precision_label(self.minimum_precision)
            ),
        ])
    }
}

/// Pattern for semantic events such as calls or field writes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventPattern {
    kind: EventPatternKind,
    values: Vec<String>,
}

impl EventPattern {
    /// Matches a call with an exact canonical target name.
    pub fn call(target: impl Into<String>) -> Self {
        Self {
            kind: EventPatternKind::Call,
            values: vec![target.into()],
        }
    }

    /// Matches a write to an exact canonical field or property name.
    pub fn write_field(field: impl Into<String>) -> Self {
        Self {
            kind: EventPatternKind::WriteField,
            values: vec![field.into()],
        }
    }

    pub(crate) fn kind(&self) -> EventPatternKind {
        self.kind
    }

    pub(crate) fn values(&self) -> &[String] {
        &self.values
    }

    pub(crate) fn query_digest(&self) -> String {
        policy_query_digest([
            encode_str(PolicyOperation::EventsMatching.label()),
            encode_event_pattern("event", self),
        ])
    }
}

/// Pattern for data-flow sources.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourcePattern {
    kind: SourcePatternKind,
    values: Vec<String>,
}

impl SourcePattern {
    /// Matches HTTP request input sources.
    pub fn http_request() -> Self {
        Self {
            kind: SourcePatternKind::HttpRequest,
            values: Vec::new(),
        }
    }

    /// Matches secret-like names from an explicit list of canonical strings.
    pub fn secret_like<I, S>(names: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self {
            kind: SourcePatternKind::SecretLike,
            values: collect_strings(names),
        }
    }

    pub(crate) fn kind(&self) -> SourcePatternKind {
        self.kind
    }

    pub(crate) fn values(&self) -> &[String] {
        &self.values
    }
}

/// Pattern for data-flow sinks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SinkPattern {
    kind: SinkPatternKind,
    values: Vec<String>,
}

impl SinkPattern {
    /// Matches a call sink by exact canonical target name.
    pub fn call(target: impl Into<String>) -> Self {
        Self {
            kind: SinkPatternKind::Call,
            values: vec![target.into()],
        }
    }

    /// Matches common logger sinks.
    pub fn logger() -> Self {
        Self {
            kind: SinkPatternKind::Logger,
            values: Vec::new(),
        }
    }

    pub(crate) fn kind(&self) -> SinkPatternKind {
        self.kind
    }

    pub(crate) fn values(&self) -> &[String] {
        &self.values
    }
}

/// Pattern for guard checks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuardPattern {
    kind: GuardPatternKind,
    values: Vec<String>,
}

impl GuardPattern {
    /// Matches any call in an explicit list of canonical target names.
    pub fn call_any<I, S>(targets: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self {
            kind: GuardPatternKind::CallAny,
            values: collect_strings(targets),
        }
    }

    pub(crate) fn kind(&self) -> GuardPatternKind {
        self.kind
    }

    pub(crate) fn values(&self) -> &[String] {
        &self.values
    }
}

/// Pattern for data-flow barriers or sanitizers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BarrierPattern {
    kind: BarrierPatternKind,
    values: Vec<String>,
}

impl BarrierPattern {
    /// Matches no barrier.
    pub fn none() -> Self {
        Self {
            kind: BarrierPatternKind::None,
            values: Vec::new(),
        }
    }

    /// Matches any call in an explicit list of canonical target names.
    pub fn call_any<I, S>(targets: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self {
            kind: BarrierPatternKind::CallAny,
            values: collect_strings(targets),
        }
    }

    pub(crate) fn kind(&self) -> BarrierPatternKind {
        self.kind
    }

    pub(crate) fn values(&self) -> &[String] {
        &self.values
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EventPatternKind {
    Call,
    WriteField,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SourcePatternKind {
    HttpRequest,
    SecretLike,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SinkPatternKind {
    Call,
    Logger,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GuardPatternKind {
    CallAny,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BarrierPatternKind {
    None,
    CallAny,
}

fn collect_strings<I, S>(values: I) -> Vec<String>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    values.into_iter().map(Into::into).collect()
}

fn policy_query_digest<const N: usize>(parts: [String; N]) -> String {
    let mut all_parts = vec![format!("version={}", encode_str(POLICY_QUERY_VERSION))];
    all_parts.extend(parts);
    let refs = all_parts.iter().map(String::as_str).collect::<Vec<_>>();
    stable_hash(&refs)
}

fn encode_event_pattern(label: &str, pattern: &EventPattern) -> String {
    format!(
        "{}=kind:{};values:{}",
        encode_str(label),
        event_pattern_kind_label(pattern.kind),
        encode_string_set(&pattern.values)
    )
}

fn encode_event_pattern_list(label: &str, patterns: &[EventPattern]) -> String {
    let mut encoded = patterns
        .iter()
        .map(|pattern| encode_event_pattern("pattern", pattern))
        .collect::<Vec<_>>();
    encoded.sort();
    format!("{}=[{}]", encode_str(label), encoded.join("|"))
}

fn encode_source_pattern(label: &str, pattern: &SourcePattern) -> String {
    format!(
        "{}=kind:{};values:{}",
        encode_str(label),
        source_pattern_kind_label(pattern.kind),
        encode_string_set(&pattern.values)
    )
}

fn encode_sink_pattern(label: &str, pattern: &SinkPattern) -> String {
    format!(
        "{}=kind:{};values:{}",
        encode_str(label),
        sink_pattern_kind_label(pattern.kind),
        encode_string_set(&pattern.values)
    )
}

fn encode_guard_pattern(label: &str, pattern: &GuardPattern) -> String {
    format!(
        "{}=kind:{};values:{}",
        encode_str(label),
        guard_pattern_kind_label(pattern.kind),
        encode_string_set(&pattern.values)
    )
}

fn encode_barrier_pattern(label: &str, pattern: &BarrierPattern) -> String {
    format!(
        "{}=kind:{};values:{}",
        encode_str(label),
        barrier_pattern_kind_label(pattern.kind),
        encode_string_set(&pattern.values)
    )
}

fn encode_string_set(values: &[String]) -> String {
    let mut values = values
        .iter()
        .map(|value| encode_str(value))
        .collect::<Vec<_>>();
    values.sort();
    values.dedup();
    values.join(",")
}

fn encode_str(value: &str) -> String {
    format!("{}:{value}", value.len())
}

fn event_pattern_kind_label(kind: EventPatternKind) -> &'static str {
    match kind {
        EventPatternKind::Call => "call",
        EventPatternKind::WriteField => "write_field",
    }
}

fn source_pattern_kind_label(kind: SourcePatternKind) -> &'static str {
    match kind {
        SourcePatternKind::HttpRequest => "http_request",
        SourcePatternKind::SecretLike => "secret_like",
    }
}

fn sink_pattern_kind_label(kind: SinkPatternKind) -> &'static str {
    match kind {
        SinkPatternKind::Call => "call",
        SinkPatternKind::Logger => "logger",
    }
}

fn guard_pattern_kind_label(kind: GuardPatternKind) -> &'static str {
    match kind {
        GuardPatternKind::CallAny => "call_any",
    }
}

fn barrier_pattern_kind_label(kind: BarrierPatternKind) -> &'static str {
    match kind {
        BarrierPatternKind::None => "none",
        BarrierPatternKind::CallAny => "call_any",
    }
}

fn policy_status_label(status: PolicyStatus) -> &'static str {
    match status {
        PolicyStatus::Exact => "exact",
        PolicyStatus::Heuristic => "heuristic",
        PolicyStatus::Unknown => "unknown",
        PolicyStatus::Unsupported => "unsupported",
        PolicyStatus::BudgetExceeded => "budget_exceeded",
    }
}

fn policy_precision_label(precision: PolicyPrecision) -> &'static str {
    match precision {
        PolicyPrecision::Exact => "exact",
        PolicyPrecision::SetupAware => "setup_aware",
        PolicyPrecision::Syntax => "syntax",
        PolicyPrecision::Conservative => "conservative",
        PolicyPrecision::Heuristic => "heuristic",
        PolicyPrecision::Unknown => "unknown",
    }
}

fn policy_confidence_label(confidence: PolicyConfidence) -> &'static str {
    match confidence {
        PolicyConfidence::High => "high",
        PolicyConfidence::Medium => "medium",
        PolicyConfidence::Low => "low",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn policy_query_digest_is_stable_for_same_query() {
        let mut left = FlowQuery::new(
            SourcePattern::secret_like(["token", "password"]),
            SinkPattern::logger(),
        );
        left.barriers = BarrierPattern::call_any(["redact", "mask_secret"]);

        let mut right = FlowQuery::new(
            SourcePattern::secret_like(["password", "token"]),
            SinkPattern::logger(),
        );
        right.barriers = BarrierPattern::call_any(["mask_secret", "redact"]);

        assert_eq!(left.query_digest(), right.query_digest());
    }

    #[test]
    fn policy_query_digest_changes_when_options_change() {
        let baseline = ReachQuery::new(EventPattern::call("dangerous"));
        let mut changed = baseline.clone();
        changed.max_depth += 1;

        assert_ne!(baseline.query_digest(), changed.query_digest());
    }
}
