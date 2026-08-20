//! OS peak-RSS capture and cold/warm wall-clock timing.
//!
//! Peak RSS is read from the process high-water mark the host OS exposes:
//! `getrusage(RUSAGE_SELF).ru_maxrss` on Unix (normalized to bytes) and
//! `PeakWorkingSetSize` (already bytes) via `K32GetProcessMemoryInfo` on
//! Windows. Both are monotonic non-decreasing across a process lifetime.
//!
//! Lives at crate root (not under `eval`) so the rules-host golden-cost path
//! can measure without compiling the eval harness.

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
#[cfg(unix)]
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

/// Current process peak resident-set size in bytes, read from the Windows
/// `PROCESS_MEMORY_COUNTERS.PeakWorkingSetSize` (already in bytes).
///
/// Uses `K32GetProcessMemoryInfo` from `kernel32` (available since Windows 7)
/// so no `psapi.dll` link or extra crate is required. Like the Unix reading,
/// the peak working set is a monotonic high-water mark for the process.
#[cfg(windows)]
#[allow(
    unsafe_code,
    reason = "single audited GetProcessMemoryInfo FFI; crate denies unsafe_code otherwise"
)]
pub(crate) fn peak_rss_bytes() -> u64 {
    // Mirror of the Win32 `PROCESS_MEMORY_COUNTERS` layout: two `DWORD` (u32)
    // fields followed by eight `SIZE_T` (usize) fields. Only `cb` and
    // `peak_working_set_size` are read; the rest exist to keep the struct size
    // (and thus `cb`) correct so the OS fills `PeakWorkingSetSize`.
    #[repr(C)]
    #[allow(
        dead_code,
        reason = "FFI layout mirror; only cb and peak_working_set_size are read"
    )]
    struct ProcessMemoryCounters {
        cb: u32,
        page_fault_count: u32,
        peak_working_set_size: usize,
        working_set_size: usize,
        quota_peak_paged_pool_usage: usize,
        quota_paged_pool_usage: usize,
        quota_peak_non_paged_pool_usage: usize,
        quota_non_paged_pool_usage: usize,
        pagefile_usage: usize,
        peak_pagefile_usage: usize,
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetCurrentProcess() -> *mut std::ffi::c_void;
        fn K32GetProcessMemoryInfo(
            process: *mut std::ffi::c_void,
            counters: *mut ProcessMemoryCounters,
            cb: u32,
        ) -> i32;
    }

    // SAFETY: `counters` is a fully-owned, zero-initialized POD whose `cb` is
    // set to its own size; `GetCurrentProcess` returns a valid pseudo-handle;
    // the call only writes into `counters` and returns nonzero on success.
    unsafe {
        let mut counters: ProcessMemoryCounters = std::mem::zeroed();
        counters.cb = std::mem::size_of::<ProcessMemoryCounters>() as u32;
        if K32GetProcessMemoryInfo(GetCurrentProcess(), &mut counters, counters.cb) == 0 {
            return 0;
        }
        // `PeakWorkingSetSize` is already in bytes.
        counters.peak_working_set_size as u64
    }
}

/// Fallback for any target that is neither Unix nor Windows: peak RSS is not
/// available, so the harness reports 0 (the metric is treated as unmeasured).
#[cfg(not(any(unix, windows)))]
pub(crate) fn peak_rss_bytes() -> u64 {
    0
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
#[cfg(test)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ColdWarm {
    pub(crate) cold_ms: u64,
    pub(crate) warm_ms: u64,
    /// Absolute peak RSS (bytes): the process-wide, monotonic high-water mark
    /// after the warm run. Because it is whole-process (not attributable to this
    /// run), it is kept for reporting only and MUST NOT be the gated metric.
    pub(crate) peak_rss_bytes: u64,
    /// Run-attributable peak-RSS growth (bytes): the larger of the two per-run
    /// deltas above the high-water mark that existed before each run. This is
    /// the confound-free metric the regression gate compares — the absolute peak
    /// also reflects allocations made by whatever process hosts the measurement.
    pub(crate) peak_rss_delta_bytes: u64,
}

/// Run `run` twice and report cold (first) and warm (second) wall-clock millis,
/// the absolute overall peak RSS (reporting), and the run-attributable peak-RSS
/// delta (the gated metric).
#[cfg(test)]
pub(crate) fn cold_then_warm<F: FnMut()>(mut run: F) -> ColdWarm {
    let cold = TimedRun::measure(&mut run);
    let warm = TimedRun::measure(&mut run);
    ColdWarm {
        cold_ms: cold.elapsed_ms,
        warm_ms: warm.elapsed_ms,
        // Absolute mark is monotonic; the warm reading is the overall peak.
        // Reporting only — not tautological `max()` since the delta below is the
        // run-attributable value the gate actually compares.
        peak_rss_bytes: warm.peak_rss_bytes,
        peak_rss_delta_bytes: warm.peak_rss_delta_bytes.max(cold.peak_rss_delta_bytes),
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
