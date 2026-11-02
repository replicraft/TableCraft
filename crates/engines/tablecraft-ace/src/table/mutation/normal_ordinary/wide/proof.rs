//! Fresh original fixed records and manual signed key/ROW populations.
use super::vector;
use crate::{
    EditError, FixedColumnUpdates, InspectError, NativeEditPlan, NativeImage, NativeIndex,
    format::{bytes::u16_at, page::PAGE_BYTES},
    header::check,
    table::{
        indexes::header::IndexHeader,
        mutation::{
            fixed_records::{self, indexed::integral_updates::IntegralUpdates, proof as records},
            normal_ordinary::capture,
            preserved_source::{Source, validation},
        },
    },
};
use std::io::Cursor;
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_model::CatalogScope;
use tablecraft_schema::ScalarType;

const ACTIVE: usize = 480;
const MASK_START: usize = size_of::<IndexHeader>();
const MASK_BYTES: usize = ACTIVE - MASK_START;
const ROW_BYTES: usize = size_of::<u32>();
const ASC_PRESENT: u8 = 0x7f;
const DESC_PRESENT: u8 = 0x80;
const ASC_NULL: u8 = 0;
const DESC_NULL: u8 = 0xff;
const FREE_AT: usize = std::mem::offset_of!(IndexHeader, free);

fn units(kind: ScalarType, value: &Value) -> Result<Option<i64>, EditError> {
    match (kind, value) {
        (ScalarType::Int32 | ScalarType::Int64 | ScalarType::Currency, Value::Null) => Ok(None),
        (ScalarType::Int32, Value::Int32(value)) => Ok(Some(i64::from(*value))),
        (ScalarType::Int64, Value::Int64(value)) => Ok(Some(*value)),
        (ScalarType::Currency, Value::Currency(value)) => Ok(Some(value.scaled())),
        _ => Err(EditError::Preservation),
    }
}

fn requested<'a>(
    source: &Source,
    groups: &'a [FixedColumnUpdates<'_>],
    row: usize,
    column: usize,
    original: &'a Value,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<&'a Value, EditError> {
    let name = source.table.schema().columns()[column].name();
    let mut selected = original;
    for group in groups {
        check(budget, cancelled)?;
        fixed_records::name_work(name, group.column(), budget)?;
        if !name.eq_ignore_ascii_case(group.column()) {
            continue;
        }
        budget
            .charge(Resource::Steps, group.updates().len() as u64)
            .map_err(InspectError::from)?;
        if let Some(update) = group.updates().iter().find(|update| update.row() == row) {
            budget
                .charge(Resource::Steps, (2 * size_of::<i64>()) as u64)
                .map_err(InspectError::from)?;
            if original != update.expected() {
                return Err(EditError::Conflict);
            }
            selected = update.value();
        }
    }
    Ok(selected)
}

