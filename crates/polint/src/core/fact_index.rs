use std::cmp::Ordering;
use std::ops::Range;

use super::FileId;

#[derive(Debug, Clone, Default)]
pub(super) struct DenseFileIndex {
    by_file: Box<[FilePositions]>,
}

impl DenseFileIndex {
    pub(super) fn build<T>(
        file_count: usize,
        facts: &[T],
        file_for: impl Fn(&T) -> Option<FileId>,
    ) -> Self {
        if facts.is_empty() {
            return Self::default();
        }

        let mut by_file = vec![FilePositions::default(); file_count].into_boxed_slice();
        for (fact_index, fact) in facts.iter().enumerate() {
            let Some(file_index) = file_for(fact).map(|file| file.raw() as usize) else {
                continue;
            };
            let Some(positions) = by_file.get_mut(file_index) else {
                continue;
            };
            positions.push(fact_index);
        }
        Self { by_file }
    }

    pub(super) fn build_sorted_by<T>(
        file_count: usize,
        facts: &[T],
        file_for: impl Fn(&T) -> Option<FileId>,
        compare: impl Fn(&T, &T) -> Ordering,
    ) -> Self {
        let mut index = Self::build(file_count, facts, file_for);
        for positions in &mut index.by_file {
            let mut fact_indexes = positions.iter().collect::<Vec<_>>();
            fact_indexes.sort_by(|left, right| compare(&facts[*left], &facts[*right]));
            *positions = FilePositions::from_indexes(fact_indexes);
        }
        index
    }

    pub(super) fn facts<'a, T>(&'a self, file: FileId, facts: &'a [T]) -> FileFactIter<'a, T> {
        let Some(positions) = self.by_file.get(file.raw() as usize) else {
            return FileFactIter::Empty;
        };
        match positions {
            FilePositions::Empty => FileFactIter::Empty,
            FilePositions::Contiguous(range) => facts
                .get(range.clone())
                .map(|facts| FileFactIter::Contiguous(facts.iter()))
                .unwrap_or(FileFactIter::Empty),
            FilePositions::Sparse(indexes) => FileFactIter::Sparse {
                facts,
                indexes: indexes.iter(),
            },
        }
    }
}

#[derive(Debug, Clone, Default)]
enum FilePositions {
    #[default]
    Empty,
    Contiguous(Range<usize>),
    Sparse(Vec<usize>),
}

impl FilePositions {
    fn from_indexes(indexes: Vec<usize>) -> Self {
        let Some(first) = indexes.first().copied() else {
            return Self::Empty;
        };
        if indexes
            .windows(2)
            .all(|window| window[0].checked_add(1) == Some(window[1]))
        {
            return Self::Contiguous(first..indexes[indexes.len() - 1] + 1);
        }
        Self::Sparse(indexes)
    }

    fn push(&mut self, index: usize) {
        *self = match std::mem::take(self) {
            Self::Empty => Self::Contiguous(index..index + 1),
            Self::Contiguous(mut range) if range.end == index => {
                range.end += 1;
                Self::Contiguous(range)
            }
            Self::Contiguous(range) => {
                let mut indexes = Vec::with_capacity(range.len() + 1);
                indexes.extend(range);
                indexes.push(index);
                Self::Sparse(indexes)
            }
            Self::Sparse(mut indexes) => {
                indexes.push(index);
                Self::Sparse(indexes)
            }
        };
    }

    fn iter(&self) -> FilePositionIter<'_> {
        match self {
            Self::Empty => FilePositionIter::Empty,
            Self::Contiguous(range) => FilePositionIter::Contiguous(range.clone()),
            Self::Sparse(indexes) => FilePositionIter::Sparse(indexes.iter()),
        }
    }
}

enum FilePositionIter<'a> {
    Empty,
    Contiguous(Range<usize>),
    Sparse(std::slice::Iter<'a, usize>),
}

impl Iterator for FilePositionIter<'_> {
    type Item = usize;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Empty => None,
            Self::Contiguous(range) => range.next(),
            Self::Sparse(indexes) => indexes.next().copied(),
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        match self {
            Self::Empty => (0, Some(0)),
            Self::Contiguous(range) => range.size_hint(),
            Self::Sparse(indexes) => indexes.size_hint(),
        }
    }
}

pub(super) enum FileFactIter<'a, T> {
    Empty,
    Contiguous(std::slice::Iter<'a, T>),
    Sparse {
        facts: &'a [T],
        indexes: std::slice::Iter<'a, usize>,
    },
}

