//! A small Andersen-style (inclusion-based) points-to fixpoint for JS/TS — the
//! "value-token heap" (see `performance/2026-06-11-js-points-to-heap-plan.md`).
//!
//! This is the solver CORE: tokens, cells, inclusion constraints, and a
//! delta-driven worklist fixpoint. It is deliberately decoupled from the AST —
//! the harvest layer (a later phase) translates JS/TS into the [`Constraint`]
//! vocabulary here, exactly as `ts_value_flows` already decodes every construct.
//!
//! ## Why a private heap, and why not the semantic-graph solver
//!
//! Phase-0 spike finding (recorded in the plan): the kernel's `semantic_graph`
//! builder runs under the benchmark's empty plan, but it is LOSSY for the shapes
//! that fail — it lumps constant computed keys (`obj["q" + "we"]`) into a single
//! `computed_bucket`, mis-attributes the base object, and under-models
//! function-objects and `new`-instances as allocation sites. Fixing that builder
//! is invasive to a shared, snapshot-pinned kernel component. So the heap is
//! built fresh, field-SENSITIVE (a property cell is keyed on the *object token*,
//! not just the name), and fed by an AST harvest that resolves keys precisely.
//!
//! ## The model in one paragraph
//!
//! A [`Token`] is an abstract value born at one place — a function (which is also
//! an object, so it can carry properties) or an allocation (object / array /
//! instance). A [`CellId`] names a place a value can live — a lexical binding, an
//! intermediate expression, a function's return / parameter / `this`, or
//! (minted lazily during solving) one specific property of one specific object
//! token. [`Constraint`]s state inclusions over those cells. The solver pushes
//! tokens along subset edges and fires deferred listeners — property loads/stores
//! resolve per object token that reaches the base, and a call resolves per
//! function token that reaches the callee, recording the edge and wiring
//! arguments → parameters and return → result. Run to a fixpoint; read the edges.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

/// An abstract value, interned to a dense [`TokenId`]. A function token doubles
/// as the object identity for that function's own properties (`f.g = …`), which
/// is what lets `Foo[prop] = g; Foo[prop]()` resolve.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum Token {
    /// A function value. The payload is an opaque function identity the harvest
    /// assigns (in practice a `FunctionId`'s inner value).
    Function(u64),
    /// A heap allocation (object / array literal, `new` instance, …), keyed by an
    /// opaque allocation-site identity.
    Object(u64),
}

/// Dense handle for a [`Token`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct TokenId(pub(crate) u32);

/// Dense handle for a cell (a place a token can live).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct CellId(pub(crate) u32);

/// Identity of a call site, carried through to the resolved edge.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct CallId(pub(crate) u64);

/// One inclusion constraint — the program, compiled. `FieldLoad`, `FieldStore`
/// and `Call` are *deferred*: they re-fire per token that reaches their base /
/// callee cell (the token-listener pattern).
#[derive(Clone, Debug)]
pub(crate) enum Constraint {
    /// `{token} ⊆ into` — a literal/allocation places its token in a cell.
    Alloc { token: Token, into: CellId },
    /// `from ⊆ to` — assignment / parameter passing / aliasing.
    Subset { from: CellId, to: CellId },
    /// `base.field = src` (statically-known field). For each object token `t` in
    /// `base`, `src ⊆ prop(t, field)`.
    FieldStore {
        base: CellId,
        field: String,
        src: CellId,
    },
    /// `into = base.field`. For each object token `t` in `base`,
    /// `prop(t, field) ⊆ into`.
    FieldLoad {
        base: CellId,
        field: String,
        into: CellId,
    },
    /// A call. For each `Token::Function(f)` reaching `callee`: record the edge
    /// `(site → f)`, wire `args[i] ⊆ param(f, i)`, `return(f) ⊆ result`, and
    /// `this_arg ⊆ this(f)`.
    Call {
        callee: CellId,
        args: Vec<CellId>,
        this_arg: Option<CellId>,
        result: CellId,
        site: CallId,
    },
    /// `new C()`: for each `Token::Function(C)` reaching `callee` that has a
    /// registered prototype token P (its instance methods), the fresh `instance`
    /// token inherits P's properties — so `new C().m()` resolves even cross-file
    /// (`new lib.X()`), where a harvest-time name lookup cannot reach the class.
    Construct { callee: CellId, instance: Token },
}

