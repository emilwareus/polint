# Algorithm: Exceptional, Cleanup, And Async CFG

Exceptional and cleanup flow is the main place where naive CFGs become misleading. This file gives pseudo-code patterns for modeling it without claiming impossible precision.

## Control Context Stack

```python
class ControlContext:
    def __init__(self):
        self.loop_stack = []
        self.exception_stack = []
        self.cleanup_stack = []
        self.return_target = None

    def active_cleanup_path(self, final_target):
        target = final_target
        for cleanup in reversed(self.cleanup_stack):
            target = cleanup.entry_for(target)
        return target
```

## Try / Catch / Finally

```python
def build_try_catch_finally(builder, stmt):
    after = builder.new_block("AfterTry")
    catch_entry = builder.new_block("Catch") if stmt.catch else None
    finally_entry = builder.new_block("Finally") if stmt.finally else None

    normal_after_try = finally_entry or after
    exception_after_try = catch_entry or finally_entry or builder.exception_exit

    with builder.context.exception_handler(exception_after_try):
        with builder.context.cleanup(finally_entry):
            build_stmt_list(builder, stmt.try_body)
            if not builder.current.terminated:
                builder.edge(builder.last_node(), normal_after_try.first_or_placeholder(), "Normal")

    if stmt.catch:
        builder.current = catch_entry
        bind_catch_parameter(builder, stmt.catch)
        build_stmt_list(builder, stmt.catch.body)
        if not builder.current.terminated:
            builder.edge(builder.last_node(), finally_entry.first_or_placeholder() if finally_entry else after.first_or_placeholder(), "Normal")

    if stmt.finally:
        builder.current = finally_entry
        build_stmt_list(builder, stmt.finally.body)
        if not builder.current.terminated:
            # This edge may represent multiple resumptions. Mark if merged.
            builder.edge(builder.last_node(), after.first_or_placeholder(), "Finally", precision="Conservative")

    builder.current = after
```

This version is conservative. Exact return/throw/break/continue-through-finally requires either:

- duplicating the finally body per reason; or
- using reason-carrying synthetic continuation nodes.

## Reason-Carrying Finally

```python
class FinallyFrame:
    def __init__(self, body):
        self.entry = new_block("FinallyEnter")
        self.exits_by_reason = {}

    def entry_for(self, reason_target):
        token = new_cleanup_reason(reason_target)
        self.exits_by_reason[token] = reason_target
        return self.entry.with_reason(token)

def leave_through_finally(builder, reason, target):
    cleanup_target = builder.context.active_cleanup_path(target)
    node = builder.append(reason.kind)
    builder.edge(node, cleanup_target, reason.edge_kind)
```

This preserves feasibility better but requires carrying a reason token through cleanup. It is more complex, but useful for max-capability path evidence.

## Go `defer`

```python
def build_defer(builder, stmt):
    call_node = build_call_expr(builder, stmt.call)
    defer_node = builder.append("Defer", stmt.span)
    builder.defer_stack.push(defer_node)

def build_return_go(builder, stmt):
    build_return_values(builder, stmt.values)
    run_defers = builder.append("RunDefers")
    for defer_node in reversed(builder.defer_stack):
        builder.edge(run_defers, defer_node, "Defer")
    builder.edge(run_defers, builder.normal_exit, "Return")
```

For panic:

```python
def build_panic(builder, stmt):
    node = builder.append("Panic", stmt.span)
    run_defers = builder.append("RunDefers", precision="Semantic")
    builder.edge(node, run_defers, "Panic")
    if builder.recover_block:
        builder.edge(run_defers, builder.recover_block, "Recover", precision="Conservative")
    builder.edge(run_defers, builder.exception_exit, "Panic")
```

## Python `with`

```python
def build_with(builder, stmt):
    enter = build_call(builder, stmt.manager, method="__enter__")
    exit_func = capture_exit(builder, stmt.manager)
    after = builder.new_block("AfterWith")
    handler = builder.new_block("WithException")

    with builder.context.exception_handler(handler):
        build_stmt_list(builder, stmt.body)
        if not builder.current.terminated:
            normal_exit = build_call(builder, exit_func, args=[None, None, None])
            builder.edge(normal_exit, after.first_or_placeholder(), "WithExit")

    builder.current = handler
    suppress = build_call(builder, exit_func, args=["exc_type", "exc", "tb"])
    builder.edge(suppress, after.first_or_placeholder(), "ExceptionSuppressed", precision="Conservative")
    builder.edge(suppress, builder.exception_exit, "Raise", precision="Conservative")

    builder.current = after
```

## Java Try-With-Resources

```python
def build_try_with_resources(builder, stmt):
    resources = []
    for resource in stmt.resources:
        resources.append(build_resource_init(builder, resource))

    with builder.context.cleanup(close_resources_reversed(resources)):
        build_try_catch_finally(builder, stmt.without_resources())
```

Resource close edges:

```python
def close_resources_reversed(resources):
    def cleanup(builder, final_target):
        target = final_target
        for resource in reversed(resources):
            close_block = builder.new_block("ResourceClose")
            builder.edge(close_block.first, target.first_or_node(), "ResourceClose")
            builder.edge(close_block.first, builder.exception_exit, "SuppressedException", precision="Conservative")
            target = close_block
        return target
    return cleanup
```

## Async And Generators

For first slice, model suspend/resume markers without scheduler interleavings.

```python
def build_await(builder, expr):
    build_expr(builder, expr.awaited)
    suspend = builder.append("AwaitSuspend", expr.span)
    resume = builder.append("AwaitResume", expr.span)
    builder.edge(suspend, resume, "AwaitResume", precision="ExactSyntax")

    if builder.context.has_exception_handler():
        builder.edge(suspend, builder.context.exception_target(), "AsyncReject", precision="Conservative")
```

```python
def build_yield(builder, expr):
    build_expr(builder, expr.value)
    suspend = builder.append("YieldSuspend", expr.span)
    resume = builder.append("YieldResume", expr.span)
    builder.edge(suspend, resume, "YieldResume")
```

Do not infer cross-task or cross-event-loop execution from these edges. That belongs to a future async effects/lifecycle layer.

## Validation Rule

Any construct that can create hidden control transfer must either:

1. emit explicit edge facts; or
2. emit `UnsupportedControlFlowFact`; or
3. produce a capability diagnostic if requested by a rule.

Silent omission is not acceptable for max-capability analysis.
