use std::collections::{BTreeMap, BTreeSet};

use super::domain::{SummaryDomain, SummaryTopReason};
use super::facts::{AccessKind, AsyncKind, ExitKind, FlowKind, FlowRoot};

// ---------------------------------------------------------------------------
// AccessKind join
// ---------------------------------------------------------------------------

impl AccessKind {
    pub(crate) fn join(self, other: Self) -> Self {
        match (self, other) {
            (Self::None, x) | (x, Self::None) => x,
            (Self::ReadWrite, _) | (_, Self::ReadWrite) => Self::ReadWrite,
            (Self::Read, Self::Read) => Self::Read,
            (Self::Write, Self::Write) => Self::Write,
            (Self::Read, Self::Write) | (Self::Write, Self::Read) => Self::ReadWrite,
        }
    }
}

// ---------------------------------------------------------------------------
// ControlEffects
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ControlEffects {
    Bottom,
    Effects {
        exits: BTreeSet<ExitKind>,
        async_kind: AsyncKind,
        has_cleanup: bool,
    },
    Top(SummaryTopReason),
}

impl SummaryDomain for ControlEffects {
    const ID: &'static str = "summary.control_effects";
    const VERSION: u32 = 1;

    fn bottom() -> Self {
        Self::Bottom
    }

    fn unknown_top(reason: SummaryTopReason) -> Self {
        Self::Top(reason)
    }

    fn is_bottom(&self) -> bool {
        matches!(self, Self::Bottom)
    }

    fn is_top(&self) -> bool {
        matches!(self, Self::Top(_))
    }

    fn leq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Bottom, _) => true,
            (_, Self::Top(_)) if !self.is_top() => true,
            (Self::Top(left), Self::Top(right)) => left == right,
            (
                Self::Effects {
                    exits: left_exits,
                    async_kind: left_async,
                    has_cleanup: left_cleanup,
                },
                Self::Effects {
                    exits: right_exits,
                    async_kind: right_async,
                    has_cleanup: right_cleanup,
                },
            ) => {
                left_exits.is_subset(right_exits)
                    && (*left_async == *right_async || *right_async == AsyncKind::Unknown)
                    && (!left_cleanup || *right_cleanup)
            }
            _ => false,
        }
    }

    fn join(&self, other: &Self) -> Self {
        match (self, other) {
            (Self::Bottom, value) | (value, Self::Bottom) => value.clone(),
            (Self::Top(left), Self::Top(right)) if left == right => self.clone(),
            (Self::Top(_), _) | (_, Self::Top(_)) => Self::Top(SummaryTopReason::ConflictingFacts),
            (
                Self::Effects {
                    exits: left_exits,
                    async_kind: left_async,
                    has_cleanup: left_cleanup,
                },
                Self::Effects {
                    exits: right_exits,
                    async_kind: right_async,
                    has_cleanup: right_cleanup,
                },
            ) => Self::Effects {
                exits: left_exits.union(right_exits).copied().collect(),
                async_kind: if left_async == right_async {
                    *left_async
                } else {
                    AsyncKind::Unknown
                },
                has_cleanup: *left_cleanup || *right_cleanup,
            },
        }
    }

    fn stable_digest_parts(&self) -> Vec<String> {
        match self {
            Self::Bottom => vec!["control=bottom".to_string()],
            Self::Effects {
                exits,
                async_kind,
                has_cleanup,
            } => {
                let mut parts = Vec::new();
                for exit in exits {
                    parts.push(format!("exit:{exit:?}"));
                }
                parts.sort();
                parts.push(format!("async:{async_kind:?}"));
                parts.push(format!("cleanup:{has_cleanup}"));
                parts
            }
            Self::Top(reason) => vec![format!("control=top:{}", reason.as_str())],
        }
    }
}

// ---------------------------------------------------------------------------
// CallEffects
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum CallEffects {
    Bottom,
    Effects {
        direct_callees: BTreeSet<String>,
        unresolved_count: u32,
        has_callback_invoked: bool,
        has_callback_stored: bool,
    },
    Top(SummaryTopReason),
}

impl SummaryDomain for CallEffects {
    const ID: &'static str = "summary.call_effects";
    const VERSION: u32 = 1;

    fn bottom() -> Self {
        Self::Bottom
    }

    fn unknown_top(reason: SummaryTopReason) -> Self {
        Self::Top(reason)
    }