/// Per-function cells known at harvest time (a function's parameters, `this`, and
/// return slot), so a `Call` can wire them when the function token resolves.
#[derive(Clone, Debug, Default)]
pub(crate) struct FunctionCells {
    pub(crate) params: Vec<CellId>,
    pub(crate) rest: Option<CellId>,
    pub(crate) this: Option<CellId>,
    pub(crate) ret: Option<CellId>,
}

/// The program the harvest builds: a flat constraint list, a cell count, and the
/// per-function cell map. Built once, solved once.
#[derive(Clone, Debug, Default)]
pub(crate) struct PointsToProgram {
    next_cell: u32,
    next_token: u32,
    token_ids: BTreeMap<Token, TokenId>,
    constraints: Vec<Constraint>,
    /// `function payload -> its parameter/this/return cells`.
    function_cells: BTreeMap<u64, FunctionCells>,
    /// `constructor function payload -> its prototype token` (carrying the class's
    /// instance methods), consulted on `Construct` to link a fresh instance.
    class_prototypes: BTreeMap<u64, Token>,
}

impl PointsToProgram {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Mint a fresh cell.
    pub(crate) fn fresh_cell(&mut self) -> CellId {
        let id = CellId(self.next_cell);
        self.next_cell += 1;
        id
    }

    /// Intern a token to a dense id (deterministic: first-seen order).
    pub(crate) fn intern_token(&mut self, token: Token) -> TokenId {
        if let Some(id) = self.token_ids.get(&token) {
            return *id;
        }
        let id = TokenId(self.next_token);
        self.next_token += 1;
        self.token_ids.insert(token, id);
        id
    }

    pub(crate) fn set_function_cells(&mut self, function: u64, cells: FunctionCells) {
        self.function_cells.insert(function, cells);
    }

    /// Register a class's prototype token (its instance methods) under its
    /// constructor payload, for `Construct` inheritance.
    pub(crate) fn set_class_prototype(&mut self, constructor: u64, prototype: Token) {
        self.intern_token(prototype);
        self.class_prototypes.insert(constructor, prototype);
    }

    pub(crate) fn add(&mut self, constraint: Constraint) {
        // Interning an allocation's token here means callers (the harvest, which
        // adds many `Alloc`s) never have to intern separately.
        match &constraint {
            Constraint::Alloc { token, .. } => {
                self.intern_token(*token);
            }
            Constraint::Construct { instance, .. } => {
                self.intern_token(*instance);
            }
            _ => {}
        }
        self.constraints.push(constraint);
    }

    /// Run the fixpoint and return the resolved call edges plus a budget verdict.
    pub(crate) fn solve(&self, budget: &PointsToBudget) -> PointsToResult {
        Solver::new(self, budget).run()
    }
}

/// Solver budgets — the contract for staying bounded on real programs. On any
/// ceiling, the solver stops and reports the reason; edges already derived stay
/// valid (honest partial results, never a panic).
#[derive(Clone, Copy, Debug)]
pub(crate) struct PointsToBudget {
    pub(crate) max_steps: u64,
    pub(crate) max_tokens_per_cell: usize,
}

impl Default for PointsToBudget {
    fn default() -> Self {
        Self {
            max_steps: 2_000_000,
            max_tokens_per_cell: 64,
        }
    }
}

/// Resolved output: the call edges (site → function payload) and an honest budget
/// verdict.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct PointsToResult {
    /// `(call site, resolved function payload)`, sorted/deduped for determinism.
    pub(crate) edges: BTreeSet<(u64, u64)>,
    pub(crate) budget_exhausted: bool,
    pub(crate) budget_reasons: BTreeSet<String>,
}

