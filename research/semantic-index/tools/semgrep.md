# Semgrep

## What It Is

Semgrep is a fast multi-language static analysis tool focused on source-like pattern matching and security rules. For semantic indexes, it is most useful as a reference for pragmatic rule ergonomics and the limits of generic AST naming.

Primary inspected files:

- `src/naming/Naming_AST.ml`
- `src/naming/Naming_utils.ml`
- `cli/src/semgrep/symbol_analysis.py`
- `cli/src/semgrep/semgrep_interfaces/semgrep_output_v1_t.mli`

## Index Shape

Core objects:

- **AST_generic:** common syntax representation across many languages.
- **Naming pass:** annotates identifiers with resolved-name references where possible.
- **Scopes:** global, block, imported, function/class context.
- **Symbol analysis output:** FQN plus source locations.
- **RPC orchestration:** subproject-level symbol analysis by ecosystem/subproject.

Semgrep's naming layer is intentionally generic. It can resolve many useful names but does not aim to match each compiler's full semantic model.

## Algorithm

```python
def generic_naming(ast_generic):
    env = ScopeStack()
    for node in walk(ast_generic):
        if node.opens_scope():
            env.push(node)
        if node.imports_name():
            env.add_import(node.name, node.fqn)
        if node.declares_name():
            env.add_symbol(node.name, current_fqn(node))
        if node.uses_identifier():
            resolved = env.lookup(node.name)
            node.resolved_name = resolved
        if node.closes_scope():
            env.pop()
    return ast_generic

def symbol_analysis(project):
    subprojects = split_by_ecosystem(project)
    for subproject in subprojects:
        yield rpc_symbol_analysis(subproject)
```

## Accuracy

Semgrep is accurate enough for many pattern rules because those rules often match local syntax and local bindings. It is not a complete semantic index:

- language-specific name resolution is incomplete;
- type-aware member resolution is not the central model;
- framework/generated/dynamic symbols require rules/models;
- comments in implementation show TODOs and limitations for language-specific import/type behavior.

This is not a criticism; it is the tradeoff that makes Semgrep broad and ergonomic.

## Complexity

Generic naming is roughly linear in generic AST size:

```text
O(N + names)
```

Rule matching cost depends heavily on pattern structure, metavariables, and language normalization. Symbol analysis orchestration depends on subproject count and RPC work.

## Strengths

- Excellent rule authoring ergonomics.
- Broad language support.
- Fast local matching.
- Useful generic AST normalization.
- Shows how far a lightweight semantic layer can go.

## Weaknesses

- Generic naming ceiling is too low for polint's "most capable" goal.
- Does not replace language-specific compiler semantics.
- Multi-language breadth trades off against per-language exactness.

## Polint Implications

Copy:

- rule ergonomics;
- source-like matching concepts for future rule SDK;
- subproject/ecosystem partitioning idea;
- willingness to label limitations.

Avoid:

- building the main semantic index on generic AST naming;
- presenting broad syntax matches as exact semantic references.

Recommended role:

```text
Semgrep is a product/ergonomics reference.
It is not the semantic-index accuracy baseline.
```

polint can support simple syntactic rules, but its differentiator should be typed semantic facts plus repo-local Rust extensions.