    fn is_bottom(&self) -> bool {
        matches!(self, Self::Bottom)
    }

    fn is_top(&self) -> bool {
        matches!(self, Self::Top(_))
    }

    fn leq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Bottom, _) => true,
            (_, Self::Top(_)) if !self.is_top() => true,
            (Self::Top(left), Self::Top(right)) => left == right,
            (
                Self::Effects {
                    direct_callees: left_callees,
                    unresolved_count: left_unresolved,
                    has_callback_invoked: left_invoked,
                    has_callback_stored: left_stored,
                },
                Self::Effects {
                    direct_callees: right_callees,
                    unresolved_count: right_unresolved,
                    has_callback_invoked: right_invoked,
                    has_callback_stored: right_stored,
                },
            ) => {
                left_callees.is_subset(right_callees)
                    && left_unresolved <= right_unresolved
                    && (!left_invoked || *right_invoked)
                    && (!left_stored || *right_stored)
            }
            _ => false,
        }
    }

    fn join(&self, other: &Self) -> Self {
        match (self, other) {
            (Self::Bottom, value) | (value, Self::Bottom) => value.clone(),
            (Self::Top(left), Self::Top(right)) if left == right => self.clone(),
            (Self::Top(_), _) | (_, Self::Top(_)) => Self::Top(SummaryTopReason::ConflictingFacts),
            (
                Self::Effects {
                    direct_callees: left_callees,
                    unresolved_count: left_unresolved,
                    has_callback_invoked: left_invoked,
                    has_callback_stored: left_stored,
                },
                Self::Effects {
                    direct_callees: right_callees,
                    unresolved_count: right_unresolved,
                    has_callback_invoked: right_invoked,
                    has_callback_stored: right_stored,
                },
            ) => Self::Effects {
                direct_callees: left_callees.union(right_callees).cloned().collect(),
                unresolved_count: (*left_unresolved).max(*right_unresolved),
                has_callback_invoked: *left_invoked || *right_invoked,
                has_callback_stored: *left_stored || *right_stored,
            },
        }
    }

    fn stable_digest_parts(&self) -> Vec<String> {
        match self {
            Self::Bottom => vec!["call=bottom".to_string()],
            Self::Effects {
                direct_callees,
                unresolved_count,
                has_callback_invoked,
                has_callback_stored,
            } => {
                let mut parts: Vec<String> = direct_callees
                    .iter()
                    .map(|c| format!("callee:{c}"))
                    .collect();
                parts.push(format!("unresolved:{unresolved_count}"));
                parts.push(format!("callback_invoked:{has_callback_invoked}"));
                parts.push(format!("callback_stored:{has_callback_stored}"));
                parts
            }
            Self::Top(reason) => vec![format!("call=top:{}", reason.as_str())],
        }
    }
}

// ---------------------------------------------------------------------------
// MemoryEffects
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum MemoryEffects {
    Bottom,
    Effects {
        receiver: AccessKind,
        params: BTreeMap<u16, AccessKind>,
        return_access: AccessKind,
        local: AccessKind,
        global: AccessKind,
        module: AccessKind,
        may_have_external_effects: bool,
    },
    Top(SummaryTopReason),
}

impl SummaryDomain for MemoryEffects {
    const ID: &'static str = "summary.memory_effects";
    const VERSION: u32 = 1;

    fn bottom() -> Self {
        Self::Bottom
    }

    fn unknown_top(reason: SummaryTopReason) -> Self {
        Self::Top(reason)
    }

    fn is_bottom(&self) -> bool {
        matches!(self, Self::Bottom)
    }

    fn is_top(&self) -> bool {
        matches!(self, Self::Top(_))
    }