struct Solver<'p> {
    program: &'p PointsToProgram,
    budget: PointsToBudget,
    /// Token sets per cell. Grows past the program's cell count as property cells
    /// are minted lazily.
    sets: Vec<BTreeSet<TokenId>>,
    /// Subset adjacency: `from -> [to, …]`.
    subset: Vec<Vec<CellId>>,
    /// Deferred field loads keyed by base cell.
    loads: Vec<Vec<(String, CellId)>>,
    /// Deferred field stores keyed by base cell.
    stores: Vec<Vec<(String, CellId)>>,
    /// Deferred calls keyed by callee cell (index into `call_specs`).
    calls: Vec<Vec<usize>>,
    call_specs: Vec<CallSpec>,
    /// Deferred `Construct`s keyed by callee cell (index into `construct_instances`).
    constructs: Vec<Vec<usize>>,
    construct_instances: Vec<TokenId>,
    /// Prototype inheritance: `object token -> [prototype tokens]`. A field load on
    /// an object also reads its prototypes' properties.
    inherits: BTreeMap<TokenId, Vec<TokenId>>,
    /// Every `(field, into)` a token has been loaded under, so a newly-linked
    /// prototype can be flushed into prior loads.
    loads_on_token: BTreeMap<TokenId, Vec<(String, CellId)>>,
    /// Lazily-minted property cells: `(object token, field) -> cell`.
    prop_cells: BTreeMap<(TokenId, String), CellId>,
    /// Reverse token table (id -> token) for dispatch decisions.
    token_by_id: Vec<Token>,
    /// Worklist of `(cell, newly-added tokens)`.
    queue: VecDeque<(CellId, Vec<TokenId>)>,
    edges: BTreeSet<(u64, u64)>,
    steps: u64,
    exhausted: bool,
    reasons: BTreeSet<String>,
}

#[derive(Clone)]
struct CallSpec {
    args: Vec<CellId>,
    this_arg: Option<CellId>,
    result: CellId,
    site: CallId,
}

impl<'p> Solver<'p> {
    fn new(program: &'p PointsToProgram, budget: &PointsToBudget) -> Self {
        let n = program.next_cell as usize;
        let mut token_by_id = vec![Token::Object(0); program.token_ids.len()];
        for (token, id) in &program.token_ids {
            token_by_id[id.0 as usize] = *token;
        }
        let mut solver = Solver {
            program,
            budget: *budget,
            sets: vec![BTreeSet::new(); n],
            subset: vec![Vec::new(); n],
            loads: vec![Vec::new(); n],
            stores: vec![Vec::new(); n],
            calls: vec![Vec::new(); n],
            call_specs: Vec::new(),
            constructs: vec![Vec::new(); n],
            construct_instances: Vec::new(),
            inherits: BTreeMap::new(),
            loads_on_token: BTreeMap::new(),
            prop_cells: BTreeMap::new(),
            token_by_id,
            queue: VecDeque::new(),
            edges: BTreeSet::new(),
            steps: 0,
            exhausted: false,
            reasons: BTreeSet::new(),
        };
        solver.install();
        solver
    }

    /// Index the constraints into adjacency / listener tables and seed the
    /// worklist from `Alloc`s.
    fn install(&mut self) {
        // Re-intern tokens against the program's table so Alloc carries dense ids.
        let token_id = |token: Token| -> TokenId {
            *self
                .program
                .token_ids
                .get(&token)
                .expect("alloc token interned during harvest")
        };
        for constraint in &self.program.constraints {
            match constraint {
                Constraint::Alloc { token, into } => {
                    let t = token_id(*token);
                    self.seed(*into, t);
                }
                Constraint::Subset { from, to } => {
                    self.subset[from.0 as usize].push(*to);
                }
                Constraint::FieldStore { base, field, src } => {
                    self.stores[base.0 as usize].push((field.clone(), *src));
                }
                Constraint::FieldLoad { base, field, into } => {
                    self.loads[base.0 as usize].push((field.clone(), *into));
                }
                Constraint::Call {
                    callee,
                    args,
                    this_arg,
                    result,
                    site,
                } => {
                    let idx = self.call_specs.len();
                    self.call_specs.push(CallSpec {
                        args: args.clone(),
                        this_arg: *this_arg,
                        result: *result,
                        site: *site,
                    });
                    self.calls[callee.0 as usize].push(idx);
                }
                Constraint::Construct { callee, instance } => {
                    let idx = self.construct_instances.len();
                    self.construct_instances.push(token_id(*instance));
                    self.constructs[callee.0 as usize].push(idx);
                }
            }
        }
    }

