//! Count shared metadata allocations once with cancellable pointer-only sorting.
use super::{ARC_COUNTERS, Binding};
use crate::{Error, display::Presentation};
use std::sync::Arc;

struct Allocation {
    identity: usize,
    bytes: usize,
}

pub(super) fn payload_bytes(
    bindings: &[Binding],
    output: &mut Presentation<'_>,
) -> Result<usize, Error> {
    let count = bindings
        .len()
        .checked_mul(2)
        .ok_or(Error::QueryContentsSize)?;
    output.allocation(count, size_of::<Allocation>())?;
    let mut allocations = Vec::new();
    allocations.try_reserve_exact(count)?;
    output.allocation(
        allocations.capacity().saturating_sub(count),
        size_of::<Allocation>(),
    )?;
    for binding in bindings {
        output.work(1)?;
        allocations.push(Allocation {
            identity: Arc::as_ptr(&binding.value).addr(),
            bytes: binding
                .value
                .retained_bytes()
                .checked_add(ARC_COUNTERS)
                .ok_or(Error::QueryContentsSize)?,
        });
        allocations.push(Allocation {
            identity: std::ptr::from_ref(binding.value.schema()).addr(),
            bytes: binding
                .value
                .schema()
                .retained_bytes()
                .checked_add(ARC_COUNTERS)
                .ok_or(Error::QueryContentsSize)?,
        });
    }
    // An owner-local heapsort permits cancellation during every sift. Only
    // allocation identities and cached lengths move; metadata is never copied.
    let count = allocations.len();
    for root in (0..count / 2).rev() {
        sift(&mut allocations, root, count, output)?;
    }
    for end in (1..count).rev() {
        output.work(1)?;
        allocations.swap(0, end);
        sift(&mut allocations, 0, end, output)?;
    }
    let mut previous = None;
    let mut bytes = 0_usize;
    for allocation in allocations {
        output.work(1)?;
        if previous != Some(allocation.identity) {
            bytes = bytes
                .checked_add(allocation.bytes)
                .ok_or(Error::QueryContentsSize)?;
            previous = Some(allocation.identity);
        }
    }
    Ok(bytes)
}

fn sift(
    values: &mut [Allocation],
    mut root: usize,
    length: usize,
    output: &mut Presentation<'_>,
) -> Result<(), Error> {
    while root < length / 2 {
        output.work(1)?;
        let mut child = 2 * root + 1;
        if child + 1 < length && values[child].identity < values[child + 1].identity {
            child += 1;
        }
        if values[root].identity >= values[child].identity {
            break;
        }
        values.swap(root, child);
        root = child;
    }
    Ok(())
}