    fn leq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Bottom, _) => true,
            (_, Self::Top(_)) if !self.is_top() => true,
            (Self::Top(left), Self::Top(right)) => left == right,
            (
                Self::Effects {
                    receiver: left_recv,
                    params: left_params,
                    return_access: left_ret,
                    local: left_local,
                    global: left_global,
                    module: left_module,
                    may_have_external_effects: left_ext,
                },
                Self::Effects {
                    receiver: right_recv,
                    params: right_params,
                    return_access: right_ret,
                    local: right_local,
                    global: right_global,
                    module: right_module,
                    may_have_external_effects: right_ext,
                },
            ) => {
                access_leq(*left_recv, *right_recv)
                    && access_leq(*left_ret, *right_ret)
                    && access_leq(*left_local, *right_local)
                    && access_leq(*left_global, *right_global)
                    && access_leq(*left_module, *right_module)
                    && (!left_ext || *right_ext)
                    && params_leq(left_params, right_params)
            }
            _ => false,
        }
    }

    fn join(&self, other: &Self) -> Self {
        match (self, other) {
            (Self::Bottom, value) | (value, Self::Bottom) => value.clone(),
            (Self::Top(left), Self::Top(right)) if left == right => self.clone(),
            (Self::Top(_), _) | (_, Self::Top(_)) => Self::Top(SummaryTopReason::ConflictingFacts),
            (
                Self::Effects {
                    receiver: left_recv,
                    params: left_params,
                    return_access: left_ret,
                    local: left_local,
                    global: left_global,
                    module: left_module,
                    may_have_external_effects: left_ext,
                },
                Self::Effects {
                    receiver: right_recv,
                    params: right_params,
                    return_access: right_ret,
                    local: right_local,
                    global: right_global,
                    module: right_module,
                    may_have_external_effects: right_ext,
                },
            ) => Self::Effects {
                receiver: left_recv.join(*right_recv),
                params: join_params(left_params, right_params),
                return_access: left_ret.join(*right_ret),
                local: left_local.join(*right_local),
                global: left_global.join(*right_global),
                module: left_module.join(*right_module),
                may_have_external_effects: *left_ext || *right_ext,
            },
        }
    }

    fn stable_digest_parts(&self) -> Vec<String> {
        match self {
            Self::Bottom => vec!["memory=bottom".to_string()],
            Self::Effects {
                receiver,
                params,
                return_access,
                local,
                global,
                module,
                may_have_external_effects,
            } => {
                let mut parts = vec![
                    format!("receiver:{receiver:?}"),
                    format!("return:{return_access:?}"),
                    format!("local:{local:?}"),
                    format!("global:{global:?}"),
                    format!("module:{module:?}"),
                    format!("external:{may_have_external_effects}"),
                ];
                for (idx, access) in params {
                    parts.push(format!("param[{idx}]:{access:?}"));
                }
                parts.sort();
                parts
            }
            Self::Top(reason) => vec![format!("memory=top:{}", reason.as_str())],
        }
    }
}

fn access_leq(left: AccessKind, right: AccessKind) -> bool {
    left == right || left.join(right) == right
}

fn params_leq(left: &BTreeMap<u16, AccessKind>, right: &BTreeMap<u16, AccessKind>) -> bool {
    for (idx, left_access) in left {
        match right.get(idx) {
            Some(right_access) => {
                if !access_leq(*left_access, *right_access) {
                    return false;
                }
            }
            None => {
                if *left_access != AccessKind::None {
                    return false;
                }
            }
        }
    }
    true
}

fn join_params(
    left: &BTreeMap<u16, AccessKind>,
    right: &BTreeMap<u16, AccessKind>,
) -> BTreeMap<u16, AccessKind> {
    let mut result = left.clone();
    for (idx, right_access) in right {
        let entry = result.entry(*idx).or_insert(AccessKind::None);
        *entry = entry.join(*right_access);
    }
    result
}