    /// Add a token to a cell's set; enqueue the delta if it is new.
    fn seed(&mut self, cell: CellId, token: TokenId) {
        let set = &mut self.sets[cell.0 as usize];
        if set.len() >= self.budget.max_tokens_per_cell && !set.contains(&token) {
            self.exhausted = true;
            self.reasons.insert("max_tokens_per_cell".to_string());
            return;
        }
        if set.insert(token) {
            self.queue.push_back((cell, vec![token]));
        }
    }

    /// Mint (or fetch) the property cell for one object token and field name.
    fn prop_cell(&mut self, token: TokenId, field: &str) -> CellId {
        if let Some(cell) = self.prop_cells.get(&(token, field.to_string())) {
            return *cell;
        }
        let id = CellId(self.sets.len() as u32);
        self.sets.push(BTreeSet::new());
        self.subset.push(Vec::new());
        self.loads.push(Vec::new());
        self.stores.push(Vec::new());
        self.calls.push(Vec::new());
        self.constructs.push(Vec::new());
        self.prop_cells.insert((token, field.to_string()), id);
        id
    }

    /// Add a subset edge during solving and flush the source's existing tokens
    /// into the destination.
    fn add_subset(&mut self, from: CellId, to: CellId) {
        if self.subset[from.0 as usize].contains(&to) {
            return;
        }
        self.subset[from.0 as usize].push(to);
        let existing: Vec<TokenId> = self.sets[from.0 as usize].iter().copied().collect();
        for token in existing {
            self.push(to, token);
        }
    }

    /// Add a token to a cell during solving (vs. `seed`, which is for install).
    fn push(&mut self, cell: CellId, token: TokenId) {
        let set = &mut self.sets[cell.0 as usize];
        if set.len() >= self.budget.max_tokens_per_cell && !set.contains(&token) {
            self.exhausted = true;
            self.reasons.insert("max_tokens_per_cell".to_string());
            return;
        }
        if set.insert(token) {
            self.queue.push_back((cell, vec![token]));
        }
    }

    fn run(mut self) -> PointsToResult {
        while let Some((cell, delta)) = self.queue.pop_front() {
            for token in delta {
                self.steps += 1;
                if self.steps > self.budget.max_steps {
                    self.exhausted = true;
                    self.reasons.insert("max_steps".to_string());
                    return self.finish();
                }
                self.process(cell, token);
            }
        }
        self.finish()
    }

    fn process(&mut self, cell: CellId, token: TokenId) {
        // 1. Propagate along subset edges.
        let succ: Vec<CellId> = self.subset[cell.0 as usize].clone();
        for to in succ {
            self.push(to, token);
        }
        // 2. Field loads on this base: prop(token, field) ⊆ into, plus the same
        //    field on every prototype this token inherits (the prototype chain).
        let loads: Vec<(String, CellId)> = self.loads[cell.0 as usize].clone();
        for (field, into) in loads {
            let pc = self.prop_cell(token, &field);
            self.add_subset(pc, into);
            self.record_load_and_chase(token, &field, into);
        }
        // 3. Field stores on this base: src ⊆ prop(token, field).
        let stores: Vec<(String, CellId)> = self.stores[cell.0 as usize].clone();
        for (field, src) in stores {
            let pc = self.prop_cell(token, &field);
            self.add_subset(src, pc);
        }
        // 4. Calls with this cell as callee.
        let call_idxs: Vec<usize> = self.calls[cell.0 as usize].clone();
        if let Token::Function(payload) = self.token_by_id[token.0 as usize] {
            for idx in call_idxs {
                let spec = self.call_specs[idx].clone();
                self.edges.insert((spec.site.0, payload));
                self.wire_call(payload, &spec);
            }
        }
        // 5. Constructs (`new C()`) with this cell as callee: link the fresh
        //    instance to inherit the constructor's prototype.
        let construct_idxs: Vec<usize> = self.constructs[cell.0 as usize].clone();
        if !construct_idxs.is_empty()
            && let Token::Function(payload) = self.token_by_id[token.0 as usize]
            && let Some(proto) = self.program.class_prototypes.get(&payload).copied()
        {
            let proto_id = *self
                .program
                .token_ids
                .get(&proto)
                .expect("prototype token interned");
            for idx in construct_idxs {
                let instance = self.construct_instances[idx];
                self.link_prototype(instance, proto_id);
            }
        }
    }

