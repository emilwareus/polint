# Entry 9: Java Adapter

## Goal

Add Java through the same adapter contract, starting with setup-aware syntax and
project facts and expanding toward capability parity.

## Why

Java repo-local policies often depend on packages, classpaths, test frameworks,
and coverage. polint should support that without exposing raw Java tooling to
rule authors.

## Difficulty

**L** for syntax facts, **XL** for resolved imports/symbols/calls, **M** for
coverage import.

## Initial Capability Tier

- packages and imports
- classes and methods
- literals
- basic branches
- JUnit/TestNG test facts
- JaCoCo coverage import

## Build Method

1. Add Java language detection and config.
2. Start with syntax, packages, classes, methods, imports, literals, and basic
   branches.
3. Choose between a self-contained parser path and a javac/JavaParser semantic
   path per capability.
4. For semantic facts, require setup that provides a Maven/Gradle classpath or
   compile command.
5. Use `JavaCompiler`, `JavacTask`, and `Trees` when leaning on JDK-native
   analysis.
6. Use JavaParser plus its symbol solver if embedded JVM-side tooling is more
   practical.
7. Parse JaCoCo XML for coverage.
8. Detect JUnit/TestNG tests and assertions for test metrics.

## Done When

- Java rules can be written with `polint::sdk::prelude::*`.
- Missing classpath/build setup produces diagnostics.
- JaCoCo reports can become coverage facts.
- External generated-rule tests cover the supported facts.

## Full Coverage Path

Move Java toward parity after Go and TS/JS prove the complete model.
