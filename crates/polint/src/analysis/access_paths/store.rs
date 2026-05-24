use std::collections::BTreeMap;

use super::facts::{AccessPathFact, AccessPathStatus};
use crate::analysis::ids::{AccessPathId, PlaceId};
use crate::core::{FunctionId, Language};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct AccessPathOutput {
    pub(crate) access_paths: Vec<AccessPathFact>,
}

impl AccessPathOutput {
    pub(crate) fn normalized(mut self) -> Self {
        self.access_paths.sort_by(|left, right| {
            (left.stable_key.as_str(), left.id).cmp(&(right.stable_key.as_str(), right.id))
        });
        for (index, row) in self.access_paths.iter_mut().enumerate() {
            row.id = AccessPathId(index as u64);
        }
        self
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct AccessPathStore {
    output: AccessPathOutput,
    by_language: BTreeMap<Language, Vec<usize>>,
    by_function: BTreeMap<FunctionId, Vec<usize>>,
    by_base: BTreeMap<PlaceId, Vec<usize>>,
    by_status: BTreeMap<AccessPathStatus, Vec<usize>>,
}

impl AccessPathStore {
    pub(crate) fn from_output(output: AccessPathOutput) -> Self {
        let output = output.normalized();
        let mut store = Self {
            output,
            ..Self::default()
        };
        for (index, row) in store.output.access_paths.iter().enumerate() {
            store
                .by_language
                .entry(row.language)
                .or_default()
                .push(index);
            if let Some(function) = row.function {
                store.by_function.entry(function).or_default().push(index);
            }
            store.by_base.entry(row.base).or_default().push(index);
            store.by_status.entry(row.status).or_default().push(index);
        }
        store
    }

    pub(crate) fn access_paths(&self) -> &[AccessPathFact] {
        &self.output.access_paths
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::access_paths::facts::AccessPathProjection;

    fn access_path(id: u64, stable_key: &str) -> AccessPathFact {
        AccessPathFact {
            id: AccessPathId(id),
            base: PlaceId(1),
            projections: vec![AccessPathProjection::Property(stable_key.to_string())],
            depth: 1,
            language: Language::JavaScript,
            file: None,
            function: None,
            body: None,
            status: AccessPathStatus::Resolved,
            stable_key: stable_key.to_string(),
        }
    }

    #[test]
    fn access_path_output_sorts_by_stable_key_and_reassigns_ids() {
        let output = AccessPathOutput {
            access_paths: vec![access_path(8, "path:z"), access_path(2, "path:a")],
        }
        .normalized();

        assert_eq!(output.access_paths[0].stable_key, "path:a");
        assert_eq!(output.access_paths[0].id, AccessPathId(0));
        assert_eq!(output.access_paths[1].id, AccessPathId(1));
    }
}
