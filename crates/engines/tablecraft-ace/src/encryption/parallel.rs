//! Caller-controlled bounded parallelism for the shared native streaming rewrite.
mod buffer;
#[cfg(test)]
mod tests;
mod transfer;

use super::{
    EncryptionRewrite,
    budget::{COORDINATOR_STEPS_PER_PAGE, PAGE_TRANSFORM_BYTES, PAGE_TRANSFORM_STEPS},
    streaming,
};
use crate::{InspectError, format::page::PAGE_BYTES};
use rayon::ThreadPool;
use std::io::{Read, Seek, Write};

// The codec owns a zeroizing page plus bounded key, digest and cipher state on
// each worker stack. Reserve another full page for that auxiliary state. Worker
// thread stacks themselves belong to the caller-owned pool, outside this lease.
const WORKER_SCRATCH_BYTES: usize = 2 * PAGE_BYTES;
const FIXED_BUFFER_BYTES: usize = 2 * PAGE_BYTES;

type PageResult = Result<(), InspectError>;

/// Caller-owned worker pool and adaptive live-buffer policy for native rewrites.
///
/// The memory callback runs only on the coordinator before each batch. Its input
/// is the actual retained data/status capacity plus fixed header buffers; workers
/// have joined and no codec scratch is live at that point. It returns the total
/// permitted working-set bytes for the next batch, including retained buffers
/// and the conservative per-active-worker codec scratch reservation. It is not
/// an additional allocation allowance. A host observing free memory should
/// account for the supplied retained capacity before choosing a total ceiling.
/// Cumulative operation budgets remain separate and are never replenished here.
/// Source/target I/O and this callback do not need Send or Sync.
pub struct ParallelEncryption<'a> {
    pool: &'a ThreadPool,
    memory: &'a mut dyn FnMut(usize) -> Result<usize, InspectError>,
}

impl<'a> ParallelEncryption<'a> {
    /// Borrow an existing pool and coordinator memory policy; no workers spawn here.
    /// A one-thread pool is valid and uses the same bounded transfer pipeline.
    pub fn new(
        pool: &'a ThreadPool,
        memory: &'a mut dyn FnMut(usize) -> Result<usize, InspectError>,
    ) -> Self {
        Self { pool, memory }
    }

    /// Minimum working set for a page-codec batch: one page, one status, header
    /// buffers and one conservative codec scratch reservation. Pool stacks are
    /// separate. No-codec copies need no worker scratch and can use less.
    pub const fn minimum_batch_bytes() -> usize {
        FIXED_BUFFER_BYTES + PAGE_BYTES + size_of::<PageResult>() + WORKER_SCRATCH_BYTES
    }
}

/// Stream any already-supported native encryption operation with bounded batches.
///
/// This shares sequential credential checks, header construction, SID selection
/// and final source verification. All cipher families supported by
/// `rewrite_encryption_to` use their existing source-specific immutable codecs;
/// no unsupported-mode or failed-worker retry occurs. Independent page decoding
/// and encoding run on disjoint zeroizing owned buffers, with original page
/// ordinals and prepaid local budgets. SID patches, reads and writes stay on the
/// coordinator in original order. Every worker phase joins before its buffer is
/// inspected or reused, and errors are selected in original page order.
///
/// No complete image is captured: the native read-file ceiling (currently 2 GiB)
/// applies, independently of the image API's smaller retention ceiling. The
/// caller's memory policy chooses each batch and can shrink or reuse buffers.
/// Agile envelope-only password changes use the same bounded I/O batches with
/// no worker transforms or scratch reservation. The pool is never created or
/// resized by the engine.
///
/// The destination must be empty and unpublished. A failure can leave a prefix
/// there, which the host must discard. This function never publishes, flushes or
/// syncs the output. Cancellation must support concurrent worker polling; no
/// active database content or integrity-HMAC authentication is added.
pub fn rewrite_encryption_parallel_to<R: Read + Seek, W: Write + Seek>(
    source: &mut R,
    target: &mut W,
    operation: EncryptionRewrite<'_>,
    parallel: ParallelEncryption<'_>,
    budget: &mut tablecraft_core::limits::ResourceBudget,
    cancelled: &(dyn Fn() -> bool + Sync),
) -> Result<(), InspectError> {
    let _span = tracing::debug_span!("native.encryption.copy_pages.parallel").entered();
    streaming::rewrite_with_transfer(source, target, operation, budget, cancelled, |pages| {
        transfer::run(pages, parallel, cancelled)
    })
}