fn wire(
    value: Option<i64>,
    width: usize,
    descending: bool,
    row: u32,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, EditError> {
    let capacity = size_of::<u8>() + width + ROW_BYTES;
    let mut bytes = vector(capacity, budget, cancelled)?;
    budget
        .charge(Resource::Steps, capacity as u64)
        .map_err(InspectError::from)?;
    if let Some(value) = value {
        bytes.push(if descending {
            DESC_PRESENT
        } else {
            ASC_PRESENT
        });
        let encoded = match width {
            width if width == size_of::<i32>() => {
                u64::from((value as u32) ^ 0x8000_0000).to_be_bytes()
            }
            width if width == size_of::<i64>() => {
                ((value as u64) ^ 0x8000_0000_0000_0000).to_be_bytes()
            }
            _ => return Err(EditError::Preservation),
        };
        for byte in &encoded[encoded.len() - width..] {
            bytes.push(if descending { !*byte } else { *byte });
        }
    } else {
        bytes.push(if descending { DESC_NULL } else { ASC_NULL });
    }
    bytes.extend_from_slice(&row.to_be_bytes());
    Ok(bytes)
}

fn sorted(
    keys: &mut [Vec<u8>],
    maximum: usize,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    check(budget, cancelled)?;
    let work = keys
        .len()
        .checked_mul(keys.len().checked_ilog2().unwrap_or(0) as usize + 1)
        .and_then(|count| count.checked_mul(maximum))
        .ok_or(InspectError::StructureLimit)?;
    budget
        .charge(Resource::Steps, work as u64)
        .map_err(InspectError::from)?;
    keys.sort_unstable();
    check(budget, cancelled)?;
    Ok(())
}

fn frame(
    keys: &[Vec<u8>],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(Vec<u8>, [u8; MASK_BYTES]), EditError> {
    let mut length = 0_usize;
    for key in keys {
        check(budget, cancelled)?;
        length = length
            .checked_add(key.len())
            .filter(|length| *length <= PAGE_BYTES - ACTIVE)
            .ok_or(EditError::Preservation)?;
    }
    let mut active = vector(length, budget, cancelled)?;
    let mut bitmap = [0; MASK_BYTES];
    for key in keys {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, key.len() as u64)
            .map_err(InspectError::from)?;
        active.extend_from_slice(key);
        let end = active.len();
        bitmap[end / 8] |= 1 << (end % 8);
    }
    Ok((active, bitmap))
}

fn index(
    image: &NativeImage,
    source: &Source,
    selected: &NativeIndex,
    groups: &[FixedColumnUpdates<'_>],
    expected: &mut [u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    let [key] = selected.columns() else {
        return Err(EditError::Preservation);
    };
    let column = key.number();
    let field = source
        .table
        .schema()
        .columns()
        .get(column)
        .ok_or(EditError::Preservation)?;
    let kind = field.scalar_type().ok_or(EditError::Preservation)?;
    let width = field.storage_length();
    let mut original = vector(source.records.len(), budget, cancelled)?;
    let mut final_keys = vector(source.records.len(), budget, cancelled)?;
    let flags = selected.constraints().ok_or(EditError::Preservation)?;
    for (ordinal, record) in source.records.iter().enumerate() {
        check(budget, cancelled)?;
        let value = source
            .table
            .rows()
            .get(ordinal)
            .and_then(|row| row.get(column))
            .ok_or(EditError::Preservation)?;
        original.push(wire(
            units(kind, value)?,
            width,
            key.descending(),
            record.reference,
            budget,
            cancelled,
        )?);
        let replacement = requested(source, groups, ordinal, column, value, budget, cancelled)?;
        let final_value = units(kind, replacement)?;
        if flags.required() && final_value.is_none() {
            return Err(EditError::Preservation);
        }
        final_keys.push(wire(
            final_value,
            width,
            key.descending(),
            record.reference,
            budget,
            cancelled,
        )?);
    }
    let maximum = width + size_of::<u8>() + ROW_BYTES;
    sorted(&mut original, maximum, budget, cancelled)?;
    sorted(&mut final_keys, maximum, budget, cancelled)?;
    if flags.unique() {
        for pair in final_keys.windows(2) {
            check(budget, cancelled)?;
            budget
                .charge(Resource::Steps, (2 * maximum) as u64)
                .map_err(InspectError::from)?;
            if pair[0].len() != size_of::<u8>() + ROW_BYTES
                && pair[0][..pair[0].len() - ROW_BYTES] == pair[1][..pair[1].len() - ROW_BYTES]
            {
                return Err(EditError::Preservation);
            }
        }
    }
    let base = selected.root_page() as usize * PAGE_BYTES;
    let before = image
        .as_bytes()
        .get(base..base + PAGE_BYTES)
        .ok_or(EditError::Preservation)?;
    let (old_active, old_mask) = frame(&original, budget, cancelled)?;
    budget
        .charge(
            Resource::Steps,
            (MASK_BYTES + old_active.len() + PAGE_BYTES + size_of::<u16>()) as u64,
        )
        .map_err(InspectError::from)?;
    if before[MASK_START..ACTIVE] != old_mask
        || before[ACTIVE..ACTIVE + old_active.len()] != old_active
        || usize::from(u16_at(before, FREE_AT)?) != PAGE_BYTES - ACTIVE - old_active.len()
        || &expected[base..base + PAGE_BYTES] != before
    {
        return Err(EditError::Preservation);
    }
    let (active, bitmap) = frame(&final_keys, budget, cancelled)?;
    budget
        .charge(
            Resource::Steps,
            (MASK_BYTES + active.len() + size_of::<u16>()) as u64,
        )
        .map_err(InspectError::from)?;
    expected[base + MASK_START..base + ACTIVE].copy_from_slice(&bitmap);
    expected[base + ACTIVE..base + ACTIVE + active.len()].copy_from_slice(&active);
    let free =
        u16::try_from(PAGE_BYTES - ACTIVE - active.len()).map_err(|_| EditError::Preservation)?;
    expected[base + FREE_AT..base + FREE_AT + size_of::<u16>()]
        .copy_from_slice(&free.to_le_bytes());
    Ok(())
}

pub(super) fn prove(
    plan: &NativeEditPlan,
    scope: CatalogScope,
    table: &str,
    groups: &[FixedColumnUpdates<'_>],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    let image = NativeImage::capture(
        &mut Cursor::new(plan.before_bytes()),
        scope,
        budget,
        cancelled,
    )?;
    let source = capture(&image, table, budget, cancelled)?;
    let facts = IntegralUpdates::capture(&image, &source, budget, cancelled)?;
    let expected = fixed_records::copy(image.as_bytes(), budget, cancelled)?;
    let (mut expected, changed_rows) = records::reconstruct_integral_original(
        records::Original {
            image: &image,
            source: &source,
        },
        expected,
        groups,
        &facts,
        budget,
        cancelled,
    )?;
    for (position, selected) in facts.checked().indexes().iter().enumerate() {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, position as u64)
            .map_err(InspectError::from)?;
        let selected = selected.index();
        if facts.checked().indexes()[..position]
            .iter()
            .any(|prior| prior.index().data_number() == selected.data_number())
        {
            continue;
        }
        facts
            .ownership
            .require(&image, source.table.schema(), selected, budget, cancelled)?;
        index(
            &image,
            &source,
            selected,
            groups,
            &mut expected,
            budget,
            cancelled,
        )?;
    }
    if changed_rows != plan.changed_rows() || expected.len() != plan.after_bytes().len() {
        return Err(EditError::Preservation);
    }
    super::compare(&expected, plan.after_bytes(), budget, cancelled)?;
    validation::expected(&image, table, &source.table, &expected, budget, cancelled)?;
    let after = NativeImage::capture(&mut Cursor::new(&expected), scope, budget, cancelled)?;
    let final_source = capture(&after, table, budget, cancelled)?;
    let final_facts = IntegralUpdates::capture(&after, &final_source, budget, cancelled)?;
    if final_source.table.schema().object_id() != source.table.schema().object_id()
        || final_source.table.schema().definition_page() != source.table.schema().definition_page()
        || final_source.records.len() != source.records.len()
        || final_facts.checked().indexes().len() != facts.checked().indexes().len()
    {
        return Err(EditError::Preservation);
    }
    check(budget, cancelled)?;
    Ok(())
}