    /// Record that `token` was loaded under `field` into `into`, and copy the same
    /// field from each of `token`'s current prototypes into `into`.
    fn record_load_and_chase(&mut self, token: TokenId, field: &str, into: CellId) {
        let mut seen = BTreeSet::new();
        self.chase_loads(token, field, into, &mut seen);
    }

    /// Inner walk of [`record_load_and_chase`]. `seen` breaks prototype cycles:
    /// today `inherits` is only ever populated instance→prototype (never
    /// proto→proto), so the chain is acyclic, but the guard keeps the solver core
    /// total even if a future constraint links prototypes mutually.
    fn chase_loads(
        &mut self,
        token: TokenId,
        field: &str,
        into: CellId,
        seen: &mut BTreeSet<TokenId>,
    ) {
        if !seen.insert(token) {
            return;
        }
        self.loads_on_token
            .entry(token)
            .or_default()
            .push((field.to_string(), into));
        let protos: Vec<TokenId> = self.inherits.get(&token).cloned().unwrap_or_default();
        for proto in protos {
            let pc = self.prop_cell(proto, field);
            self.add_subset(pc, into);
            // Transitive chains (prototype of a prototype).
            self.chase_loads(proto, field, into, seen);
        }
    }

    /// Make `object` inherit `proto`, flushing `object`'s prior loads against the
    /// newly-linked prototype.
    fn link_prototype(&mut self, object: TokenId, proto: TokenId) {
        let protos = self.inherits.entry(object).or_default();
        if protos.contains(&proto) {
            return;
        }
        protos.push(proto);
        let prior: Vec<(String, CellId)> = self
            .loads_on_token
            .get(&object)
            .cloned()
            .unwrap_or_default();
        for (field, into) in prior {
            let pc = self.prop_cell(proto, &field);
            self.add_subset(pc, into);
            self.record_load_and_chase(proto, &field, into);
        }
    }

    /// Wire a resolved call: arguments → parameters, return → result, this → this.
    fn wire_call(&mut self, payload: u64, spec: &CallSpec) {
        let Some(cells) = self.program.function_cells.get(&payload).cloned() else {
            return; // unknown/native function: edge recorded, no body to wire.
        };
        for (index, arg) in spec.args.iter().enumerate() {
            if let Some(param) = cells.params.get(index) {
                self.add_subset(*arg, *param);
            } else if let Some(rest) = cells.rest {
                self.add_subset(*arg, rest);
            }
        }
        if let (Some(this_arg), Some(this_cell)) = (spec.this_arg, cells.this) {
            self.add_subset(this_arg, this_cell);
        }
        if let Some(ret) = cells.ret {
            self.add_subset(ret, spec.result);
        }
    }

