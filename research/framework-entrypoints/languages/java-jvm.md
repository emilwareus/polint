# Java / JVM Framework Entrypoint Research

Java/JVM is future-facing for polint until a Java adapter exists. It is still crucial design input because it stresses annotations, meta-annotations, DI, reflection, generated code, servlet filters, test runners, and Android lifecycle.

## Core Recommendation

Treat JVM framework support as staged entrypoint/modeling facts, not just a Java call graph.

```text
classes/methods/annotations
  -> annotation closure
  -> framework components
  -> route/lifecycle/DI/reflection facts
  -> synthetic dispatch overlay
  -> call graph/data flow
```

## Fact Families

| Fact | Meaning |
|---|---|
| `JavaAnnotations<'_>` | Annotations, meta-annotations, retention, targets, values, aliases/composed relationships. |
| `JvmEntrypoints<'_>` | Framework/test/lifecycle entrypoints with trigger metadata. |
| `HttpEndpoints<'_>` | Path, method, params, headers, consumes/produces, request-bound parameters. |
| `DiGraph<'_>` | Beans/components/factory methods/injection points/candidates/status. |
| `JvmCallGraph<'_>` | Direct, virtual, CHA/RTA/points-to, reflective, framework dispatch, generated, lifecycle edges. |
| `ReflectionFacts<'_>` | `Class.forName`, method lookup, invoke, resolved/unresolved strings. |
| `JavaTests<'_>` | JUnit/TestNG tests and lifecycle hooks. |
| `AndroidLifecycle<'_>` | Manifest components, callbacks, lifecycle transition edges. |
| `GeneratedDispatch<'_>` | Generated sources/classes and dispatch edges. |

## Spring MVC

Recognize:

- `@Controller`
- `@RestController`
- `@RequestMapping`
- `@GetMapping`, `@PostMapping`, etc.
- composed/meta-annotations;
- class + method path composition;
- parameter annotations: `@RequestParam`, `@PathVariable`, `@RequestBody`, `@RequestHeader`, `@CookieValue`, etc.

Pseudo-code:

```python
def recover_spring(classes):
    annotation_index = build_annotation_closure(classes)

    for cls in classes:
        if not has_annotation_closure(cls, ["Controller", "RestController"]):
            continue

        class_mapping = request_mapping(cls)

        for method in cls.methods:
            method_mapping = request_mapping(method)
            if not method_mapping:
                continue

            route = compose(class_mapping, method_mapping)
            inputs = []
            for param in method.params:
                inputs.extend(request_sources_from_annotations(param))

            emit_http_endpoint(route, method.symbol, inputs)
```

## Servlets And Filters

Recognize:

- subclasses of `HttpServlet`;
- `doGet`, `doPost`, etc.;
- servlet mappings from annotations/config;
- `Filter.doFilter`;
- listeners and context lifecycle.

Synthetic edges:

```text
ServletContainer -> FilterChain -> Servlet.service -> doGet/doPost
```

## JAX-RS

Recognize:

- `@Path` on class/method;
- HTTP method annotations;
- sub-resource locators;
- `@PathParam`, `@QueryParam`, `@HeaderParam`, `@BeanParam`, etc.

## DI

DI affects both call graph and framework facts.

Recognize:

- `@Component`, `@Service`, `@Repository`, `@Controller`;
- `@Bean` factory methods;
- `@Autowired`, `@Inject`;
- qualifiers and profiles;
- constructor/field/method injection;
- ambiguous or missing dependency status.

Do not hide ambiguity. Emit DI unknown/conflict facts.

## Reflection

Reflection should not be ignored.

First tier:

- constant/string propagation for `Class.forName`;
- `getMethod`, `getDeclaredMethod`;
- constructor lookup;
- `Method.invoke`;
- unresolved reflective calls as explicit facts.

## Android

FlowDroid's lesson:

- Android lifecycle and callbacks need explicit synthetic entrypoints.
- A dummy-main-like dispatch graph can seed reachability and data flow.
- Callback discovery must be bounded and filtered.

polint should generalize this to lifecycle facts rather than hardcode Android as the only lifecycle model.

## Call Graph Tiers

| Tier | Meaning |
|---|---|
| Direct | Syntactic calls with exact target. |
| CHA | Class Hierarchy Analysis for virtual dispatch. |
| RTA | Rapid Type Analysis to prune by instantiated classes. |
| Points-to | More precise pointer-analysis-backed call graph. |
| Context-sensitive | 0-CFA/1-CFA/object-sensitive variants. |
| Framework dispatch | Synthetic edges from framework facts. |
| Reflective | Constant-resolved or unresolved reflection edges. |

## Benchmarks

Use:

- SecuriBench Micro;
- DroidBench;
- JCG/CATS for call graph unsoundness;
- OWASP Benchmark Java;
- Spring PetClinic as smoke/reference app;
- native fixtures for Spring, servlet, JAX-RS, DI, JUnit, reflection, Android.

## Limits

- classpath/build setup;
- Lombok/generated code;
- Spring AOT/proxy generation;
- profile/condition-dependent beans;
- reflection/dynamic class loading;
- bytecode-only dependencies.

Capability diagnostics must be explicit for missing setup.
