//! Independent Int32 presence/payload facts from selected original logical roots.
use super::push;
use crate::{
    EditError, FixedColumnUpdates, InspectError, NativeImage, TableSchema, WriteBlocker,
    format::{
        bytes::u16_at,
        row::{COLUMN_COUNT_BYTES, MASK_BITS},
    },
    header::check,
    table::{
        indexes::{IndexKeyUpdate, MutationKey},
        mutation::{field, page_update::roots},
    },
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_schema::ScalarType;

pub(super) struct Byte {
    pub offset: usize,
    pub after: u8,
    assigned: u8,
}

pub(super) struct Fields {
    pub bytes: Vec<Byte>,
    pub keys: Vec<IndexKeyUpdate>,
    pub changed: usize,
}

pub(super) fn derive(
    image: &NativeImage,
    schema: &TableSchema,
    groups: &[FixedColumnUpdates<'_>],
    after: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Fields, EditError> {
    let mut bytes: Vec<Byte> = Vec::new();
    let mut keys = Vec::new();
    let mut changed = Vec::new();
    let mut assignments = 0usize;
    let kind = MutationKey::new(ScalarType::Int32)?;
    for group in groups {
        check(budget, cancelled)?;
        assignments = assignments
            .checked_add(group.updates().len())
            .filter(|count| *count <= NativeImage::MAX_FIXED_BATCH_ASSIGNMENTS)
            .ok_or(InspectError::StructureLimit)?;
        let column =
            field::fixed_column(schema, group.column(), ScalarType::Int32, size_of::<i32>())?;
        let metadata = &schema.columns()[column];
        let captured = roots::capture(image, schema, group.updates(), budget, cancelled)?;
        for root in captured.selected {
            check(budget, cancelled)?;
            let record = &root.final_record.data;
            if usize::from(u16_at(record, 0)?) != schema.layout.max_columns {
                return Err(EditError::Unsupported(WriteBlocker::RowLayout));
            }
            let mask = record
                .len()
                .checked_sub(schema.layout.max_columns.div_ceil(MASK_BITS))
                .ok_or(InspectError::Malformed)?;
            let mask_offset = mask + metadata.number() / MASK_BITS;
            let stored_mask = *record.get(mask_offset).ok_or(InspectError::Malformed)?;
            let bit = 1 << (metadata.number() % MASK_BITS);
            let expected = value(root.update.expected())?;
            let value = value(root.update.value())?;
            if (stored_mask & bit != 0) != expected.is_some() {
                return Err(EditError::Conflict);
            }
            let offset = COLUMN_COUNT_BYTES + metadata.field.offset;
            let end = offset
                .checked_add(size_of::<i32>())
                .filter(|end| *end <= mask)
                .ok_or(EditError::Unsupported(WriteBlocker::RowLayout))?;
            let stored = record.get(offset..end).ok_or(InspectError::Malformed)?;
            if let Some(expected) = expected
                && stored != expected.to_le_bytes()
            {
                return Err(EditError::Conflict);
            }
            if expected == value {
                continue;
            }
            budget
                .charge(Resource::Steps, changed.len() as u64)
                .map_err(InspectError::from)?;
            if !changed.contains(&root.reference) {
                push(&mut changed, root.reference, budget)?;
            }
            // Include unindexed fields too. Physical audit selects actual
            // metadata components, never caller-supplied column positions.
            push(
                &mut keys,
                IndexKeyUpdate {
                    reference: root.reference,
                    column,
                    expected: expected
                        .map(|value| kind.stored(&value.to_le_bytes()))
                        .transpose()?,
                    value: value
                        .map(|value| kind.stored(&value.to_le_bytes()))
                        .transpose()?,
                },
                budget,
            )?;
            if let Some(value) = value {
                for (position, byte) in value.to_le_bytes().into_iter().enumerate() {
                    merge(
                        &mut bytes,
                        root.final_record.offset + offset + position,
                        byte,
                        u8::MAX,
                        image,
                        budget,
                    )?;
                }
            }
            let next_mask = (stored_mask & !bit) | if value.is_some() { bit } else { 0 };
            merge(
                &mut bytes,
                root.final_record.offset + mask_offset,
                next_mask,
                bit,
                image,
                budget,
            )?;
        }
    }
    if after.len() < image.as_bytes().len() {
        return Err(EditError::Preservation);
    }
    Ok(Fields {
        bytes,
        keys,
        changed: changed.len(),
    })
}

fn value(value: &Value) -> Result<Option<i32>, EditError> {
    match value {
        Value::Null => Ok(None),
        Value::Int32(value) => Ok(Some(*value)),
        _ => Err(EditError::Unsupported(WriteBlocker::Column)),
    }
}

fn merge(
    bytes: &mut Vec<Byte>,
    offset: usize,
    value: u8,
    assigned: u8,
    image: &NativeImage,
    budget: &mut ResourceBudget,
) -> Result<(), EditError> {
    budget
        .charge(Resource::Steps, bytes.len() as u64)
        .map_err(InspectError::from)?;
    if let Some(existing) = bytes.iter_mut().find(|byte| byte.offset == offset) {
        if existing.assigned & assigned != 0 {
            return Err(EditError::Conflict);
        }
        existing.after = (existing.after & !assigned) | (value & assigned);
        existing.assigned |= assigned;
    } else {
        let original = *image
            .as_bytes()
            .get(offset)
            .ok_or(EditError::Preservation)?;
        push(
            bytes,
            Byte {
                offset,
                after: (original & !assigned) | (value & assigned),
                assigned,
            },
            budget,
        )?;
    }
    Ok(())
}
