use std::collections::{BTreeMap, BTreeSet, VecDeque};

use super::facts::{
    PointsToBudgetStatus, PointsToConstraintFact, PointsToConstraintKind, PointsToPrecision,
    PointsToSetFact, PointsToStatus,
};
use super::store::PointsToOutput;
use super::vars;
use crate::analysis_api::{FactFamily, stable_key_from_parts, stable_key_text_from_parts};
use crate::analysis_neutral::ids::{ObjectTokenId, PointsToSetId, PtVarId};
use crate::analysis_neutral::solver::budget::BudgetReason;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PointsToBudget {
    pub max_steps: usize,
    pub max_objects_per_var: usize,
    pub max_dynamic_vars: usize,
}

impl Default for PointsToBudget {
    fn default() -> Self {
        Self {
            max_steps: 10_000,
            max_objects_per_var: 64,
            max_dynamic_vars: 512,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PointsToSolveResult {
    pub sets: Vec<PointsToSetFact>,
    pub budget_status: PointsToBudgetStatus,
    pub budget_reasons: BTreeSet<String>,
}

pub fn solve_points_to(
    interner: &crate::internal_core::StableKeyInterner,
    constraints: &[PointsToConstraintFact],
    budget: PointsToBudget,
) -> PointsToSolveResult {
    let mut solver = Solver::new(interner, constraints, budget);
    solver.solve(interner)
}

struct Solver<'a> {
    interner: &'a crate::internal_core::StableKeyInterner,
    constraints: &'a [PointsToConstraintFact],
    budget: PointsToBudget,
    sets: BTreeMap<PtVarId, BTreeSet<ObjectTokenId>>,
    copy_edges: BTreeMap<PtVarId, BTreeSet<PtVarId>>,
    loads: BTreeMap<PtVarId, BTreeSet<PtVarId>>,
    stores: BTreeMap<PtVarId, BTreeSet<PtVarId>>,
    field_loads: BTreeMap<PtVarId, BTreeSet<(String, PtVarId)>>,
    field_stores: BTreeMap<PtVarId, BTreeSet<(String, PtVarId)>>,
    object_slots: BTreeMap<(ObjectTokenId, String), PtVarId>,
    queue: VecDeque<(PtVarId, BTreeSet<ObjectTokenId>)>,
    dynamic_vars: BTreeSet<PtVarId>,
    variable_identities: BTreeMap<PtVarId, BTreeSet<String>>,
    object_identities: BTreeMap<ObjectTokenId, BTreeSet<String>>,
    steps: usize,
    budget_exceeded: bool,
    budget_reasons: BTreeSet<String>,
}

impl<'a> Solver<'a> {
    fn new(
        interner: &'a crate::internal_core::StableKeyInterner,
        constraints: &'a [PointsToConstraintFact],
        budget: PointsToBudget,
    ) -> Self {
        Self {
            interner,
            constraints,
            budget,
            sets: BTreeMap::new(),
            copy_edges: BTreeMap::new(),
            loads: BTreeMap::new(),
            stores: BTreeMap::new(),
            field_loads: BTreeMap::new(),
            field_stores: BTreeMap::new(),
            object_slots: BTreeMap::new(),
            queue: VecDeque::new(),
            dynamic_vars: BTreeSet::new(),
            variable_identities: BTreeMap::new(),
            object_identities: BTreeMap::new(),
            steps: 0,
            budget_exceeded: false,
            budget_reasons: BTreeSet::new(),
        }
    }

    fn solve(&mut self, interner: &crate::internal_core::StableKeyInterner) -> PointsToSolveResult {
        self.initialize();
        while let Some((var, delta)) = self.queue.pop_front() {
            if !self.step_budget_ok() {
                break;
            }
            self.propagate_copy(var, &delta);
            self.propagate_load(var, &delta);
            self.propagate_store(var, &delta);
            self.propagate_field_load(var, &delta);
            self.propagate_field_store(var, &delta);
        }
        self.to_result(interner)
    }

    fn initialize(&mut self) {
        for constraint in self.constraints.iter().cloned() {
            self.record_constraint_identities(&constraint);
            match &constraint.kind {
                PointsToConstraintKind::AddressOf { dst, object } => {
                    self.add_object(*dst, *object);
                }
                PointsToConstraintKind::Copy { dst, src }
                | PointsToConstraintKind::SummaryFlow { dst, src, .. } => {
                    self.add_copy_edge(*src, *dst);
                }
                PointsToConstraintKind::Load { dst, pointer } => {
                    self.loads.entry(*pointer).or_default().insert(*dst);
                }
                PointsToConstraintKind::Store { pointer, src } => {
                    self.stores.entry(*pointer).or_default().insert(*src);
                }
                PointsToConstraintKind::FieldLoad { dst, base, field } => {
                    self.field_loads
                        .entry(*base)
                        .or_default()
                        .insert((field.clone(), *dst));
                }
                PointsToConstraintKind::FieldStore { base, field, src } => {
                    self.field_stores
                        .entry(*base)
                        .or_default()
                        .insert((field.clone(), *src));
                }
                PointsToConstraintKind::ElementLoad { dst, base, index } => {
                    self.field_loads
                        .entry(*base)
                        .or_default()
                        .insert((format!("[{index}]"), *dst));
                }
                PointsToConstraintKind::ElementStore { base, index, src } => {
                    self.field_stores
                        .entry(*base)
                        .or_default()
                        .insert((format!("[{index}]"), *src));
                }
                PointsToConstraintKind::CallReturn { dst, value } => {
                    self.add_object(*dst, vars::value_fact_object(*value));
                }
            }
        }
        let copy_edges = self.copy_edges.clone();
        for (src, dsts) in copy_edges {
            let Some(objects) = self.sets.get(&src).cloned() else {
                continue;
            };
            for dst in dsts {
                self.add_all(dst, &objects);
            }
        }
    }

    fn propagate_copy(&mut self, var: PtVarId, delta: &BTreeSet<ObjectTokenId>) {
        let dsts = self.copy_edges.get(&var).cloned().unwrap_or_default();
        for dst in dsts {
            self.add_all(dst, delta);
        }
    }

    fn propagate_load(&mut self, var: PtVarId, delta: &BTreeSet<ObjectTokenId>) {
        let dsts = self.loads.get(&var).cloned().unwrap_or_default();
        for dst in dsts {
            for object in delta {
                let contents = self.object_slot(*object, "$contents");
                self.add_copy_edge(contents, dst);
            }
        }
    }

    fn propagate_store(&mut self, var: PtVarId, delta: &BTreeSet<ObjectTokenId>) {
        let srcs = self.stores.get(&var).cloned().unwrap_or_default();
        for src in srcs {
            for object in delta {
                let contents = self.object_slot(*object, "$contents");
                self.add_copy_edge(src, contents);
            }
        }
    }

    fn propagate_field_load(&mut self, var: PtVarId, delta: &BTreeSet<ObjectTokenId>) {
        let loads = self.field_loads.get(&var).cloned().unwrap_or_default();
        for (field, dst) in loads.into_iter() {
            for object in delta {
                let field_var = self.object_slot(*object, field.as_str());
                self.add_copy_edge(field_var, dst);
            }
        }
    }

    fn propagate_field_store(&mut self, var: PtVarId, delta: &BTreeSet<ObjectTokenId>) {
        let stores = self.field_stores.get(&var).cloned().unwrap_or_default();
        for (field, src) in stores.into_iter() {
            for object in delta {
                let field_var = self.object_slot(*object, field.as_str());
                self.add_copy_edge(src, field_var);
            }
        }
    }

    fn add_copy_edge(&mut self, src: PtVarId, dst: PtVarId) {
        if !self.copy_edges.entry(src).or_default().insert(dst) {
            return;
        }
        if let Some(objects) = self.sets.get(&src).cloned() {
            self.add_all(dst, &objects);
        }
    }

    fn add_object(&mut self, dst: PtVarId, object: ObjectTokenId) {
        let mut singleton = BTreeSet::new();
        singleton.insert(object);
        self.add_all(dst, &singleton);
    }

    fn add_all(&mut self, dst: PtVarId, objects: &BTreeSet<ObjectTokenId>) {
        if self.budget_exceeded {
            return;
        }
        let set = self.sets.entry(dst).or_default();
        let mut delta = BTreeSet::new();
        for object in objects {
            if set.insert(*object) {
                delta.insert(*object);
            }
        }
        if set.len() > self.budget.max_objects_per_var {
            self.budget_exceeded = true;
            self.budget_reasons
                .insert(BudgetReason::PointsToMaxObjectsPerVar.as_str().to_string());
            return;
        }
        if !delta.is_empty() {
            self.queue.push_back((dst, delta));
        }
    }

    fn object_slot(&mut self, object: ObjectTokenId, slot: &str) -> PtVarId {
        let key = (object, slot.to_string());
        if let Some(var) = self.object_slots.get(&key) {
            return *var;
        }
        let var = vars::dynamic_var(self.object_slots.len());
        let object_identity = self.semantic_object_identity(object);
        let slot_identity = stable_key_text_from_parts(
            FactFamily::PointsToSet,
            &[("object", object_identity), ("slot", slot.to_string())],
        );
        self.variable_identities
            .entry(var)
            .or_default()
            .insert(slot_identity);
        self.object_slots.insert(key, var);
        self.dynamic_vars.insert(var);
        if self.dynamic_vars.len() > self.budget.max_dynamic_vars {
            self.budget_exceeded = true;
            self.budget_reasons
                .insert(BudgetReason::PointsToMaxDynamicVars.as_str().to_string());
        }
        var
    }

    fn record_constraint_identities(&mut self, constraint: &PointsToConstraintFact) {
        use PointsToConstraintKind as Kind;
        let key = self.interner.resolve(constraint.stable_key);
        match &constraint.kind {
            Kind::AddressOf { dst, object } => {
                self.record_variable_identity(*dst, &key, "address_of_destination");
                self.record_object_identity(*object, &key, "address_of_object");
            }
            Kind::Copy { dst, src } => {
                self.record_variable_identity(*dst, &key, "copy_destination");
                self.record_variable_identity(*src, &key, "copy_source");
            }
            Kind::Load { dst, pointer } => {
                self.record_variable_identity(*dst, &key, "load_destination");
                self.record_variable_identity(*pointer, &key, "load_pointer");
            }
            Kind::Store { pointer, src } => {
                self.record_variable_identity(*pointer, &key, "store_pointer");
                self.record_variable_identity(*src, &key, "store_source");
            }
            Kind::FieldLoad { dst, base, .. } | Kind::ElementLoad { dst, base, .. } => {
                self.record_variable_identity(*dst, &key, "projection_load_destination");
                self.record_variable_identity(*base, &key, "projection_load_base");
            }
            Kind::FieldStore { base, src, .. } | Kind::ElementStore { base, src, .. } => {
                self.record_variable_identity(*base, &key, "projection_store_base");
                self.record_variable_identity(*src, &key, "projection_store_source");
            }
            Kind::CallReturn { dst, value } => {
                self.record_variable_identity(*dst, &key, "call_return_destination");
                self.record_object_identity(
                    vars::value_fact_object(*value),
                    &key,
                    "call_return_object",
                );
            }
            Kind::SummaryFlow { dst, src, .. } => {
                self.record_variable_identity(*dst, &key, "summary_destination");
                self.record_variable_identity(*src, &key, "summary_source");
            }
        }
    }

    fn record_variable_identity(&mut self, variable: PtVarId, key: &str, role: &str) {
        let fragment = stable_key_text_from_parts(
            FactFamily::PointsToSet,
            &[("relation", key.to_string()), ("role", role.to_string())],
        );
        self.variable_identities
            .entry(variable)
            .or_default()
            .insert(fragment);
    }

    fn record_object_identity(&mut self, object: ObjectTokenId, key: &str, role: &str) {
        let fragment = stable_key_text_from_parts(
            FactFamily::PointsToSet,
            &[("relation", key.to_string()), ("role", role.to_string())],
        );
        self.object_identities
            .entry(object)
            .or_default()
            .insert(fragment);
    }

    fn semantic_variable_identity(&self, variable: PtVarId) -> String {
        semantic_relation_identity(
            self.variable_identities
                .get(&variable)
                .expect("points-to set variable must originate in a constraint or object slot"),
        )
    }

    fn semantic_object_identity(&self, object: ObjectTokenId) -> String {
        semantic_relation_identity(
            self.object_identities
                .get(&object)
                .expect("points-to object must originate in an address or call-return constraint"),
        )
    }

    fn step_budget_ok(&mut self) -> bool {
        self.steps += 1;
        if self.steps > self.budget.max_steps {
            self.budget_exceeded = true;
            self.budget_reasons
                .insert(BudgetReason::SolverMaxSteps.as_str().to_string());
            return false;
        }
        true
    }

    fn to_result(&self, interner: &crate::internal_core::StableKeyInterner) -> PointsToSolveResult {
        let budget_status = if self.budget_exceeded {
            PointsToBudgetStatus::BudgetExceeded
        } else {
            PointsToBudgetStatus::WithinBudget
        };
        let status = if self.budget_exceeded {
            PointsToStatus::BudgetExceeded
        } else {
            PointsToStatus::Present
        };
        let precision = if self.budget_exceeded {
            PointsToPrecision::Unknown
        } else {
            PointsToPrecision::FlowInsensitive
        };
        let sets = self
            .sets
            .iter()
            .map(|(variable, objects)| PointsToSetFact {
                id: PointsToSetId(0),
                variable: *variable,
                objects: objects.iter().copied().collect(),
                status,
                precision,
                budget: budget_status,
                stable_key: stable_key_from_parts(
                    interner,
                    FactFamily::PointsToSet,
                    &[
                        ("variable", self.semantic_variable_identity(*variable)),
                        (
                            "budget",
                            points_to_budget_status_label(budget_status).to_string(),
                        ),
                    ],
                ),
            })
            .collect();
        PointsToSolveResult {
            sets,
            budget_status,
            budget_reasons: self.budget_reasons.clone(),
        }
    }
}

fn semantic_relation_identity(fragments: &BTreeSet<String>) -> String {
    fragments
        .iter()
        .map(|fragment| format!("{}:{fragment}", fragment.len()))
        .collect()
}

fn points_to_budget_status_label(status: PointsToBudgetStatus) -> &'static str {
    match status {
        PointsToBudgetStatus::WithinBudget => "within_budget",
        PointsToBudgetStatus::BudgetExceeded => "budget_exceeded",
        PointsToBudgetStatus::NotRun => "not_run",
    }
}

pub fn output_with_solved_sets(
    interner: &crate::internal_core::StableKeyInterner,
    constraints: Vec<PointsToConstraintFact>,
    budget: PointsToBudget,
) -> PointsToOutput {
    let solve = solve_points_to(interner, &constraints, budget);
    PointsToOutput {
        constraints,
        sets: solve.sets,
    }
    .normalized(interner)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis_neutral::ids::{PointsToConstraintId, ValueFactId};

    #[test]
    fn solver_handles_core_constraint_vocabulary_deterministically() {
        let constraints = vec![
            constraint(
                "addr-a",
                PointsToConstraintKind::AddressOf {
                    dst: PtVarId(1),
                    object: ObjectTokenId(10),
                },
            ),
            constraint(
                "copy",
                PointsToConstraintKind::Copy {
                    dst: PtVarId(2),
                    src: PtVarId(1),
                },
            ),
            constraint(
                "field-store",
                PointsToConstraintKind::FieldStore {
                    base: PtVarId(1),
                    field: "name".to_string(),
                    src: PtVarId(2),
                },
            ),
            constraint(
                "field-load",
                PointsToConstraintKind::FieldLoad {
                    dst: PtVarId(3),
                    base: PtVarId(1),
                    field: "name".to_string(),
                },
            ),
            constraint(
                "element-store",
                PointsToConstraintKind::ElementStore {
                    base: PtVarId(1),
                    index: "dynamic".to_string(),
                    src: PtVarId(2),
                },
            ),
            constraint(
                "element-load",
                PointsToConstraintKind::ElementLoad {
                    dst: PtVarId(4),
                    base: PtVarId(1),
                    index: "dynamic".to_string(),
                },
            ),
            constraint(
                "call-return",
                PointsToConstraintKind::CallReturn {
                    dst: PtVarId(5),
                    value: crate::analysis_neutral::ids::ValueFactId(42),
                },
            ),
            constraint(
                "summary-flow",
                PointsToConstraintKind::SummaryFlow {
                    dst: PtVarId(6),
                    src: PtVarId(5),
                    summary_key: "return".to_string(),
                },
            ),
            constraint(
                "load",
                PointsToConstraintKind::Load {
                    dst: PtVarId(7),
                    pointer: PtVarId(1),
                },
            ),
            constraint(
                "store",
                PointsToConstraintKind::Store {
                    pointer: PtVarId(1),
                    src: PtVarId(2),
                },
            ),
        ];
        let first = solve_points_to(
            &crate::internal_core::test_stable_key_interner(),
            &constraints,
            PointsToBudget::default(),
        );
        let second = solve_points_to(
            &crate::internal_core::test_stable_key_interner(),
            &constraints,
            PointsToBudget::default(),
        );

        assert_eq!(first, second);
        assert_eq!(first.budget_status, PointsToBudgetStatus::WithinBudget);
        assert!(
            first.sets.iter().any(|set| {
                set.variable == PtVarId(2) && set.objects.contains(&ObjectTokenId(10))
            })
        );
        assert!(first.sets.iter().any(|set| {
            set.variable == PtVarId(6)
                && set
                    .objects
                    .contains(&vars::value_fact_object(ValueFactId(42)))
        }));
    }

    #[test]
    fn solver_reports_budget_exhaustion_as_unknown_budget_status() {
        let constraints = vec![
            constraint(
                "addr-a",
                PointsToConstraintKind::AddressOf {
                    dst: PtVarId(1),
                    object: ObjectTokenId(10),
                },
            ),
            constraint(
                "addr-b",
                PointsToConstraintKind::AddressOf {
                    dst: PtVarId(1),
                    object: ObjectTokenId(11),
                },
            ),
        ];
        let result = solve_points_to(
            &crate::internal_core::test_stable_key_interner(),
            &constraints,
            PointsToBudget {
                max_objects_per_var: 1,
                ..PointsToBudget::default()
            },
        );

        assert_eq!(result.budget_status, PointsToBudgetStatus::BudgetExceeded);
        assert_eq!(
            result.budget_reasons,
            BTreeSet::from(["points_to.max_objects_per_var".to_string()])
        );
        assert!(
            result
                .sets
                .iter()
                .any(|set| set.status == PointsToStatus::BudgetExceeded)
        );
    }

    #[test]
    fn solver_reports_dynamic_var_budget_reason() {
        let constraints = vec![
            constraint(
                "addr",
                PointsToConstraintKind::AddressOf {
                    dst: PtVarId(1),
                    object: ObjectTokenId(10),
                },
            ),
            constraint(
                "load",
                PointsToConstraintKind::Load {
                    dst: PtVarId(2),
                    pointer: PtVarId(1),
                },
            ),
        ];
        let result = solve_points_to(
            &crate::internal_core::test_stable_key_interner(),
            &constraints,
            PointsToBudget {
                max_dynamic_vars: 0,
                ..PointsToBudget::default()
            },
        );

        assert_eq!(result.budget_status, PointsToBudgetStatus::BudgetExceeded);
        assert_eq!(
            result.budget_reasons,
            BTreeSet::from(["points_to.max_dynamic_vars".to_string()])
        );
    }

    #[test]
    fn solver_reports_step_budget_reason() {
        let constraints = vec![constraint(
            "addr",
            PointsToConstraintKind::AddressOf {
                dst: PtVarId(1),
                object: ObjectTokenId(10),
            },
        )];
        let result = solve_points_to(
            &crate::internal_core::test_stable_key_interner(),
            &constraints,
            PointsToBudget {
                max_steps: 0,
                ..PointsToBudget::default()
            },
        );

        assert_eq!(result.budget_status, PointsToBudgetStatus::BudgetExceeded);
        assert_eq!(
            result.budget_reasons,
            BTreeSet::from(["solver.max_steps".to_string()])
        );
    }

    fn constraint(stable_key: &str, kind: PointsToConstraintKind) -> PointsToConstraintFact {
        PointsToConstraintFact {
            id: PointsToConstraintId(0),
            kind,
            status: PointsToStatus::Present,
            precision: PointsToPrecision::FlowInsensitive,
            stable_key: crate::internal_core::stable_key_for_test(stable_key),
        }
    }
}