// ---------------------------------------------------------------------------
// DataFlowTito
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct FlowEdge {
    pub(crate) from: FlowRoot,
    pub(crate) to: FlowRoot,
    pub(crate) kind: FlowKind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum DataFlowTito {
    Bottom,
    Flows {
        edges: BTreeSet<FlowEdge>,
        has_source_return: bool,
        has_sink_param: bool,
    },
    Top(SummaryTopReason),
}

impl SummaryDomain for DataFlowTito {
    const ID: &'static str = "summary.data_flow_tito";
    const VERSION: u32 = 1;

    fn bottom() -> Self {
        Self::Bottom
    }

    fn unknown_top(reason: SummaryTopReason) -> Self {
        Self::Top(reason)
    }

    fn is_bottom(&self) -> bool {
        matches!(self, Self::Bottom)
    }

    fn is_top(&self) -> bool {
        matches!(self, Self::Top(_))
    }

    fn leq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Bottom, _) => true,
            (_, Self::Top(_)) if !self.is_top() => true,
            (Self::Top(left), Self::Top(right)) => left == right,
            (
                Self::Flows {
                    edges: left_edges,
                    has_source_return: left_source,
                    has_sink_param: left_sink,
                },
                Self::Flows {
                    edges: right_edges,
                    has_source_return: right_source,
                    has_sink_param: right_sink,
                },
            ) => {
                left_edges.is_subset(right_edges)
                    && (!left_source || *right_source)
                    && (!left_sink || *right_sink)
            }
            _ => false,
        }
    }

    fn join(&self, other: &Self) -> Self {
        match (self, other) {
            (Self::Bottom, value) | (value, Self::Bottom) => value.clone(),
            (Self::Top(left), Self::Top(right)) if left == right => self.clone(),
            (Self::Top(_), _) | (_, Self::Top(_)) => Self::Top(SummaryTopReason::ConflictingFacts),
            (
                Self::Flows {
                    edges: left_edges,
                    has_source_return: left_source,
                    has_sink_param: left_sink,
                },
                Self::Flows {
                    edges: right_edges,
                    has_source_return: right_source,
                    has_sink_param: right_sink,
                },
            ) => Self::Flows {
                edges: left_edges.union(right_edges).cloned().collect(),
                has_source_return: *left_source || *right_source,
                has_sink_param: *left_sink || *right_sink,
            },
        }
    }

    fn stable_digest_parts(&self) -> Vec<String> {
        match self {
            Self::Bottom => vec!["tito=bottom".to_string()],
            Self::Flows {
                edges,
                has_source_return,
                has_sink_param,
            } => {
                let mut parts: Vec<String> = edges
                    .iter()
                    .map(|e| format!("edge:{:?}->{:?}:{:?}", e.from, e.to, e.kind))
                    .collect();
                parts.push(format!("source_return:{has_source_return}"));
                parts.push(format!("sink_param:{has_sink_param}"));
                parts
            }
            Self::Top(reason) => vec![format!("tito=top:{}", reason.as_str())],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // Generic law test helpers
    // -----------------------------------------------------------------------

    fn assert_bottom_leq_all<D: SummaryDomain + std::fmt::Debug>(values: &[D]) {
        let bottom = D::bottom();
        for value in values {
            assert!(bottom.leq(value), "bottom must be leq to {value:?}",);
        }
    }

    fn assert_top_geq_all<D: SummaryDomain + std::fmt::Debug>(
        values: &[D],
        reason: SummaryTopReason,
    ) {
        let top = D::unknown_top(reason);
        for value in values {
            assert!(value.leq(&top), "{value:?} must be leq to top",);
        }
    }

    fn assert_join_commutative<D: SummaryDomain + std::fmt::Debug>(a: &D, b: &D) {
        assert_eq!(
            a.join(b),
            b.join(a),
            "join must be commutative for {a:?} and {b:?}"
        );
    }

    fn assert_join_idempotent<D: SummaryDomain + std::fmt::Debug>(value: &D) {
        assert_eq!(
            value.join(value),
            *value,
            "join must be idempotent for {value:?}"
        );
    }

    fn assert_digest_deterministic<D: SummaryDomain + std::fmt::Debug>(value: &D) {
        assert_eq!(
            value.stable_digest_parts(),
            value.stable_digest_parts(),
            "stable_digest_parts must be deterministic"
        );
    }

    // -----------------------------------------------------------------------
    // AccessKind tests
    // -----------------------------------------------------------------------

    #[test]
    fn access_kind_join_none_is_identity() {
        assert_eq!(AccessKind::None.join(AccessKind::Read), AccessKind::Read);
        assert_eq!(AccessKind::None.join(AccessKind::Write), AccessKind::Write);
        assert_eq!(AccessKind::None.join(AccessKind::None), AccessKind::None);
    }

    #[test]
    fn access_kind_join_read_write_is_readwrite() {
        assert_eq!(
            AccessKind::Read.join(AccessKind::Write),
            AccessKind::ReadWrite
        );
        assert_eq!(
            AccessKind::Write.join(AccessKind::Read),
            AccessKind::ReadWrite
        );
    }

    #[test]
    fn access_kind_join_readwrite_absorbs_all() {
        assert_eq!(
            AccessKind::ReadWrite.join(AccessKind::None),
            AccessKind::ReadWrite
        );
        assert_eq!(
            AccessKind::ReadWrite.join(AccessKind::Read),
            AccessKind::ReadWrite
        );
        assert_eq!(
            AccessKind::ReadWrite.join(AccessKind::Write),
            AccessKind::ReadWrite
        );
        assert_eq!(
            AccessKind::ReadWrite.join(AccessKind::ReadWrite),
            AccessKind::ReadWrite
        );
    }

    // -----------------------------------------------------------------------
    // ControlEffects law tests
    // -----------------------------------------------------------------------

    fn sample_control_effects() -> Vec<ControlEffects> {
        vec![
            ControlEffects::Bottom,
            ControlEffects::Effects {
                exits: BTreeSet::from([ExitKind::Returns]),
                async_kind: AsyncKind::Sync,
                has_cleanup: false,
            },
            ControlEffects::Effects {
                exits: BTreeSet::from([ExitKind::Throws, ExitKind::Returns]),
                async_kind: AsyncKind::Async,
                has_cleanup: true,
            },
            ControlEffects::unknown_top(SummaryTopReason::UnresolvedCallee),
        ]
    }

    #[test]
    fn control_bottom_leq_all() {
        assert_bottom_leq_all(&sample_control_effects());
    }

    #[test]
    fn control_top_geq_all() {
        assert_top_geq_all(
            &sample_control_effects(),
            SummaryTopReason::UnresolvedCallee,
        );
    }

    #[test]
    fn control_join_commutative() {
        let samples = sample_control_effects();
        for a in &samples {
            for b in &samples {
                assert_join_commutative(a, b);
            }
        }
    }

    #[test]
    fn control_join_idempotent() {
        for value in &sample_control_effects() {
            assert_join_idempotent(value);
        }
    }

    #[test]
    fn control_unknown_top_is_top() {
        assert!(ControlEffects::unknown_top(SummaryTopReason::SetupMissing).is_top());
    }

    #[test]
    fn control_bottom_is_bottom() {
        assert!(ControlEffects::bottom().is_bottom());
    }

    #[test]
    fn control_digest_deterministic() {
        for value in &sample_control_effects() {
            assert_digest_deterministic(value);
        }
    }

    #[test]
    fn control_join_merges_exit_sets() {
        let a = ControlEffects::Effects {
            exits: BTreeSet::from([ExitKind::Returns]),
            async_kind: AsyncKind::Sync,
            has_cleanup: false,
        };
        let b = ControlEffects::Effects {
            exits: BTreeSet::from([ExitKind::Throws]),
            async_kind: AsyncKind::Sync,
            has_cleanup: true,
        };
        let joined = a.join(&b);
        match &joined {
            ControlEffects::Effects {
                exits, has_cleanup, ..
            } => {
                assert!(exits.contains(&ExitKind::Returns));
                assert!(exits.contains(&ExitKind::Throws));
                assert!(*has_cleanup);
            }
            other => panic!("expected Effects, got {other:?}"),
        }
    }

    #[test]
    fn control_join_different_async_becomes_unknown() {
        let a = ControlEffects::Effects {
            exits: BTreeSet::new(),
            async_kind: AsyncKind::Sync,
            has_cleanup: false,
        };
        let b = ControlEffects::Effects {
            exits: BTreeSet::new(),
            async_kind: AsyncKind::Async,
            has_cleanup: false,
        };
        match a.join(&b) {
            ControlEffects::Effects { async_kind, .. } => {
                assert_eq!(async_kind, AsyncKind::Unknown);
            }
            other => panic!("expected Effects, got {other:?}"),
        }
    }

    // -----------------------------------------------------------------------
    // CallEffects law tests
    // -----------------------------------------------------------------------

    fn sample_call_effects() -> Vec<CallEffects> {
        vec![
            CallEffects::Bottom,
            CallEffects::Effects {
                direct_callees: BTreeSet::from(["foo".to_string()]),
                unresolved_count: 0,
                has_callback_invoked: false,
                has_callback_stored: false,
            },
            CallEffects::Effects {
                direct_callees: BTreeSet::from(["foo".to_string(), "bar".to_string()]),
                unresolved_count: 2,
                has_callback_invoked: true,
                has_callback_stored: true,
            },
            CallEffects::unknown_top(SummaryTopReason::UnresolvedCallee),
        ]
    }

    #[test]
    fn call_bottom_leq_all() {
        assert_bottom_leq_all(&sample_call_effects());
    }

    #[test]
    fn call_top_geq_all() {
        assert_top_geq_all(&sample_call_effects(), SummaryTopReason::UnresolvedCallee);
    }

    #[test]
    fn call_join_commutative() {
        let samples = sample_call_effects();
        for a in &samples {
            for b in &samples {
                assert_join_commutative(a, b);
            }
        }
    }

    #[test]
    fn call_join_idempotent() {
        for value in &sample_call_effects() {
            assert_join_idempotent(value);
        }
    }

    #[test]
    fn call_unknown_top_is_top() {
        assert!(CallEffects::unknown_top(SummaryTopReason::DynamicWrite).is_top());
    }

    #[test]
    fn call_bottom_is_bottom() {
        assert!(CallEffects::bottom().is_bottom());
    }

    #[test]
    fn call_digest_deterministic() {
        for value in &sample_call_effects() {
            assert_digest_deterministic(value);
        }
    }

    #[test]
    fn call_join_merges_callees_and_maxes_unresolved() {
        let a = CallEffects::Effects {
            direct_callees: BTreeSet::from(["foo".to_string()]),
            unresolved_count: 1,
            has_callback_invoked: false,
            has_callback_stored: false,
        };
        let b = CallEffects::Effects {
            direct_callees: BTreeSet::from(["bar".to_string()]),
            unresolved_count: 2,
            has_callback_invoked: true,
            has_callback_stored: false,
        };
        match a.join(&b) {
            CallEffects::Effects {
                direct_callees,
                unresolved_count,
                has_callback_invoked,
                has_callback_stored,
            } => {
                assert!(direct_callees.contains("foo"));
                assert!(direct_callees.contains("bar"));
                assert_eq!(unresolved_count, 2);
                assert!(has_callback_invoked);
                assert!(!has_callback_stored);
            }
            other => panic!("expected Effects, got {other:?}"),
        }
    }

    // -----------------------------------------------------------------------
    // MemoryEffects law tests
    // -----------------------------------------------------------------------

    fn sample_memory_effects() -> Vec<MemoryEffects> {
        vec![
            MemoryEffects::Bottom,
            MemoryEffects::Effects {
                receiver: AccessKind::Read,
                params: BTreeMap::from([(0, AccessKind::Read)]),
                return_access: AccessKind::Write,
                local: AccessKind::None,
                global: AccessKind::None,
                module: AccessKind::None,
                may_have_external_effects: false,
            },
            MemoryEffects::Effects {
                receiver: AccessKind::ReadWrite,
                params: BTreeMap::from([(0, AccessKind::Write), (1, AccessKind::Read)]),
                return_access: AccessKind::ReadWrite,
                local: AccessKind::Read,
                global: AccessKind::Write,
                module: AccessKind::Read,
                may_have_external_effects: true,
            },
            MemoryEffects::unknown_top(SummaryTopReason::DynamicWrite),
        ]
    }

    #[test]
    fn memory_bottom_leq_all() {
        assert_bottom_leq_all(&sample_memory_effects());
    }

    #[test]
    fn memory_top_geq_all() {
        assert_top_geq_all(&sample_memory_effects(), SummaryTopReason::DynamicWrite);
    }

    #[test]
    fn memory_join_commutative() {
        let samples = sample_memory_effects();
        for a in &samples {
            for b in &samples {
                assert_join_commutative(a, b);
            }
        }
    }

    #[test]
    fn memory_join_idempotent() {
        for value in &sample_memory_effects() {
            assert_join_idempotent(value);
        }
    }

    #[test]
    fn memory_unknown_top_is_top() {
        assert!(MemoryEffects::unknown_top(SummaryTopReason::SetupMissing).is_top());
    }

    #[test]
    fn memory_bottom_is_bottom() {
        assert!(MemoryEffects::bottom().is_bottom());
    }

    #[test]
    fn memory_digest_deterministic() {
        for value in &sample_memory_effects() {
            assert_digest_deterministic(value);
        }
    }

    #[test]
    fn memory_join_per_resource_access_kind() {
        let a = MemoryEffects::Effects {
            receiver: AccessKind::Read,
            params: BTreeMap::from([(0, AccessKind::Read)]),
            return_access: AccessKind::None,
            local: AccessKind::None,
            global: AccessKind::None,
            module: AccessKind::None,
            may_have_external_effects: false,
        };
        let b = MemoryEffects::Effects {
            receiver: AccessKind::Write,
            params: BTreeMap::from([(0, AccessKind::Write)]),
            return_access: AccessKind::Read,
            local: AccessKind::None,
            global: AccessKind::None,
            module: AccessKind::None,
            may_have_external_effects: true,
        };
        match a.join(&b) {
            MemoryEffects::Effects {
                receiver,
                params,
                return_access,
                may_have_external_effects,
                ..
            } => {
                assert_eq!(receiver, AccessKind::ReadWrite);
                assert_eq!(params.get(&0), Some(&AccessKind::ReadWrite));
                assert_eq!(return_access, AccessKind::Read);
                assert!(may_have_external_effects);
            }
            other => panic!("expected Effects, got {other:?}"),
        }
    }

    // -----------------------------------------------------------------------
    // DataFlowTito law tests
    // -----------------------------------------------------------------------

    fn sample_tito() -> Vec<DataFlowTito> {
        vec![
            DataFlowTito::Bottom,
            DataFlowTito::Flows {
                edges: BTreeSet::from([FlowEdge {
                    from: FlowRoot::Param(0),
                    to: FlowRoot::Return,
                    kind: FlowKind::Value,
                }]),
                has_source_return: true,
                has_sink_param: false,
            },
            DataFlowTito::Flows {
                edges: BTreeSet::from([
                    FlowEdge {
                        from: FlowRoot::Param(0),
                        to: FlowRoot::Return,
                        kind: FlowKind::Value,
                    },
                    FlowEdge {
                        from: FlowRoot::Receiver,
                        to: FlowRoot::Receiver,
                        kind: FlowKind::BySideEffect,
                    },
                ]),
                has_source_return: true,
                has_sink_param: true,
            },
            DataFlowTito::unknown_top(SummaryTopReason::UnsupportedSemantic),
        ]
    }

    #[test]
    fn tito_bottom_leq_all() {
        assert_bottom_leq_all(&sample_tito());
    }

    #[test]
    fn tito_top_geq_all() {
        assert_top_geq_all(&sample_tito(), SummaryTopReason::UnsupportedSemantic);
    }

    #[test]
    fn tito_join_commutative() {
        let samples = sample_tito();
        for a in &samples {
            for b in &samples {
                assert_join_commutative(a, b);
            }
        }
    }

    #[test]
    fn tito_join_idempotent() {
        for value in &sample_tito() {
            assert_join_idempotent(value);
        }
    }

    #[test]
    fn tito_unknown_top_is_top() {
        assert!(DataFlowTito::unknown_top(SummaryTopReason::BudgetExceeded).is_top());
    }

    #[test]
    fn tito_bottom_is_bottom() {
        assert!(DataFlowTito::bottom().is_bottom());
    }

    #[test]
    fn tito_digest_deterministic() {
        for value in &sample_tito() {
            assert_digest_deterministic(value);
        }
    }

    #[test]
    fn tito_join_merges_edge_sets() {
        let a = DataFlowTito::Flows {
            edges: BTreeSet::from([FlowEdge {
                from: FlowRoot::Param(0),
                to: FlowRoot::Return,
                kind: FlowKind::Value,
            }]),
            has_source_return: false,
            has_sink_param: false,
        };
        let b = DataFlowTito::Flows {
            edges: BTreeSet::from([FlowEdge {
                from: FlowRoot::Receiver,
                to: FlowRoot::Receiver,
                kind: FlowKind::BySideEffect,
            }]),
            has_source_return: true,
            has_sink_param: true,
        };
        match a.join(&b) {
            DataFlowTito::Flows {
                edges,
                has_source_return,
                has_sink_param,
            } => {
                assert_eq!(edges.len(), 2);
                assert!(has_source_return);
                assert!(has_sink_param);
            }
            other => panic!("expected Flows, got {other:?}"),
        }
    }

    #[test]
    fn flow_edge_has_from_to_kind() {
        let edge = FlowEdge {
            from: FlowRoot::Param(0),
            to: FlowRoot::Return,
            kind: FlowKind::Value,
        };
        assert_eq!(edge.from, FlowRoot::Param(0));
        assert_eq!(edge.to, FlowRoot::Return);
        assert_eq!(edge.kind, FlowKind::Value);
    }
}
