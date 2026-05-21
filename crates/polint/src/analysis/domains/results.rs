#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::cfg::ids::BasicBlockId;
    use crate::analysis::ids::{MirBodyId, MirOpId, PlaceId};

    #[test]
    fn result_cursor_exposes_entry_block_operation_and_exit_states() {
        let body = MirBodyId(1);
        let block = BasicBlockId(2);
        let operation = MirOpId(3);
        let place = PlaceId(4);
        let results = DomainResults::for_test(body, block, operation, place);

        assert!(results.entry_state(body).is_some());
        assert!(results.block_entry(block).is_some());
        assert!(results.before_operation(operation).is_some());
        assert!(results.after_operation(operation).is_some());
        assert!(results.block_exit(block).is_some());
        assert!(results.place_observations().any(|row| row.place == place));
    }

    #[test]
    fn stable_key_result_iteration_is_deterministic() {
        let results = DomainResults::for_test(
            MirBodyId(1),
            BasicBlockId(2),
            MirOpId(3),
            PlaceId(4),
        );

        assert_eq!(
            results.stable_digest_parts(),
            results.stable_digest_parts()
        );
    }
}
