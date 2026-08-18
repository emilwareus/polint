//! Architectural layering guard.
//!
//! The engine is layered: identity and IR at the bottom, neutral contracts above
//! them, neutral analysis above those, concrete language frontends above that, and
//! the facade composing everything. Those directions are currently respected, but a
//! single-crate layout gives the compiler no way to enforce them, so this test does.
//!
//! Each rule below states a module that must not name another module. Violations are
//! reported with file and line so the offending edge is actionable rather than a bare
//! count.

use std::fs;
use std::path::{Path, PathBuf};

/// A module that must not reference any of `forbidden`.
struct LayerRule {
    /// Directory under `src/`, relative.
    module: &'static str,
    /// `crate::<name>` path segments this module may not name.
    forbidden: &'static [&'static str],
    /// Why the edge is forbidden, quoted back on failure.
    reason: &'static str,
}

const RULES: &[LayerRule] = &[
    LayerRule {
        module: "internal_core",
        forbidden: &[
            "analysis",
            "analysis_api",
            "analysis_kernel",
            "analysis_neutral",
            "frontend",
            "frontend_api",
            "go",
            "ts",
            "cli",
        ],
        reason: "identity, spans, and diagnostics must not know about facts, analyses, or languages",
    },
    LayerRule {
        module: "ir",
        forbidden: &[
            "analysis",
            "analysis_kernel",
            "analysis_neutral",
            "frontend",
            "go",
            "ts",
            "cli",
        ],
        reason: "the IR must stay language-neutral and analysis-neutral",
    },
    LayerRule {
        module: "analysis_api",
        forbidden: &[
            "analysis_kernel",
            "analysis_neutral",
            "frontend",
            "go",
            "ts",
            "cli",
        ],
        reason: "provider and fact-store contracts must not name concrete analyses or frontends",
    },
    LayerRule {
        module: "frontend_api",
        forbidden: &["analysis_kernel", "go", "ts", "cli"],
        reason: "the frontend contract must not name a concrete frontend",
    },
    LayerRule {
        module: "analysis_neutral",
        forbidden: &["go", "ts", "frontend", "cli"],
        reason: "neutral analysis must not reach into a specific language; that is the whole point of the IR",
    },
    LayerRule {
        module: "go",
        forbidden: &["ts"],
        reason: "language frontends must not depend on each other",
    },
    LayerRule {
        module: "ts",
        forbidden: &["go"],
        reason: "language frontends must not depend on each other",
    },
];

fn src_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src")
}

fn rust_sources(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(entries) = fs::read_dir(dir) else {
        return out;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            out.extend(rust_sources(&path));
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
    }
    out.sort();
    out
}

/// Lines inside `#[cfg(test)]` modules are excluded: test code legitimately reaches
/// across layers to build fixtures, and constraining it would push tests out of the
/// module they cover.
fn non_test_lines(source: &str) -> Vec<(usize, &str)> {
    let mut out = Vec::new();
    let mut test_depth: Option<i32> = None;
    let mut pending_cfg_test = false;

    for (index, line) in source.lines().enumerate() {
        let trimmed = line.trim();

        if let Some(depth) = test_depth.as_mut() {
            *depth += line.matches('{').count() as i32;
            *depth -= line.matches('}').count() as i32;
            if *depth <= 0 {
                test_depth = None;
            }
            continue;
        }

        if trimmed.starts_with("#[cfg(test)]") || trimmed.starts_with("#[cfg(all(test") {
            pending_cfg_test = true;
            continue;
        }

        if pending_cfg_test {
            pending_cfg_test = false;
            if trimmed.starts_with("mod ") || trimmed.starts_with("pub mod ") {
                let opens = line.matches('{').count() as i32;
                let closes = line.matches('}').count() as i32;
                if opens > 0 && opens - closes > 0 {
                    test_depth = Some(opens - closes);
                } else if opens == 0 {
                    // `mod tests {` on the following line.
                    test_depth = Some(0);
                }
                continue;
            }
        }

        if let Some(depth) = test_depth.as_mut()
            && *depth == 0
        {
            *depth += line.matches('{').count() as i32;
            *depth -= line.matches('}').count() as i32;
            if *depth <= 0 && line.contains('{') {
                // Body opened and closed on one line.
                test_depth = None;
            }
            continue;
        }

        out.push((index + 1, line));
    }

    out
}

#[test]
fn module_layering_has_no_wrong_direction_edges() {
    let root = src_root();
    let mut violations: Vec<String> = Vec::new();

    for rule in RULES {
        let module_dir = root.join(rule.module);
        assert!(
            module_dir.is_dir(),
            "layering rule names `{}`, which is not a directory under src/. \
             Update this test when modules move — do not delete the rule.",
            rule.module
        );

        for file in rust_sources(&module_dir) {
            let Ok(source) = fs::read_to_string(&file) else {
                continue;
            };
            let relative = file
                .strip_prefix(&root)
                .unwrap_or(&file)
                .display()
                .to_string();

            for (line_number, line) in non_test_lines(&source) {
                for forbidden in rule.forbidden {
                    let needle = format!("crate::{forbidden}::");
                    if line.contains(&needle) {
                        violations.push(format!(
                            "  {relative}:{line_number}\n    {} -> {forbidden}\n    {}\n    {}",
                            rule.module,
                            rule.reason,
                            line.trim()
                        ));
                    }
                }
            }
        }
    }

    assert!(
        violations.is_empty(),
        "architectural layering violated by {} edge(s):\n\n{}\n\n\
         These directions are load-bearing: they are what make the analysis core \
         language-neutral and the IR reusable. If an edge is genuinely required, the \
         layering is wrong and needs a decision — do not delete the rule to go green.",
        violations.len(),
        violations.join("\n\n")
    );
}