    fn finish(self) -> PointsToResult {
        PointsToResult {
            edges: self.edges,
            budget_exhausted: self.exhausted,
            budget_reasons: self.reasons,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Convenience builder: a function literal that lives in its own cell and
    // (optionally) has param/return cells.
    struct Build {
        p: PointsToProgram,
    }
    impl Build {
        fn new() -> Self {
            Self {
                p: PointsToProgram::new(),
            }
        }
        fn func(&mut self, payload: u64) -> (Token, CellId) {
            let token = Token::Function(payload);
            self.p.intern_token(token);
            let cell = self.p.fresh_cell();
            self.p.add(Constraint::Alloc { token, into: cell });
            (token, cell)
        }
        fn object(&mut self, alloc: u64) -> (Token, CellId) {
            let token = Token::Object(alloc);
            self.p.intern_token(token);
            let cell = self.p.fresh_cell();
            self.p.add(Constraint::Alloc { token, into: cell });
            (token, cell)
        }
    }

    fn resolves(result: &PointsToResult, site: u64, payload: u64) -> bool {
        result.edges.contains(&(site, payload))
    }

    #[test]
    fn resolves_direct_call() {
        // const f = () => {}; f();
        let mut b = Build::new();
        let (_g, gcell) = b.func(2);
        let fcell = b.p.fresh_cell();
        b.p.add(Constraint::Subset {
            from: gcell,
            to: fcell,
        }); // const f = g
        let result = b.p.fresh_cell();
        b.p.add(Constraint::Call {
            callee: fcell,
            args: vec![],
            this_arg: None,
            result,
            site: CallId(100),
        });
        let out = b.p.solve(&PointsToBudget::default());
        assert!(resolves(&out, 100, 2), "f() should resolve to g; {out:?}");
    }

    #[test]
    fn resolves_function_object_dynamic_const_key() {
        // The approx/simple.js shape: function Foo(){}; const g = ()=>{};
        // const prop="qwe"; Foo[prop]=g; Foo[prop]();  →  edge to g.
        // The const key is resolved by the (future) harvest to the string "qwe";
        // here we model that resolved key directly to validate the heap.
        let mut b = Build::new();
        let (_foo, foo) = b.func(1);
        let (_g, g) = b.func(2);
        b.p.add(Constraint::FieldStore {
            base: foo,
            field: "qwe".into(),
            src: g,
        });
        let callee = b.p.fresh_cell();
        b.p.add(Constraint::FieldLoad {
            base: foo,
            field: "qwe".into(),
            into: callee,
        });
        let result = b.p.fresh_cell();
        b.p.add(Constraint::Call {
            callee,
            args: vec![],
            this_arg: None,
            result,
            site: CallId(101),
        });
        let out = b.p.solve(&PointsToBudget::default());
        assert!(
            resolves(&out, 101, 2),
            "Foo[prop]() should resolve to g via the function-object property; {out:?}"
        );
    }

    #[test]
    fn resolves_instance_dynamic_key() {
        // const inst = new Foo(); inst[p2]=g; inst.p2();  →  edge to g.
        let mut b = Build::new();
        let (_inst, inst) = b.object(10); // the `new Foo()` allocation
        let (_g, g) = b.func(2);
        b.p.add(Constraint::FieldStore {
            base: inst,
            field: "p2".into(),
            src: g,
        });
        let callee = b.p.fresh_cell();
        b.p.add(Constraint::FieldLoad {
            base: inst,
            field: "p2".into(),
            into: callee,
        });
        let result = b.p.fresh_cell();
        b.p.add(Constraint::Call {
            callee,
            args: vec![],
            this_arg: None,
            result,
            site: CallId(102),
        });
        let out = b.p.solve(&PointsToBudget::default());
        assert!(
            resolves(&out, 102, 2),
            "inst.p2() should resolve to g; {out:?}"
        );
    }

    #[test]
    fn resolves_object_return_depth_chain() {
        // make() { return { run: leaf }; }; const o = make(); const f = o.run; f();
        // The recognizer resolves this only when `make` is a named declaration;
        // the heap resolves it through return-cell wiring regardless.
        let mut b = Build::new();
        let (_leaf, leaf) = b.func(3);
        // The object literal { run: leaf }.
        let (_lit, lit) = b.object(20);
        b.p.add(Constraint::FieldStore {
            base: lit,
            field: "run".into(),
            src: leaf,
        });
        // function make(): returns the literal. Give it a return cell.
        let make_ret = b.p.fresh_cell();
        b.p.add(Constraint::Subset {
            from: lit,
            to: make_ret,
        });
        let (_make, make_cell) = b.func(4);
        b.p.set_function_cells(
            4,
            FunctionCells {
                ret: Some(make_ret),
                ..Default::default()
            },
        );
        // const o = make();
        let o = b.p.fresh_cell();
        b.p.add(Constraint::Call {
            callee: make_cell,
            args: vec![],
            this_arg: None,
            result: o,
            site: CallId(200),
        });
        // const f = o.run;
        let f = b.p.fresh_cell();
        b.p.add(Constraint::FieldLoad {
            base: o,
            field: "run".into(),
            into: f,
        });
        // f();
        let result = b.p.fresh_cell();
        b.p.add(Constraint::Call {
            callee: f,
            args: vec![],
            this_arg: None,
            result,
            site: CallId(201),
        });
        let out = b.p.solve(&PointsToBudget::default());
        assert!(
            resolves(&out, 201, 3),
            "f() should resolve to leaf through the returned object; {out:?}"
        );
    }

    #[test]
    fn resolves_higher_order_param_flow() {
        // function apply(cb){ cb(); } apply(g);  →  cb() resolves to g.
        let mut b = Build::new();
        let (_g, g) = b.func(2);
        // apply with one param cell; its body calls the param.
        let cb_param = b.p.fresh_cell();
        let body_result = b.p.fresh_cell();
        b.p.add(Constraint::Call {
            callee: cb_param,
            args: vec![],
            this_arg: None,
            result: body_result,
            site: CallId(300), // the `cb()` site inside apply
        });
        let (_apply, apply_cell) = b.func(5);
        b.p.set_function_cells(
            5,
            FunctionCells {
                params: vec![cb_param],
                ..Default::default()
            },
        );
        // apply(g)
        let call_result = b.p.fresh_cell();
        b.p.add(Constraint::Call {
            callee: apply_cell,
            args: vec![g],
            this_arg: None,
            result: call_result,
            site: CallId(301),
        });
        let out = b.p.solve(&PointsToBudget::default());
        assert!(
            resolves(&out, 300, 2),
            "cb() inside apply should resolve to the passed g; {out:?}"
        );
    }

    #[test]
    fn resolves_new_instance_via_prototype_chain() {
        // class C { m(){} }; const c = new C(); c.m();  →  edge to m, even though
        // the instance is a fresh token (the method lives on C's prototype).
        let mut b = Build::new();
        let (_ctor, ctor_cell) = b.func(1); // the class constructor C
        let (method, method_cell) = b.func(2); // method m
        // Prototype token P carries the instance methods.
        let proto = Token::Object(900);
        b.p.intern_token(proto);
        let proto_cell = b.p.fresh_cell();
        b.p.add(Constraint::Alloc {
            token: proto,
            into: proto_cell,
        });
        b.p.add(Constraint::FieldStore {
            base: proto_cell,
            field: "m".into(),
            src: method_cell,
        });
        b.p.set_class_prototype(1, proto);
        // const c = new C();
        let inst = Token::Object(800);
        let c_cell = b.p.fresh_cell();
        b.p.add(Constraint::Alloc {
            token: inst,
            into: c_cell,
        });
        b.p.add(Constraint::Construct {
            callee: ctor_cell,
            instance: inst,
        });
        // c.m()
        let callee = b.p.fresh_cell();
        b.p.add(Constraint::FieldLoad {
            base: c_cell,
            field: "m".into(),
            into: callee,
        });
        let result = b.p.fresh_cell();
        b.p.add(Constraint::Call {
            callee,
            args: vec![],
            this_arg: None,
            result,
            site: CallId(400),
        });
        let _ = method;
        let out = b.p.solve(&PointsToBudget::default());
        assert!(
            resolves(&out, 400, 2),
            "new C().m() should resolve to m via the prototype chain; {out:?}"
        );
    }

    #[test]
    fn budget_cap_is_honest_and_terminates() {
        // A tiny program that would exceed a step budget of 1 still terminates and
        // reports the ceiling rather than looping or panicking.
        let mut b = Build::new();
        let (_g, gcell) = b.func(2);
        let c2 = b.p.fresh_cell();
        b.p.add(Constraint::Subset {
            from: gcell,
            to: c2,
        });
        let out = b.p.solve(&PointsToBudget {
            max_steps: 1,
            max_tokens_per_cell: 64,
        });
        assert!(out.budget_exhausted);
        assert!(out.budget_reasons.contains("max_steps"));
    }

    #[test]
    fn deterministic_across_runs() {
        let mut b = Build::new();
        let (_foo, foo) = b.func(1);
        let (_g, g) = b.func(2);
        b.p.add(Constraint::FieldStore {
            base: foo,
            field: "qwe".into(),
            src: g,
        });
        let callee = b.p.fresh_cell();
        b.p.add(Constraint::FieldLoad {
            base: foo,
            field: "qwe".into(),
            into: callee,
        });
        let result = b.p.fresh_cell();
        b.p.add(Constraint::Call {
            callee,
            args: vec![],
            this_arg: None,
            result,
            site: CallId(101),
        });
        let a = b.p.solve(&PointsToBudget::default());
        let b2 = b.p.solve(&PointsToBudget::default());
        assert_eq!(a, b2);
    }
}
