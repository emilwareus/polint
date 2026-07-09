//! OS peak-RSS capture and cold/warm wall-clock timing (BENCH-01).
//!
//! Populates the real measurement that `RuntimeStatsSummary.peak_rss_bytes`
//! (see `crate::eval::performance`) has always declared but never set in
//! production. Peak RSS is read from `getrusage(RUSAGE_SELF).ru_maxrss` and
//! normalized to bytes per-OS: only Darwin (macOS/iOS) reports `ru_maxrss` in
//! bytes; Linux and every BSD (FreeBSD/OpenBSD/NetBSD/DragonFly) report it in
//! kilobytes.

use std::time::Instant;

/// Current process peak resident-set size in bytes, read from
/// `getrusage(RUSAGE_SELF).ru_maxrss`.
///
/// `ru_maxrss` is the maximum RSS the process has reached (a high-water mark),
/// so this is monotonic non-decreasing across a process lifetime. Units differ
/// per OS and are normalized here:
/// - Darwin (macOS/iOS): `ru_maxrss` is already in bytes.
/// - Linux and the BSDs (FreeBSD/OpenBSD/NetBSD/DragonFly): `ru_maxrss` is in
///   kilobytes -> multiply by 1024.
#[allow(
    unsafe_code,
    reason = "single audited getrusage FFI; crate denies unsafe_code otherwise"
)]
pub(crate) fn peak_rss_bytes() -> u64 {
    // SAFETY: `getrusage` writes into a fully-owned, zero-initialized `rusage`
    // and only reads `RUSAGE_SELF`. No aliasing or lifetime concerns.
    let ru_maxrss = unsafe {
        let mut usage: libc::rusage = std::mem::zeroed();
        if libc::getrusage(libc::RUSAGE_SELF, &mut usage) != 0 {
            return 0;
        }
        usage.ru_maxrss
    };

    let raw = ru_maxrss.max(0) as u64;
    // Only Darwin (macOS/iOS) reports `ru_maxrss` in bytes. Linux and every BSD
    // (FreeBSD/OpenBSD/NetBSD/DragonFly) report it in kilobytes.
    if cfg!(any(target_os = "macos", target_os = "ios")) {
        raw
    } else {
        raw.saturating_mul(1024)
    }
}

/// Result of executing a closure once: wall-clock elapsed and the peak-RSS
/// high-water mark observed after the run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct TimedRun {
    pub(crate) elapsed_ms: u64,
    /// Peak RSS (bytes) observed immediately after the run completed.
    pub(crate) peak_rss_bytes: u64,
    /// Peak-RSS delta (bytes) versus the high-water mark captured before the run
    /// started. Because `ru_maxrss` is a monotonic high-water mark, this is the
    /// additional peak the run itself is responsible for (0 if it never exceeded
    /// the pre-existing high-water mark).
    pub(crate) peak_rss_delta_bytes: u64,
}

impl TimedRun {
    /// Execute `run` once, capturing wall-clock elapsed and the peak-RSS delta.
    pub(crate) fn measure<F: FnMut()>(mut run: F) -> Self {
        let baseline_peak = peak_rss_bytes();
        let start = Instant::now();
        run();
        let elapsed_ms = start.elapsed().as_millis().min(u128::from(u64::MAX)) as u64;
        let peak = peak_rss_bytes();
        Self {
            elapsed_ms,
            peak_rss_bytes: peak,
            peak_rss_delta_bytes: peak.saturating_sub(baseline_peak),
        }
    }
}

/// Cold-then-warm measurement of a repeatable closure. The closure is executed
/// twice: the first run is the cold measurement (nothing warmed yet) and the
/// second is the warm measurement (caches/allocations primed by the cold run).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ColdWarm {
    pub(crate) cold_ms: u64,
    pub(crate) warm_ms: u64,
    /// Peak RSS (bytes) observed across both runs — the high-water mark is
    /// monotonic, so the warm run's reading is the overall peak.
    pub(crate) peak_rss_bytes: u64,
}

/// Run `run` twice and report cold (first) and warm (second) wall-clock millis
/// plus the overall peak RSS.
pub(crate) fn cold_then_warm<F: FnMut()>(mut run: F) -> ColdWarm {
    let cold = TimedRun::measure(&mut run);
    let warm = TimedRun::measure(&mut run);
    ColdWarm {
        cold_ms: cold.elapsed_ms,
        warm_ms: warm.elapsed_ms,
        // The high-water mark is monotonic; the warm reading is the overall peak.
        peak_rss_bytes: warm.peak_rss_bytes.max(cold.peak_rss_bytes),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn peak_rss_bytes_is_nonzero_on_host() {
        // A running process always has a non-zero peak RSS.
        assert!(
            peak_rss_bytes() > 0,
            "peak RSS should be measurable and > 0"
        );
    }

    #[test]
    fn timed_run_captures_elapsed_and_peak() {
        let run = TimedRun::measure(|| {
            // Touch some memory so the run does observable work.
            let v: Vec<u64> = (0..10_000).collect();
            std::hint::black_box(&v);
        });
        assert!(run.peak_rss_bytes > 0);
    }

    #[test]
    fn cold_then_warm_runs_twice_and_reports_peak() {
        let mut calls = 0u32;
        let result = cold_then_warm(|| {
            calls += 1;
            let v: Vec<u64> = (0..1_000).collect();
            std::hint::black_box(&v);
        });
        assert_eq!(calls, 2, "closure should run exactly twice (cold + warm)");
        assert!(result.peak_rss_bytes > 0);
    }
}
