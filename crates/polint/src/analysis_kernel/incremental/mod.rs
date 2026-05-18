mod digest;
mod keys;
mod stats;

#[expect(
    unused_imports,
    reason = "Phase 23 establishes this crate-private vocabulary before later kernel consumers wire it in."
)]
pub(crate) use digest::{Digest, DigestKind};
#[expect(
    unused_imports,
    reason = "Phase 23 establishes this crate-private vocabulary before later kernel consumers wire it in."
)]
pub(crate) use stats::CacheStats;
