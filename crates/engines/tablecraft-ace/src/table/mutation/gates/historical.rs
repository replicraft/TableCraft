//! Full-width original-root proof before historical fixed UPDATE admission.
use super::{EditError, WriteBlocker};
use crate::{
    InspectError, NativeImage, TableSchema,
    format::bytes::u16_at,
    header::check,
    limits::MAX_LIVE_ROWS,
    table::{
        mutation::{page_update::roots, rows::RowAssignment},
        record_layout,
    },
};
use tablecraft_core::limits::ResourceBudget;

struct Ordinal(usize);

impl RowAssignment for Ordinal {
    fn row(&self) -> usize {
        self.0
    }
}

pub(super) fn qualify(
    image: &NativeImage,
    schema: &TableSchema,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    check(budget, cancelled)?;
    let count = usize::try_from(schema.declared_rows())
        .ok()
        .filter(|count| *count <= MAX_LIVE_ROWS)
        .ok_or(InspectError::StructureLimit)?;
    budget
        .charge_allocation(count as u64, size_of::<Ordinal>() as u64)
        .map_err(InspectError::from)?;
    let mut ordinals = Vec::new();
    ordinals
        .try_reserve_exact(count)
        .map_err(|_| InspectError::Allocation)?;

    for ordinal in 0..count {
        check(budget, cancelled)?;
        ordinals.push(Ordinal(ordinal));
    }

    // The real owner proves all logical roots, complete DATA ownership and
    // exclusive final-record identities before returning selected records.
    let captured = roots::capture(image, schema, &ordinals, budget, cancelled)?;
    if captured.selected.len() != count {
        return Err(InspectError::Malformed.into());
    }

    for root in captured.selected {
        check(budget, cancelled)?;
        let data = &root.final_record.data;
        if usize::from(u16_at(data, 0)?) != schema.layout.max_columns {
            return Err(EditError::Unsupported(WriteBlocker::RowLayout));
        }
        record_layout::parse(data, schema, budget)?;
    }

    check(budget, cancelled)?;
    Ok(())
}
