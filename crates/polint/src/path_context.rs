//! Bounded-context path pairing for monorepos (configured in `.polint.toml` under `[path_contexts]`).

use crate::config::PathContextsConfig;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Default)]
pub(crate) struct PathContextIndex {
    /// Pair name → context key → left and right relative path buckets.
    inner: BTreeMap<String, BTreeMap<String, ContextBuckets>>,
}

#[derive(Debug, Clone, Default)]
struct ContextBuckets {
    left: Vec<String>,
    right: Vec<String>,
}

impl PathContextIndex {
    pub(crate) fn build(config: &PathContextsConfig, relative_paths: &[String]) -> Self {
        let mut inner: BTreeMap<String, BTreeMap<String, ContextBuckets>> = BTreeMap::new();
        for pair in &config.pairs {
            let slot = inner.entry(pair.name.clone()).or_default();
            for path in relative_paths {
                if let Some(ctx) = extract_ctx(path, &pair.left_before_ctx, &pair.left_after_ctx) {
                    slot.entry(ctx.clone()).or_default().left.push(path.clone());
                } else if let Some(ctx) =
                    extract_ctx(path, &pair.right_before_ctx, &pair.right_after_ctx)
                {
                    slot.entry(ctx.clone())
                        .or_default()
                        .right
                        .push(path.clone());
                }
            }
        }
        Self { inner }
    }

    /// Other repository-relative paths paired with `rel_path` under the named rule (opposite bucket, same context key).
    pub(crate) fn related_paths(&self, pair_name: &str, rel_path: &str) -> Vec<String> {
        let Some(by_ctx) = self.inner.get(pair_name) else {
            return Vec::new();
        };
        for buckets in by_ctx.values() {
            if buckets.left.iter().any(|p| p == rel_path) {
                return buckets
                    .right
                    .iter()
                    .filter(|p| *p != rel_path)
                    .cloned()
                    .collect();
            }
            if buckets.right.iter().any(|p| p == rel_path) {
                return buckets
                    .left
                    .iter()
                    .filter(|p| *p != rel_path)
                    .cloned()
                    .collect();
            }
        }
        Vec::new()
    }
}

fn extract_ctx(path: &str, before: &str, after: &str) -> Option<String> {
    let rest = path.strip_prefix(before)?;
    let idx = rest.find(after)?;
    let seg = &rest[..idx];
    if seg.is_empty() || seg.contains('/') {
        return None;
    }
    Some(seg.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::PathContextPair;

    #[test]
    fn pairs_left_and_right() {
        let cfg = PathContextsConfig {
            pairs: vec![PathContextPair {
                name: "svc_ports".to_string(),
                left_before_ctx: "internal/".to_string(),
                left_after_ctx: "/service/".to_string(),
                right_before_ctx: "internal/".to_string(),
                right_after_ctx: "/ports/".to_string(),
            }],
        };
        let paths = vec![
            "internal/acme/service/foo.go".to_string(),
            "internal/acme/ports/bar.go".to_string(),
        ];
        let idx = PathContextIndex::build(&cfg, &paths);
        let related = idx.related_paths("svc_ports", "internal/acme/service/foo.go");
        assert_eq!(related, vec!["internal/acme/ports/bar.go".to_string()]);
    }
}
