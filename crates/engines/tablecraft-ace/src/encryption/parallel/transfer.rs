//! Coordinator I/O/SID ordering around joined disjoint page-codec phases.
use super::{
    COORDINATOR_STEPS_PER_PAGE, PAGE_BYTES, PAGE_TRANSFORM_BYTES, PAGE_TRANSFORM_STEPS, PageResult,
    ParallelEncryption,
    buffer::{Buffer, pages_for_memory},
};
use crate::{
    InspectError,
    encryption::{
        PageDecoder,
        streaming::{self, Encoder, Transfer},
    },
    header::check,
};
use rayon::{ThreadPool, prelude::*};
use std::io::{Read, Seek, Write};
use tablecraft_core::limits::{Resource, ResourceBudget};

#[derive(Clone, Copy)]
enum Transform<'a> {
    Decode(&'a PageDecoder),
    Encode(&'a Encoder),
}

impl Transform<'_> {
    fn apply(
        self,
        number: u32,
        page: &mut [u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), InspectError> {
        match self {
            Self::Decode(decoder) => decoder.decode_page(number, page, budget, cancelled),
            Self::Encode(encoder) => encoder.encode(number, page, budget, cancelled),
        }
    }
}

pub(super) fn run<R: Read + Seek, W: Write + Seek>(
    pages: Transfer<'_, R, W>,
    parallel: ParallelEncryption<'_>,
    cancelled: &(dyn Fn() -> bool + Sync),
) -> Result<(), InspectError> {
    if pages.decoder.is_none() && pages.encoder.is_none() {
        // Password-envelope changes and unencoded Jet password changes perform
        // no page crypto. Keep their copy path simple and avoid pool admission.
        return streaming::transfer_sequential(pages);
    }
    let Transfer {
        source,
        target,
        header,
        decoder,
        encoder,
        patches,
        budget,
        ..
    } = pages;
    let mut buffers = Buffer::new();
    let mut first = 1_u32;
    while first < header.page_count() {
        check(budget, cancelled)?;
        let ceiling = (parallel.memory)(buffers.retained_bytes()?)?;
        let count = pages_for_memory(
            ceiling,
            parallel.pool.current_num_threads(),
            (header.page_count() - first) as usize,
        )?;
        buffers.prepare(count, parallel.pool.current_num_threads(), ceiling, budget)?;
        let phases = u64::from(decoder.is_some()) + u64::from(encoder.is_some());
        budget.charge_allocation(count as u64 * phases, PAGE_TRANSFORM_BYTES)?;
        budget.charge(
            Resource::Steps,
            count as u64 * (phases * PAGE_TRANSFORM_STEPS + COORDINATOR_STEPS_PER_PAGE),
        )?;
        // Read/output byte work is cumulative even when the same allocation is
        // reused. Memory policy is a live-capacity limit, not a work refund.
        budget.charge(Resource::Bytes, 2 * buffers.data.len() as u64)?;
        let offset = u64::from(first) * PAGE_BYTES as u64;
        tablecraft_io::read_exact_at(source, offset, &mut buffers.data, budget, cancelled)?;

        if let Some(decoder) = decoder {
            apply(
                parallel.pool,
                first,
                &mut buffers.data,
                &mut buffers.outcomes,
                Transform::Decode(decoder),
                cancelled,
            )?;
        }
        let mut prefix = buffers
            .outcomes
            .iter()
            .position(Result::is_err)
            .unwrap_or(count);
        for index in 0..prefix {
            let start = index * PAGE_BYTES;
            let page = &mut buffers.data[start..start + PAGE_BYTES];
            if let Err(error) = patches.apply(offset as usize + start, page, budget, cancelled) {
                buffers.outcomes[index] = Err(error);
                prefix = index;
                break;
            }
        }
        if let Some(encoder) = encoder {
            // A later decode failure must not hide an earlier encode failure.
            // Only the successful original prefix reaches SID/encode work; all
            // statuses are still examined before any byte from this batch writes.
            apply(
                parallel.pool,
                first,
                &mut buffers.data[..prefix * PAGE_BYTES],
                &mut buffers.outcomes[..prefix],
                Transform::Encode(encoder),
                cancelled,
            )?;
        }
        for outcome in &mut buffers.outcomes {
            std::mem::replace(outcome, Ok(()))?;
        }
        check(budget, cancelled)?;
        tablecraft_io::write_exact_at(target, offset, &buffers.data, budget, cancelled)?;
        first += u32::try_from(count).map_err(|_| InspectError::StructureLimit)?;
    }
    Ok(())
}

fn apply(
    pool: &ThreadPool,
    first: u32,
    data: &mut [u8],
    outcomes: &mut [PageResult],
    transform: Transform<'_>,
    cancelled: &(dyn Fn() -> bool + Sync),
) -> Result<(), InspectError> {
    if data.len() != outcomes.len() * PAGE_BYTES {
        return Err(InspectError::MalformedEncryption);
    }
    pool.install(|| {
        data.par_chunks_exact_mut(PAGE_BYTES)
            .zip(outcomes.par_iter_mut())
            .enumerate()
            .for_each(|(index, (page, outcome))| {
                let mut local = ResourceBudget::new(PAGE_BYTES as u64, 0, PAGE_TRANSFORM_STEPS);
                *outcome = transform.apply(first + index as u32, page, &mut local, cancelled);
            });
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    use zeroize::Zeroizing;

    #[test]
    fn joined_statuses_keep_original_ordinals_and_failed_pages_keep_ciphertext() {
        let encrypted = super::super::tests::image();
        let mut budget = crate::encryption_rewrite_budget(encrypted.len() as u64).unwrap();
        let count = crate::inspect_header(&mut Cursor::new(&encrypted), &mut budget, &|| false)
            .unwrap()
            .page_count();
        let decoder = PageDecoder::open(
            &mut Cursor::new(&encrypted),
            Some("public password"),
            &mut budget,
            &|| false,
        )
        .unwrap();
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(4)
            .build()
            .unwrap();
        let mut data = Zeroizing::new(encrypted[PAGE_BYTES..7 * PAGE_BYTES].to_vec());
        let original = data.clone();
        let mut statuses = (0..6).map(|_| Ok(())).collect::<Vec<_>>();
        apply(
            &pool,
            count - 2,
            &mut data,
            &mut statuses,
            Transform::Decode(&decoder),
            &|| false,
        )
        .unwrap();
        assert!(statuses[..2].iter().all(Result::is_ok));
        assert!(
            statuses[2..]
                .iter()
                .all(|status| matches!(status, Err(InspectError::MalformedEncryption)))
        );
        assert_eq!(&data[2 * PAGE_BYTES..], &original[2 * PAGE_BYTES..]);
        assert_eq!(statuses.iter().position(Result::is_err), Some(2));
    }
}
