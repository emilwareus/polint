# Algorithm: CFG Construction

This is stripped-down pseudo-code for the native CFG builder polint should implement.

## Core Builder

```python
class Builder:
    def __init__(self, function):
        self.function = function
        self.nodes = []
        self.blocks = []
        self.edges = []
        self.context = ControlContext()

        self.entry = self.new_node("Entry")
        self.normal_exit = self.new_node("ExitNormal")
        self.exception_exit = self.new_node("ExitExceptional")
        self.current = self.new_block(kind="Entry", first=self.entry)

    def append(self, kind, span=None, anchor=None):
        node = self.new_node(kind, span, anchor)
        self.edge(self.last_node(), node, "Normal")
        self.current.add(node)
        return node

    def edge(self, frm, to, kind, label=None, precision="ExactSyntax"):
        self.edges.append(Edge(frm, to, kind, label, precision))

    def split(self, kind="Normal"):
        block = self.new_block(kind=kind)
        self.current = block
        return block

    def terminate(self, kind, target):
        node = self.append(kind)
        self.edge(node, target, kind)
        self.current.mark_terminated()
        self.current = self.new_block(kind="Unreachable")
        return node
```

## Branches

```python
def build_if(builder, stmt):
    cond = build_condition(builder, stmt.condition)

    then_block = builder.new_block("Then")
    else_block = builder.new_block("Else")
    join_block = builder.new_block("Join")

    builder.edge(cond, then_block.first_or_placeholder(), "True")
    builder.edge(cond, else_block.first_or_placeholder(), "False")

    builder.current = then_block
    build_stmt_list(builder, stmt.then_body)
    if not builder.current.terminated:
        builder.edge(builder.last_node(), join_block.first_or_placeholder(), "Normal")

    builder.current = else_block
    build_stmt_list(builder, stmt.else_body)
    if not builder.current.terminated:
        builder.edge(builder.last_node(), join_block.first_or_placeholder(), "Normal")

    builder.current = join_block
```

## Loops

```python
def build_loop(builder, loop):
    header = builder.new_block("LoopHeader")
    body = builder.new_block("LoopBody")
    exit = builder.new_block("LoopExit")

    builder.edge(builder.last_node(), header.first_or_placeholder(), "LoopEnter")

    with builder.context.loop(continue_target=header, break_target=exit):
        builder.current = header
        cond = build_condition(builder, loop.condition)
        builder.edge(cond, body.first_or_placeholder(), "True")
        builder.edge(cond, exit.first_or_placeholder(), "False")

        builder.current = body
        build_stmt_list(builder, loop.body)
        if not builder.current.terminated:
            builder.edge(builder.last_node(), header.first_or_placeholder(), "LoopBack")

    builder.current = exit
```

## Break / Continue

```python
def build_break(builder, stmt):
    target = builder.context.resolve_break(stmt.label)
    node = builder.append("Break", stmt.span)
    builder.edge(node, target.first_or_placeholder(), "Break")
    builder.current.mark_terminated()
    builder.current = builder.new_block("Unreachable")

def build_continue(builder, stmt):
    target = builder.context.resolve_continue(stmt.label)
    node = builder.append("Continue", stmt.span)
    builder.edge(node, target.first_or_placeholder(), "Continue")
    builder.current.mark_terminated()
    builder.current = builder.new_block("Unreachable")
```

## Return / Throw / Panic

```python
def build_return(builder, stmt):
    for expr in stmt.values:
        build_expr(builder, expr)
    node = builder.append("Return", stmt.span)
    target = builder.context.return_target_through_cleanups(builder.normal_exit)
    builder.edge(node, target.first_or_node(), "Return")
    builder.current.mark_terminated()
    builder.current = builder.new_block("Unreachable")

def build_throw(builder, stmt):
    build_expr(builder, stmt.expr)
    node = builder.append("Throw", stmt.span)
    target = builder.context.exception_target_through_cleanups(builder.exception_exit)
    builder.edge(node, target.first_or_node(), "Throw")
    builder.current.mark_terminated()
    builder.current = builder.new_block("Unreachable")
```

## Short-Circuit Expressions

```python
def build_logical_and(builder, expr):
    left = build_expr_as_condition(builder, expr.left)
    right_block = builder.new_block("LogicalRight")
    false_block = builder.new_block("LogicalFalse")
    join = builder.new_block("LogicalJoin")

    builder.edge(left, right_block.first_or_placeholder(), "True")
    builder.edge(left, false_block.first_or_placeholder(), "False")

    builder.current = right_block
    right = build_expr_as_condition(builder, expr.right)
    builder.edge(right, join.first_or_placeholder(), "Normal")

    builder.current = false_block
    synthetic_false = builder.append("SyntheticFalse", expr.left.span)
    builder.edge(synthetic_false, join.first_or_placeholder(), "ShortCircuit")

    builder.current = join
    return join.result_node()
```

## Invariant Validation

```python
def validate(cfg):
    assert cfg.entry is not None
    assert cfg.normal_exit is not None
    for edge in cfg.edges:
        assert edge.from_node in cfg.nodes
        assert edge.to_node in cfg.nodes
        assert edge.kind != "Normal" or not edge.represents_exception
    assert deterministic_order(cfg.nodes)
    assert deterministic_order(cfg.edges)
    assert no_duplicate_edges(cfg.edges)
```

## Complexity

The builder is linear in lowered operation count plus emitted edges:

```text
O(N + E)
```

Exception/finally/cleanup modeling can add edges or duplicate cleanup bodies. The first implementation should prefer synthetic cleanup/finalizer nodes with precision labels; exact duplication can be added for specific languages/constructs after benchmarks justify it.