impl<'a, T> Iterator for FileFactIter<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Empty => None,
            Self::Contiguous(facts) => facts.next(),
            Self::Sparse { facts, indexes } => indexes.by_ref().find_map(|index| facts.get(*index)),
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        match self {
            Self::Empty => (0, Some(0)),
            Self::Contiguous(facts) => facts.size_hint(),
            Self::Sparse { indexes, .. } => (0, indexes.size_hint().1),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub(super) struct DenseIdIndex {
    by_id: Box<[usize]>,
}

impl DenseIdIndex {
    pub(super) fn build<T>(
        id_count: usize,
        facts: &[T],
        raw_id_for: impl Fn(&T) -> Option<u64>,
    ) -> Self {
        if facts.is_empty() || id_count == 0 {
            return Self::default();
        }

        let mut by_id = vec![usize::MAX; id_count].into_boxed_slice();
        for (fact_index, fact) in facts.iter().enumerate() {
            let Some(raw_id) = raw_id_for(fact).and_then(|id| usize::try_from(id).ok()) else {
                continue;
            };
            let Some(index) = by_id.get_mut(raw_id) else {
                continue;
            };
            if *index == usize::MAX {
                *index = fact_index;
            }
        }
        Self { by_id }
    }

    pub(super) fn get<'a, T>(&self, raw_id: u64, facts: &'a [T]) -> Option<&'a T> {
        let raw_id = usize::try_from(raw_id).ok()?;
        let index = self
            .by_id
            .get(raw_id)
            .copied()
            .filter(|index| *index != usize::MAX)?;
        facts.get(index)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dense_file_index_preserves_global_order_for_contiguous_and_sparse_files() {
        let file_zero = FileId::from_raw(0);
        let file_one = FileId::from_raw(1);
        let facts = [
            (file_zero, "a0"),
            (file_zero, "a1"),
            (file_one, "b0"),
            (file_zero, "a2"),
        ];
        let index = DenseFileIndex::build(2, &facts, |fact| Some(fact.0));

        assert_eq!(
            index
                .facts(file_zero, &facts)
                .map(|fact| fact.1)
                .collect::<Vec<_>>(),
            ["a0", "a1", "a2"]
        );
        assert_eq!(
            index
                .facts(file_one, &facts)
                .map(|fact| fact.1)
                .collect::<Vec<_>>(),
            ["b0"]
        );
        assert_eq!(index.facts(FileId::from_raw(9), &facts).count(), 0);
    }

    #[test]
    fn dense_file_index_ignores_fact_ids_outside_the_file_domain() {
        let invalid_file = FileId::from_raw(u32::MAX);
        let facts = [(FileId::from_raw(0), "valid"), (invalid_file, "invalid")];
        let index = DenseFileIndex::build(1, &facts, |fact| Some(fact.0));

        assert_eq!(index.by_file.len(), 1);
        assert_eq!(
            index
                .facts(FileId::from_raw(0), &facts)
                .map(|fact| fact.1)
                .collect::<Vec<_>>(),
            ["valid"]
        );
        assert_eq!(index.facts(invalid_file, &facts).count(), 0);
    }

    #[test]
    fn dense_file_index_can_preserve_a_family_specific_stable_order() {
        let file = FileId::from_raw(0);
        let facts = [
            (file, 30, "thirty"),
            (file, 10, "first ten"),
            (file, 20, "twenty"),
            (file, 10, "second ten"),
        ];
        let index = DenseFileIndex::build_sorted_by(
            1,
            &facts,
            |fact| Some(fact.0),
            |left, right| left.1.cmp(&right.1),
        );

        assert_eq!(
            index
                .facts(file, &facts)
                .map(|fact| fact.2)
                .collect::<Vec<_>>(),
            ["first ten", "second ten", "twenty", "thirty"]
        );
    }

    #[test]
    fn dense_id_index_supports_sparse_ids_and_keeps_first_match() {
        let facts = [
            (4, "four"),
            (1, "one"),
            (1, "duplicate"),
            (u64::MAX, "invalid"),
        ];
        let index = DenseIdIndex::build(5, &facts, |fact| Some(fact.0));

        assert_eq!(index.by_id.len(), 5);
        assert_eq!(index.get(1, &facts), Some(&(1, "one")));
        assert_eq!(index.get(4, &facts), Some(&(4, "four")));
        assert_eq!(index.get(2, &facts), None);
        assert_eq!(index.get(u64::MAX, &facts), None);
    }
}
