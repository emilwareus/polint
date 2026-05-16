# Semantic Index Pipeline Algorithms

This file describes the implementation-neutral pipeline polint should use.

## Pipeline Overview

```python
def build_semantic_index(project, demand, extensions):
    files = discover_files(project)
    syntax = parse_files(files, demand.languages)

    declarations = {}
    scopes = {}
    imports = {}
    references = {}

    for lang in demand.languages:
        scopes[lang], declarations[lang], references[lang], imports[lang] = (
            lang_provider(lang).bind_syntax(syntax[lang])
        )

    module_graph = resolve_modules(project, imports)
    import_resolutions = resolve_imports(imports, module_graph)
    aliases = resolve_aliases_and_reexports(import_resolutions, declarations)

    resolution_steps = []
    for lang in demand.languages:
        references[lang], steps = lang_provider(lang).resolve_references(
            scopes=scopes[lang],
            declarations=declarations[lang],
            imports=import_resolutions[lang],
            aliases=aliases[lang],
            module_graph=module_graph,
        )
        resolution_steps.extend(steps)

    extension_facts = run_extensions(extensions, current_facts())
    merged = validate_and_merge(extension_facts, current_facts())

    xref_index = build_xref_index(merged.symbols, merged.references)
    return SemanticIndex(merged, xref_index, resolution_steps)
```

## Provider Contract

```python
class SemanticProvider:
    id: ProviderId
    language: Language
    version: ProviderVersion
    inputs: list[FactFamily]
    outputs: list[FactFamily]

    def bind_syntax(self, syntax) -> BoundSyntaxFacts:
        ...

    def resolve_references(self, facts) -> ResolvedReferenceFacts:
        ...
```

Providers must be deterministic:

- stable input ordering;
- stable ID allocation;
- sorted outputs;
- no wall-clock or process-random values in cache keys;
- diagnostics for non-convergence.

## Binding Algorithm

```python
def bind_file(file_ast, language_rules):
    scopes = ScopeArena()
    symbols = SymbolArena()
    refs = []
    imports = []

    scope_stack = [scopes.alloc(kind=language_rules.file_scope_kind(file_ast))]

    for event in language_rules.traverse(file_ast):
        match event:
            case EnterScope(node):
                parent = scope_stack[-1]
                scope_stack.append(scopes.alloc(parent=parent, owner=owner_symbol(node)))

            case Declare(node):
                symbol = symbols.alloc(
                    stable_key=make_stable_key(node, scope_stack[-1]),
                    kind=language_rules.symbol_kind(node),
                    namespace=language_rules.namespace(node),
                    declarations=[span(node)],
                )
                scopes[scope_stack[-1]].declare(symbol)

            case Import(node):
                imports.append(import_fact(node, scope_stack[-1]))

            case Reference(node):
                refs.append(reference_fact(node, scope_stack[-1]))

            case LeaveScope(_node):
                scope_stack.pop()

    return scopes, symbols, refs, imports
```

## Lexical Lookup

```python
def lexical_lookup(ref, scopes, symbols, language_rules):
    name = ref.spelling
    namespace = language_rules.reference_namespace(ref)
    scope = scopes[ref.enclosing_scope]

    while scope is not None:
        candidates = scope.lookup(name, namespace)
        if candidates:
            return Resolution(
                candidates=candidates,
                selected=select_if_unambiguous(candidates),
                status="ExactLocal" if len(candidates) == 1 else "Ambiguous",
                precision="BinderExact" if len(candidates) == 1 else "Conservative",
            )
        scope = scopes[scope.parent]

    return Resolution(status="Unresolved", candidates=[])
```

## Import Resolution

```python
def resolve_imports(imports, module_graph):
    results = []
    for imp in imports:
        candidates = module_graph.lookup(imp.from_module, imp.import_path)
        if len(candidates) == 1:
            results.append(resolved_import(imp, candidates[0]))
        elif len(candidates) > 1:
            results.append(ambiguous_import(imp, candidates))
        elif module_graph.is_external(imp.import_path):
            results.append(external_import(imp))
        elif imp.is_dynamic:
            results.append(dynamic_import(imp))
        else:
            results.append(missing_import(imp))
    return results
```

## Alias/Reexport Fixpoint

```python
def alias_reexport_fixpoint(imports, exports, declarations, extension_aliases):
    aliases = seed_aliases(imports, exports, declarations, extension_aliases)
    seen_state = None

    for iteration in range(MAX_ALIAS_ITERATIONS):
        state = aliases.digest()
        if state == seen_state:
            return aliases
        seen_state = state

        for alias in aliases.pending():
            targets = resolve_alias_targets(alias, aliases)
            aliases.update(alias, targets)

    emit_diagnostic("semantic/alias-fixpoint-limit")
    aliases.mark_unresolved_cycles()
    return aliases
```

## Type-Assisted Resolution

```python
def type_assisted_resolution(ref, candidates, type_facts):
    if not ref.requires_type_assist():
        return candidates

    receiver_type = type_facts.receiver_type(ref)
    if receiver_type is None:
        return unresolved("TypeRequired")

    members = type_facts.members(receiver_type, ref.member_name)
    if len(members) == 1:
        return exact(members[0], status="TypeAssisted")
    if members:
        return ambiguous(members)
    return unresolved("NoMember")
```

Type-assisted resolution must be a separate provider layer so rules can distinguish binder-exact facts from type-dependent facts.

## Extension Merge

```python
def validate_and_merge(extension_facts, native_facts):
    merged = native_facts.copy()

    for fact in extension_facts:
        schema_check(fact)
        referential_check(fact, merged)
        conflict = detect_conflict(fact, merged)

        if conflict and conflict.native_precision == "BinderExact":
            emit_conflict_diagnostic(fact, conflict)
            merged.add_side_table_fact(fact, validation="RejectedConflict")
            continue

        merged.add(fact, provenance="Extension", validation=validation_status(fact))

    return merged
```

## Xref Index

```python
def build_xref_index(symbols, references):
    by_name = defaultdict(list)
    by_symbol = defaultdict(list)
    by_file = defaultdict(list)
    by_scope = defaultdict(list)

    for sym in symbols:
        by_name[sym.name].append(sym.id)
        by_scope[sym.scope].append(sym.id)

    for ref in references:
        by_name[ref.spelling].append(ref.id)
        by_file[ref.file].append(ref.id)
        if ref.chosen:
            by_symbol[ref.chosen].append(ref.id)

    return sort_and_freeze(by_name, by_symbol, by_file, by_scope)
```

Start with sorted vectors and hash maps. Compact FST/bitset encodings should be introduced only when benchmark data shows the need.
