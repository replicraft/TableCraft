//! Manual raw scalar binding and independent complete GUID components/ROWs.
use crate::{
    EditError, FixedColumnUpdates, InspectError, NativeColumn, NativeIndex,
    format::data_type::{GUID, INT32},
    header::check,
    table::{
        mutation::{fixed_records, preserved_source::Source},
        record_layout,
    },
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};

const ROW_BYTES: usize = size_of::<u32>();
const GUID_BYTES: usize = 16;
const GUID_SEGMENT_BYTES: usize = 8;
const GUID_CONTINUED_TAG: u8 = 9;
const GUID_FINAL_TAG: u8 = 8;
const GUID_COMPONENT_BYTES: usize = 1 + GUID_BYTES + 2;
const MASK_BITS: usize = u8::BITS as usize;
const I32_SIGN_MASK: u32 = 0x8000_0000;
const ORIGINAL_VALUE_PASSES: usize = 5;
const REQUESTED_VALUE_PASSES: usize = 6;
const CONSTRAINT_VALUE_PASSES: usize = 4;
const GUID_DATA_BYTE_ORDER: [usize; GUID_BYTES] =
    [3, 2, 1, 0, 5, 4, 7, 6, 8, 9, 10, 11, 12, 13, 14, 15];

#[derive(Clone, Copy, PartialEq, Eq)]
enum Key {
    I32(i32),
    Guid([u8; GUID_BYTES]),
}

pub(super) struct Population {
    pub original: Vec<Vec<u8>>,
    pub final_keys: Vec<Vec<u8>>,
}

fn vector<T>(count: usize, budget: &mut ResourceBudget) -> Result<Vec<T>, EditError> {
    budget
        .charge_allocation(count as u64, size_of::<T>() as u64)
        .map_err(InspectError::from)?;
    let mut output = Vec::new();
    output
        .try_reserve_exact(count)
        .map_err(|_| InspectError::Allocation)?;
    Ok(output)
}

fn cell(field: &NativeColumn, value: &Value) -> Result<Option<Key>, EditError> {
    match (field.native_type(), value) {
        (GUID | INT32, Value::Null) => Ok(None),
        (GUID, Value::Guid(value)) => Ok(Some(Key::Guid(value.to_bytes()))),
        (INT32, Value::Int32(value)) => Ok(Some(Key::I32(*value))),
        _ => Err(EditError::Preservation),
    }
}

