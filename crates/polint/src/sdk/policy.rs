//! Preview policy-query vocabulary for repo-local rules.
//!
//! These types define the public shape for v1.4 policy queries. They are
//! intentionally plain data structs with `new(required...)` constructors and
//! named option fields. Runtime query behavior is fail-closed until the
//! corresponding preview capabilities are backed by provider facts.

use crate::diagnostics::{Diagnostic, TextRange};

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

/// Preview violation returned by policy-query views.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyViolation {
    file: String,
    range: TextRange,
    status: PolicyStatus,
    precision: PolicyPrecision,
    evidence: Vec<(String, String)>,
}

impl PolicyViolation {
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
        }
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
        }
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EventPatternKind {
    Call,
    WriteField,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SourcePatternKind {
    HttpRequest,
    SecretLike,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SinkPatternKind {
    Call,
    Logger,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GuardPatternKind {
    CallAny,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BarrierPatternKind {
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
