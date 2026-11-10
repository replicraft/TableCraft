use crate::{InspectError, NativeReadSource, format::reader::Reader};
use std::collections::HashSet;

// Conservative logical allocation allowance for each retained reference and its
// hash-table metadata/spare capacity; not an allocator-resident byte guarantee.
const REFERENCE_ALLOCATION_BYTES: u64 = 4 * size_of::<u32>() as u64;

pub(crate) struct Visited {
    references: HashSet<u32>,
}

impl Visited {
    pub(crate) fn new() -> Self {
        Self {
            references: HashSet::new(),
        }
    }

    pub(crate) fn insert<R: NativeReadSource>(
        &mut self,
        reference: u32,
        reader: &mut Reader<'_, R>,
    ) -> Result<(), InspectError> {
        if reference == 0 {
            return Err(InspectError::Malformed);
        }

        reader.tick()?;
        if self.references.contains(&reference) {
            return Err(InspectError::Malformed);
        }
        reader
            .budget
            .charge_allocation(REFERENCE_ALLOCATION_BYTES, 1)?;
        self.references
            .try_reserve(1)
            .map_err(|_| InspectError::Allocation)?;
        self.references.insert(reference);

        Ok(())
    }
}
