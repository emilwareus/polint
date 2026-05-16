# Andersen Solver Notes

## Why Andersen

Andersen-style inclusion constraints are the best baseline because they preserve subset relationships instead of merging variables eagerly. They are more precise than Steensgaard/unification and are widely used as a conceptual foundation for production pointer-analysis engines.

The downside is cost. Naive Andersen solving has poor worst-case complexity, commonly described as cubic. A production-grade implementation depends on engineering techniques, not just the textbook rules.

## Constraint Vocabulary

```text
AddressOf(dst, obj)        obj in Pt(dst)
Copy(dst, src)             Pt(src) subset Pt(dst)
Load(dst, p)               for o in Pt(p): Pt(o.contents) subset Pt(dst)
Store(p, src)              for o in Pt(p): Pt(src) subset Pt(o.contents)
FieldLoad(dst, base, f)    for o in Pt(base): Pt(o.f) subset Pt(dst)
FieldStore(base, f, src)   for o in Pt(base): Pt(src) subset Pt(o.f)
```

## Engineering Requirements

### Dense IDs

Every place, object token, field variable, and constraint variable should have a dense integer ID.

```text
PtVar(0..N)
ObjectToken(0..M)
FieldKey(0..K)
```

Dense IDs enable compact bitsets and deterministic ordering.

### Bitsets

Use bitsets for points-to sets. The critical operation is `add_all(delta)`.

```python
def add_all(dst, delta):
    new = delta - pt[dst]
    if new:
        pt[dst] |= new
        enqueue(dst, new)
```

### Delta Propagation

Only propagate newly added objects, not entire sets.

```python
while queue:
    var, new_objects = queue.pop()
    propagate_delta(var, new_objects)
```

### SCC Collapsing

Copy constraints create graph cycles. Collapse SCCs before or during solving:

```python
for scc in tarjan(copy_graph):
    if len(scc) > 1:
        merged = merge_vars(scc)
        redirect_edges(scc, merged)
```

This is essential for recursive assignments and common value-flow cycles.

### Field Sensitivity

Field-sensitive variables:

```text
object_field(object_token, field_key)
```

Field-based fallback:

```text
global_field(field_key)
```

Recommendation:

- default to field-sensitive for object literals, structs, classes, and known shapes;
- collapse to field-based for dynamic/unknown shapes or when budgets are exceeded;
- preserve precision labels when collapsing.

### Type Filtering

Use type facts to avoid impossible propagation:

```python
if not type_compatible(var, object_token):
    skip_with_evidence("type-disjoint")
```

This is a major advantage of building type facts first.

### Budgets

Budgets should include:

- max variables;
- max objects per variable;
- max dynamic edges created;
- max propagation steps;
- max field variables;
- max context instances.

Budget exhaustion must emit an `Unknown` fact, not silently truncate to a false precise result.

## Solver Pseudo-Code

```python
class Solver:
    def __init__(self, constraints, budget):
        self.pt = BitsetMap()
        self.delta = Queue()
        self.copy = Graph()
        self.loads = MultiMap()
        self.stores = MultiMap()
        self.field_loads = MultiMap()
        self.field_stores = MultiMap()
        self.budget = budget

    def initialize(self):
        for c in constraints:
            if c.kind == "AddressOf":
                self.add_object(c.dst, c.object)
            elif c.kind == "Copy":
                self.add_copy_edge(c.src, c.dst)
            elif c.kind == "Load":
                self.loads[c.pointer].append(c.dst)
            elif c.kind == "Store":
                self.stores[c.pointer].append(c.src)
            elif c.kind == "FieldLoad":
                self.field_loads[c.base].append((c.field, c.dst))
            elif c.kind == "FieldStore":
                self.field_stores[c.base].append((c.field, c.src))

        self.collapse_copy_sccs()
        self.seed_copy_edges()

    def add_object(self, var, obj):
        if self.pt[var].insert(obj):
            self.delta.push(var, {obj})

    def add_copy_edge(self, src, dst):
        if self.copy.add(src, dst):
            existing = self.pt[src]
            if existing:
                self.add_all(dst, existing)

    def add_all(self, dst, objects):
        new = objects - self.pt[dst]
        if new:
            self.pt[dst] |= new
            self.delta.push(dst, new)

    def solve(self):
        self.initialize()
        while self.delta and self.budget.ok():
            var, new_objects = self.delta.pop()

            for dst in self.copy.successors(var):
                self.add_all(dst, new_objects)

            for dst in self.loads[var]:
                for obj in new_objects:
                    self.add_copy_edge(contents_var(obj), dst)

            for src in self.stores[var]:
                for obj in new_objects:
                    self.add_copy_edge(src, contents_var(obj))

            for field, dst in self.field_loads[var]:
                for obj in new_objects:
                    self.add_copy_edge(field_var(obj, field), dst)

            for field, src in self.field_stores[var]:
                for obj in new_objects:
                    self.add_copy_edge(src, field_var(obj, field))

        if not self.budget.ok():
            return PartialPointsTo(self.pt, Unknown("budget exhausted"))
        return CompletePointsTo(self.pt)
```

## Validation Fixtures

Minimum tests:

- copy chains;
- pointer load/store;
- field load/store;
- recursive cycles;
- function object flow;
- object literal property flow;
- interface/dynamic dispatch target flow;
- unknown dynamic key collapse;
- budget exhaustion;
- extension-provided constraint merge;
- source evidence for a points-to entry.

## Why Not Steensgaard As Default

Steensgaard's unification is much cheaper because it merges variables instead of preserving subset relationships. That is also why it is too coarse for polint's goal. It will quickly turn useful repo-specific rules into noisy `MayAlias` answers.

Use it only as:

- an emergency fallback;
- a quick pre-pass to find coarse components;
- a comparison baseline in the evaluation harness.
