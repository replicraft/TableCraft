use crate::{
    InspectError, NativeReadSource, format::reader::Reader, limits::MAX_LONG_VALUE_CHUNKS,
};
use std::collections::HashSet;

// Conservative logical allocation allowance for each retained reference and its
// hash-table metadata/spare capacity; not an allocator-resident byte guarantee.
const REFERENCE_ALLOCATION_BYTES: u64 = 4 * size_of::<u32>() as u64;

pub(super) struct Visited {
    default: [u32; MAX_LONG_VALUE_CHUNKS],
    extended: Option<HashSet<u32>>,
}

impl Visited {
    pub(super) fn new(extended: bool) -> Self {
        Self {
            default: [0; MAX_LONG_VALUE_CHUNKS],
            extended: extended.then(HashSet::new),
        }
    }

    pub(super) fn insert<R: NativeReadSource>(
        &mut self,
        reference: u32,
        hop: usize,
        reader: &mut Reader<'_, R>,
    ) -> Result<(), InspectError> {
        if reference == 0 {
            return Err(InspectError::Malformed);
        }

        match &mut self.extended {
            None => {
                let visited = &mut self.default;
                reader.work(hop + 1)?;
                if visited[..hop].contains(&reference) {
                    return Err(InspectError::Malformed);
                }
                visited[hop] = reference;
            }
            Some(visited) => {
                reader.tick()?;
                if visited.contains(&reference) {
                    return Err(InspectError::Malformed);
                }
                reader
                    .budget
                    .charge_allocation(REFERENCE_ALLOCATION_BYTES, 1)?;
                visited
                    .try_reserve(1)
                    .map_err(|_| InspectError::Allocation)?;
                visited.insert(reference);
            }
        }

        Ok(())
    }
}