fn original(
    source: &Source,
    raw: &[u8],
    field: &NativeColumn,
    observed: &Value,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Option<Key>, EditError> {
    check(budget, cancelled)?;
    let layout = record_layout::parse(raw, source.table.schema(), budget)?;
    if field.number() >= layout.count {
        return Err(EditError::Preservation);
    }
    let mask = raw
        .get(layout.mask_start + field.number() / MASK_BITS)
        .ok_or(EditError::Preservation)?;
    budget
        .charge(
            Resource::Steps,
            (field.storage_length() * ORIGINAL_VALUE_PASSES) as u64,
        )
        .map_err(InspectError::from)?;
    let value = cell(field, observed)?;
    if mask & (1 << (field.number() % MASK_BITS)) == 0 {
        return if value.is_none() {
            Ok(None)
        } else {
            Err(EditError::Preservation)
        };
    }
    let start = size_of::<u16>()
        .checked_add(field.field.offset)
        .ok_or(EditError::Preservation)?;
    let end = start
        .checked_add(field.storage_length())
        .filter(|end| *end <= layout.data_end)
        .ok_or(EditError::Preservation)?;
    let stored = raw.get(start..end).ok_or(EditError::Preservation)?;
    let actual = match field.native_type() {
        GUID => {
            let bytes: [u8; GUID_BYTES] = stored.try_into().map_err(|_| EditError::Preservation)?;
            Key::Guid(GUID_DATA_BYTE_ORDER.map(|at| bytes[at]))
        }
        INT32 => Key::I32(i32::from_le_bytes(
            stored.try_into().map_err(|_| EditError::Preservation)?,
        )),
        _ => return Err(EditError::Preservation),
    };
    if value != Some(actual) {
        return Err(EditError::Preservation);
    }
    Ok(value)
}

fn requested(
    field: &NativeColumn,
    groups: &[FixedColumnUpdates<'_>],
    ordinal: usize,
    before: Option<Key>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Option<Key>, EditError> {
    let mut replacement = None;
    for group in groups {
        check(budget, cancelled)?;
        fixed_records::name_work(field.name(), group.column(), budget)?;
        if !field.name().eq_ignore_ascii_case(group.column()) {
            continue;
        }
        for update in group.updates() {
            check(budget, cancelled)?;
            if update.row() != ordinal {
                continue;
            }
            budget
                .charge(
                    Resource::Steps,
                    (REQUESTED_VALUE_PASSES * GUID_BYTES) as u64,
                )
                .map_err(InspectError::from)?;
            if cell(field, update.expected())? != before
                || replacement.replace(cell(field, update.value())?).is_some()
            {
                return Err(EditError::Preservation);
            }
        }
    }
    Ok(replacement.unwrap_or(before))
}

fn wire(
    key: Option<Key>,
    reference: u32,
    guid: bool,
    descending: bool,
    budget: &mut ResourceBudget,
) -> Result<Vec<u8>, EditError> {
    let width = if guid {
        GUID_COMPONENT_BYTES
    } else {
        1 + size_of::<i32>()
    } + ROW_BYTES;
    let mut output = vector(width, budget)?;
    budget
        .charge(Resource::Steps, width as u64)
        .map_err(InspectError::from)?;
    match key {
        None => output.push(if descending { 0xff } else { 0 }),
        Some(Key::I32(value)) if !guid => {
            output.push(if descending { 0x80 } else { 0x7f });
            let mut bytes = ((value as u32) ^ I32_SIGN_MASK).to_be_bytes();
            if descending {
                bytes.iter_mut().for_each(|byte| *byte = !*byte);
            }
            output.extend_from_slice(&bytes);
        }
        Some(Key::Guid(mut bytes)) if guid => {
            output.push(if descending { 0x80 } else { 0x7f });
            if descending {
                bytes.iter_mut().for_each(|byte| *byte = !*byte);
            }
            output.extend_from_slice(&bytes[..GUID_SEGMENT_BYTES]);
            output.push(GUID_CONTINUED_TAG);
            output.extend_from_slice(&bytes[GUID_SEGMENT_BYTES..]);
            output.push(if descending {
                !GUID_FINAL_TAG
            } else {
                GUID_FINAL_TAG
            });
        }
        _ => return Err(EditError::Preservation),
    }
    output.extend_from_slice(&reference.to_be_bytes());
    Ok(output)
}

pub(super) fn capture(
    source: &Source,
    index: &NativeIndex,
    groups: &[FixedColumnUpdates<'_>],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Population, EditError> {
    check(budget, cancelled)?;
    let [column] = index.columns() else {
        return Err(EditError::Preservation);
    };
    let field = source
        .table
        .schema()
        .columns()
        .get(column.number())
        .ok_or(EditError::Preservation)?;
    let mut original_keys: Vec<Vec<u8>> = vector(source.records.len(), budget)?;
    let mut final_values: Vec<(Vec<u8>, Option<Key>)> = vector(source.records.len(), budget)?;
    for (ordinal, record) in source.records.iter().enumerate() {
        check(budget, cancelled)?;
        let observed = source
            .table
            .rows()
            .get(ordinal)
            .and_then(|row| row.get(column.number()))
            .ok_or(EditError::Preservation)?;
        let before = original(source, &record.data, field, observed, budget, cancelled)?;
        let after = requested(field, groups, ordinal, before, budget, cancelled)?;
        original_keys.push(wire(
            before,
            record.reference,
            field.native_type() == GUID,
            column.descending(),
            budget,
        )?);
        final_values.push((
            wire(
                after,
                record.reference,
                field.native_type() == GUID,
                column.descending(),
                budget,
            )?,
            after,
        ));
    }
    let work = source
        .records
        .len()
        .checked_mul(source.records.len().checked_ilog2().unwrap_or(0) as usize + 1)
        .and_then(|n| n.checked_mul(2 * (GUID_COMPONENT_BYTES + ROW_BYTES)))
        .ok_or(InspectError::StructureLimit)?;
    budget
        .charge(Resource::Steps, work as u64)
        .map_err(InspectError::from)?;
    original_keys.sort_unstable();
    final_values.sort_unstable_by(|left, right| left.0.cmp(&right.0));
    let constraints = index.constraints().ok_or(EditError::Preservation)?;
    let mut previous = None;
    let mut final_keys = vector(final_values.len(), budget)?;
    for (wire, key) in final_values {
        check(budget, cancelled)?;
        // This final pass copies the key and may compare it with its
        // predecessor; sorting paid for a different set of comparisons.
        budget
            .charge(
                Resource::Steps,
                (CONSTRAINT_VALUE_PASSES * GUID_BYTES) as u64,
            )
            .map_err(InspectError::from)?;
        if key.is_none() && constraints.required() {
            return Err(EditError::Preservation);
        }
        if let Some(value) = key {
            if constraints.unique() && previous == Some(value) {
                return Err(EditError::Preservation);
            }
            previous = Some(value);
        }
        final_keys.push(wire);
    }
    Ok(Population {
        original: original_keys,
        final_keys,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn budget() -> ResourceBudget {
        ResourceBudget::new(4096, 64, 4096)
    }

    #[test]
    fn canonical_guid_segments_keep_direction_tags_and_original_row_suffix() {
        let canonical = [
            0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd, 0xee,
            0xff, 0,
        ];
        assert_eq!(
            wire(
                Some(Key::Guid(canonical)),
                0x005d_0000,
                true,
                false,
                &mut budget()
            )
            .unwrap(),
            [
                0x7f, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 9, 0x99, 0xaa, 0xbb, 0xcc,
                0xdd, 0xee, 0xff, 0, 8, 0, 0x5d, 0, 0,
            ],
        );
        assert_eq!(
            wire(
                Some(Key::Guid(canonical)),
                0x005d_0000,
                true,
                true,
                &mut budget()
            )
            .unwrap(),
            [
                0x80, 0xee, 0xdd, 0xcc, 0xbb, 0xaa, 0x99, 0x88, 0x77, 9, 0x66, 0x55, 0x44, 0x33,
                0x22, 0x11, 0, 0xff, 0xf7, 0, 0x5d, 0, 0,
            ],
        );
    }

    #[test]
    fn null_membership_and_retained_signed_i32_have_separate_manual_wire() {
        assert_eq!(
            wire(None, 0x005f_0011, true, false, &mut budget()).unwrap(),
            [0, 0, 0x5f, 0, 0x11],
        );
        assert_eq!(
            wire(None, 0x005f_0011, true, true, &mut budget()).unwrap(),
            [0xff, 0, 0x5f, 0, 0x11],
        );
        assert_eq!(
            wire(
                Some(Key::I32(-64)),
                0x005d_0000,
                false,
                false,
                &mut budget()
            )
            .unwrap(),
            [0x7f, 0x7f, 0xff, 0xff, 0xc0, 0, 0x5d, 0, 0],
        );
        assert_eq!(
            wire(Some(Key::I32(-64)), 0x005d_0000, false, true, &mut budget()).unwrap(),
            [0x80, 0x80, 0, 0, 0x3f, 0, 0x5d, 0, 0],
        );
    }
}
