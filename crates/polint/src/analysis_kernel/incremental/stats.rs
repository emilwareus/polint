use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct CacheStats {
    pub(crate) hits: u64,
    pub(crate) misses: u64,
    pub(crate) recomputes: u64,
    pub(crate) writes: u64,
    pub(crate) bypasses_disabled: u64,
    pub(crate) invalid_evicted_reads: u64,
    pub(crate) verified_reuse: u64,
    pub(crate) quarantines: u64,
}

impl CacheStats {
    pub(crate) fn record_hit(&mut self) {
        self.hits += 1;
    }

    pub(crate) fn record_miss(&mut self) {
        self.misses += 1;
    }

    pub(crate) fn record_recompute(&mut self) {
        self.recomputes += 1;
    }

    pub(crate) fn record_write(&mut self) {
        self.writes += 1;
    }

    pub(crate) fn record_disabled_bypass(&mut self) {
        self.bypasses_disabled += 1;
    }

    pub(crate) fn record_invalid_evicted_read(&mut self) {
        self.invalid_evicted_reads += 1;
    }

    pub(crate) fn record_verified_reuse(&mut self) {
        self.verified_reuse += 1;
    }

    pub(crate) fn record_quarantine(&mut self) {
        self.quarantines += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_cache_stats_serialize_zero_counters() {
        let stats = CacheStats::default();
        let json = serde_json::to_value(stats).expect("cache stats should serialize");

        assert_eq!(json["hits"], 0);
        assert_eq!(json["misses"], 0);
        assert_eq!(json["recomputes"], 0);
        assert_eq!(json["writes"], 0);
        assert_eq!(json["bypasses_disabled"], 0);
        assert_eq!(json["invalid_evicted_reads"], 0);
        assert_eq!(json["verified_reuse"], 0);
        assert_eq!(json["quarantines"], 0);
    }

    #[test]
    fn record_methods_increment_each_counter() {
        let mut stats = CacheStats::default();

        stats.record_hit();
        stats.record_miss();
        stats.record_recompute();
        stats.record_write();
        stats.record_disabled_bypass();
        stats.record_invalid_evicted_read();
        stats.record_verified_reuse();
        stats.record_quarantine();

        assert_eq!(
            stats,
            CacheStats {
                hits: 1,
                misses: 1,
                recomputes: 1,
                writes: 1,
                bypasses_disabled: 1,
                invalid_evicted_reads: 1,
                verified_reuse: 1,
                quarantines: 1,
            }
        );
    }
}
