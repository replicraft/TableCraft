//! Explicit captured Agile removal; native metadata and publication stay serial.
use super::{
    AgilePageDecoder, EncryptionCopy, EncryptionInspection, EncryptionRewrite,
    OfficeEncryptionFamily, PageDecoder, inspect_encryption, rewrite,
};
use crate::{InspectError, format::page::PAGE_BYTES, header::check};
use rayon::{ThreadPool, prelude::*};
use std::io::{Read, Seek};
use tablecraft_core::limits::{Resource, ResourceBudget};

const MIN_THREADS: usize = 2;
const MAX_THREADS: usize = 16;
const PAGE_CHECK_STEPS: u64 = 2;
const PAGE_STEPS: u64 = (PAGE_BYTES / super::agile::AES_BLOCK_BYTES) as u64 + PAGE_CHECK_STEPS;

type PageResult = Result<(), InspectError>;

/// Remove Agile AES encryption using an explicitly supplied native worker pool.
///
/// Only `EncryptionRewrite::Remove` on a qualified Agile source is admitted.
/// Other operations and cipher families are refused without a serial retry.
/// The pool must have 2..=16 threads and is never constructed by the engine.
/// Password verification, header changes and native SID remasking remain serial;
/// only disjoint owned data pages are decoded concurrently. The complete image
/// remains bounded to 64 MiB and is returned only after every page and native
/// rewrite check succeeds. Callers retain the existing stable/offline-source and
/// fresh-output obligations. This does not authenticate the file's integrity HMAC.
///
/// Worker budgets are prepaid from the caller's cumulative budget before page
/// callbacks. Failures are examined in original page order; any partial decoded
/// candidate is zeroized and discarded. Cancellation must be safe to query from
/// multiple workers; it never authorizes partial output publication.
/// Existing sequential and streaming APIs retain their original behavior.
pub fn rewrite_encryption_parallel<R: Read + Seek>(
    source: &mut R,
    operation: EncryptionRewrite<'_>,
    pool: &ThreadPool,
    budget: &mut ResourceBudget,
    cancelled: &(dyn Fn() -> bool + Sync),
) -> Result<EncryptionCopy, InspectError> {
    let _span = tracing::debug_span!("native.encryption.remove.parallel").entered();
    check(budget, cancelled)?;
    if !(MIN_THREADS..=MAX_THREADS).contains(&pool.current_num_threads()) {
        return Err(InspectError::StructureLimit);
    }
    if !matches!(&operation, EncryptionRewrite::Remove { .. }) {
        return Err(InspectError::UnsupportedEncryption);
    }
    match inspect_encryption(source, budget, cancelled)? {
        EncryptionInspection::Office(info) if info.family() == OfficeEncryptionFamily::Agile => {}
        _ => return Err(InspectError::UnsupportedEncryption),
    }

    rewrite::rewrite_with_pages(
        source,
        operation,
        budget,
        cancelled,
        |decoder, bytes, budget, _| match decoder {
            PageDecoder::Agile(decoder) => decode_pages(decoder, bytes, pool, budget, cancelled),
            PageDecoder::Legacy(_) => Err(InspectError::UnsupportedEncryption),
        },
    )
}

fn decode_pages(
    decoder: &AgilePageDecoder,
    bytes: &mut [u8],
    pool: &ThreadPool,
    budget: &mut ResourceBudget,
    cancelled: &(dyn Fn() -> bool + Sync),
) -> Result<(), InspectError> {
    check(budget, cancelled)?;
    if !bytes.len().is_multiple_of(PAGE_BYTES) || bytes.len() < PAGE_BYTES {
        return Err(InspectError::MalformedEncryption);
    }
    let pages = bytes.len() / PAGE_BYTES - 1;
    budget.charge_allocation(pages as u64, size_of::<PageResult>() as u64)?;
    budget.charge_allocation(pages as u64, PAGE_BYTES as u64)?;
    budget.charge(Resource::Steps, pages as u64 * PAGE_STEPS)?;

    let mut outcomes = Vec::new();
    outcomes
        .try_reserve_exact(pages)
        .map_err(|_| InspectError::Allocation)?;
    outcomes.resize_with(pages, || Ok(()));
    pool.install(|| {
        bytes[PAGE_BYTES..]
            .par_chunks_exact_mut(PAGE_BYTES)
            .zip(outcomes.par_iter_mut())
            .enumerate()
            .for_each(|(index, (page, outcome))| {
                let mut local = ResourceBudget::new(PAGE_BYTES as u64, 0, PAGE_STEPS);
                *outcome = decoder.decode_page(index as u32 + 1, page, &mut local, cancelled);
            });
    });
    // Statuses contain payload-free errors, never page contents or key material.
    // Do not short circuit worker scheduling or publish a successful prefix.
    for outcome in outcomes {
        outcome?;
    }
    check(budget, cancelled)
}

#[cfg(test)]
mod tests;
