# Repository Index

All third-party repositories are cloned under `research/agent-extension-surface/repos/` or reused from `research/data-flow/repos/`. The `repos/` folders are gitignored and should stay local.

## Newly Cloned For This Track

| System | Repo | Commit Inspected | Why It Matters |
|---|---|---:|---|
| Dylint | `https://github.com/trailofbits/dylint.git` | `694ffa304fdf0de62ed1277e2eedef2986f643e3` | Rust-code lint libraries, dynamic loading, toolchain-qualified plugin artifacts, repo-local lint authoring. |
| Error Prone | `https://github.com/google/error-prone.git` | `960e04b8c6269512bab29bc7eac1fa6669961d1a` | Java compiler-integrated semantic checkers, `BugChecker`, matcher interfaces, strong fixture testing. |
| OpenRewrite | `https://github.com/openrewrite/rewrite.git` | `0f600f466394ec6043880ba8270889a3b289ace8` | Recipe lifecycle, visitors, scanning recipes, validation, multi-file scan -> generate -> transform model. |
| ESLint | `https://github.com/eslint/eslint.git` | `6616856f28fa514a30f87b5539fc100d739a94bf` | Best-known custom rule/plugin ergonomics, `RuleTester`, AST visitor activation, context shape. |

## Reused From Data-Flow Research

| System | Local Repo | Commit Inspected | Why It Matters |
|---|---|---:|---|
| CodeQL | `research/data-flow/repos/codeql` | `a84332ac150ebbbe57d46af4a40cf49f8fc8568a` | Models-as-data, extensible predicates, source/sink/summary/barrier taxonomy, provenance columns. |
| Pyre/Pysa | `research/data-flow/repos/pyre-check` | `34af3721bc04bcbc9a9a1b05b59195ac35a88d03` | Taint models, model generators, model DSL, project-specific coverage expansion. |
| Semgrep | `research/data-flow/repos/semgrep` | `db2be62416a2f777f74b71f62ac0f9eea28863de` | Rule-level taint modeling, propagators, side-effect taint, trace reporting. |
| Joern | `research/data-flow/repos/joern` | `6f93016d9413f2b46b7d1cbef53e78de272d60f5` | Code property graph, overlays/layers, custom passes, graph query model. |

## Key Local Files Inspected

### Dylint

- `research/agent-extension-surface/repos/dylint/README.md`
- `research/agent-extension-surface/repos/dylint/docs/how_dylint_works.md`
- `research/agent-extension-surface/repos/dylint/driver/src/lib.rs`
- `research/agent-extension-surface/repos/dylint/dylint/src/lib.rs`
- `research/agent-extension-surface/repos/dylint/dylint-link/src/main.rs`
- `research/agent-extension-surface/repos/dylint/utils/linting/src/lib.rs`
- `research/agent-extension-surface/repos/dylint/examples/restriction/try_io_result/src/lib.rs`

### Error Prone

- `research/agent-extension-surface/repos/error-prone/check_api/src/main/java/com/google/errorprone/bugpatterns/BugChecker.java`
- `research/agent-extension-surface/repos/error-prone/check_api/src/main/java/com/google/errorprone/VisitorState.java`
- `research/agent-extension-surface/repos/error-prone/check_api/src/main/java/com/google/errorprone/ErrorPronePlugins.java`
- `research/agent-extension-surface/repos/error-prone/core/src/main/java/com/google/errorprone/bugpatterns/ArrayHashCode.java`
- `research/agent-extension-surface/repos/error-prone/test_helpers/src/main/java/com/google/errorprone/CompilationTestHelper.java`
- `research/agent-extension-surface/repos/error-prone/test_helpers/src/main/java/com/google/errorprone/BugCheckerRefactoringTestHelper.java`

### OpenRewrite

- `research/agent-extension-surface/repos/rewrite/rewrite-core/src/main/java/org/openrewrite/Recipe.java`
- `research/agent-extension-surface/repos/rewrite/rewrite-core/src/main/java/org/openrewrite/ScanningRecipe.java`
- `research/agent-extension-surface/repos/rewrite/rewrite-core/src/main/java/org/openrewrite/TreeVisitor.java`
- `research/agent-extension-surface/repos/rewrite/rewrite-test/src/main/java/org/openrewrite/test/RewriteTest.java`
- `research/agent-extension-surface/repos/rewrite/rewrite-json/src/main/java/org/openrewrite/json/ChangeValue.java`

### ESLint

- `research/agent-extension-surface/repos/eslint/docs/src/extend/custom-rules.md`
- `research/agent-extension-surface/repos/eslint/docs/src/extend/plugins.md`
- `research/agent-extension-surface/repos/eslint/lib/linter/linter.js`
- `research/agent-extension-surface/repos/eslint/lib/rule-tester/rule-tester.js`
- `research/agent-extension-surface/repos/eslint/docs/_examples/custom-rule-tutorial-code/enforce-foo-bar.js`

### CodeQL

- `research/data-flow/repos/codeql/actions/ql/lib/codeql/actions/dataflow/internal/ExternalFlowExtensions.qll`
- `research/data-flow/repos/codeql/actions/ql/lib/codeql/actions/dataflow/ExternalFlow.qll`
- `research/data-flow/repos/codeql/go/ql/lib/ext/**/*.model.yml`

### Pysa

- `research/data-flow/repos/pyre-check/documentation/website/docs/pysa_precollectors.md`
- `research/data-flow/repos/pyre-check/documentation/website/docs/pysa_model_dsl.md`
- `research/data-flow/repos/pyre-check/tools/generate_taint_models/model_generator.py`

### Joern

- `research/data-flow/repos/joern/semanticcpg/src/main/scala/io/shiftleft/semanticcpg/layers/LayerCreator.scala`
- `research/data-flow/repos/joern/semanticcpg/src/main/scala/io/shiftleft/semanticcpg/Overlays.scala`
