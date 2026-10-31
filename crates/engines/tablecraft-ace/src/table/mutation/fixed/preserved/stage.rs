//! Exact original Boolean expectations become disjoint mask patches.
use super::{FixedColumnUpdates, int32, name_work, source::Source, unique};
use crate::{
    EditError, InspectError, NativeImage, WriteBlocker,
    format::row::MASK_BITS,
    header::check,
    table::mutation::{
        field,
        patch::{BytePatch, spans},
        rows,
    },
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_schema::ScalarType;

struct Bit {
    offset: usize,
    before: [u8; 1],
    after: [u8; 1],
}

impl BytePatch for Bit {
    fn offset(&self) -> usize {
        self.offset
    }

    fn before(&self) -> &[u8] {
        &self.before
    }

    fn after(&self) -> &[u8] {
        &self.after
    }

    fn is_mask(&self) -> bool {
        true
    }
}

pub(super) fn prepare(
    image: &NativeImage,
    source: &Source,
    groups: &[FixedColumnUpdates<'_>],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(Vec<spans::Patch>, usize), EditError> {
    // Original typed intent chooses this extra closed profile; the numeric
    // stage independently checks actual native declaration and non-NULL values.
    // Healthy Boolean paths retain their existing allocations, polls and charges.
    if int32::requested(groups) {
        return int32::prepare(image, source, groups, budget, cancelled);
    }
    let schema = source.table.schema();
    let count = groups
        .iter()
        .try_fold(0_usize, |count, group| {
            count.checked_add(group.updates().len())
        })
        .ok_or(InspectError::StructureLimit)?;
    budget
        .charge_allocation(count as u64, size_of::<u32>() as u64)
        .map_err(InspectError::from)?;
    let mut changed = Vec::new();
    changed
        .try_reserve_exact(count)
        .map_err(|_| InspectError::Allocation)?;
    budget
        .charge_allocation(count as u64, size_of::<Bit>() as u64)
        .map_err(InspectError::from)?;
    let mut bits = Vec::new();
    bits.try_reserve_exact(count)
        .map_err(|_| InspectError::Allocation)?;
    let mut patches = Vec::new();

    for (position, group) in groups.iter().enumerate() {
        check(budget, cancelled)?;
        for prior in &groups[..position] {
            name_work(prior.column(), group.column(), budget)?;
            if prior.column().eq_ignore_ascii_case(group.column()) {
                return Err(EditError::Conflict);
            }
        }
        for field in schema.columns() {
            name_work(field.name(), group.column(), budget)?;
        }
        let column = match field::fixed_column(schema, group.column(), ScalarType::Boolean, 0) {
            Ok(column) => column,
            Err(error @ EditError::Unsupported(WriteBlocker::Column))
                if groups.len() == 1 && group.updates().is_empty() =>
            {
                // Empty numeric requests have no typed value to select a profile.
                // Existing zero-capacity Boolean buffers allocate no heap here.
                if int32::empty_target(schema, group, budget, cancelled)? {
                    return int32::prepare(image, source, groups, budget, cancelled);
                }
                return Err(error);
            }
            Err(error) => return Err(error),
        };
        budget
            .charge(
                Resource::Steps,
                (source.dependencies.len()
                    + source
                        .indexes
                        .indexes()
                        .iter()
                        .map(|index| index.columns().len())
                        .sum::<usize>()) as u64,
            )
            .map_err(InspectError::from)?;
        if source.dependencies.contains(&column)
            || source
                .indexes
                .indexes()
                .iter()
                .any(|index| index.columns().iter().any(|key| key.number() == column))
        {
            return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
        }
        rows::unique(group.updates(), budget, cancelled)?;
        for update in group.updates() {
            check(budget, cancelled)?;
            if !matches!(
                (update.expected(), update.value()),
                (Value::Boolean(_), Value::Boolean(_))
            ) {
                return Err(EditError::Unsupported(WriteBlocker::Column));
            }
        }

        rows::final_record::visit(
            image,
            schema,
            group.updates(),
            budget,
            cancelled,
            |row, update| {
                let Value::Boolean(expected) = update.expected() else {
                    return Err(EditError::Preservation);
                };
                let Value::Boolean(value) = update.value() else {
                    return Err(EditError::Preservation);
                };
                let at = row.mask + column / MASK_BITS;
                let before = *row.data.get(at).ok_or(InspectError::Malformed)?;
                let bit = 1 << (column % MASK_BITS);
                if (before & bit != 0) != *expected {
                    return Err(EditError::Conflict);
                }
                if expected == value {
                    return Ok(());
                }
                let after = if *value { before | bit } else { before & !bit };
                bits.push(Bit {
                    offset: row.offset + at,
                    before: [before],
                    after: [after],
                });
                changed.push(row.reference);
                Ok(())
            },
        )?;
    }
    for bit in bits {
        spans::push(&mut patches, &bit, budget, cancelled)?;
    }
    spans::coalesce(&mut patches, budget, cancelled)?;
    let count = unique(&mut changed, budget, cancelled)?;
    Ok((patches, count))
}
