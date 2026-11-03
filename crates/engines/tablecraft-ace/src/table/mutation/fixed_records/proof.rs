//! Independent original authority for preserved joint fixed-field candidates.
use super::FixedCarriers;
#[cfg(feature = "ace16-write-preview")]
use super::copy;
use super::{
    indexed::{CheckedIndexes, IndexTargets},
    name_work, unique,
};
#[cfg(feature = "ace16-write-preview")]
use crate::table::mutation::preserved_source as source;
use crate::table::mutation::{
    preserved_source::{Source, validation},
    requests::FixedColumnUpdates,
};
use crate::{
    EditError, InspectError, NativeColumn, NativeImage, WriteBlocker,
    format::{
        data_type::{
            BOOLEAN, BYTE, CURRENCY, DATE_TIME, DOUBLE, GUID, GUID_BYTES, INT16, INT32, INT64,
            SINGLE,
        },
        layout::{FIXED_FLAG, UPDATABLE_FLAG},
        row::{COLUMN_COUNT_BYTES, MASK_BITS},
    },
    header::check,
    preservation::COPY_CHUNK_BYTES,
    table::record_layout,
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_schema::ScalarType;

const WIDE_BYTES: usize = size_of::<i64>();
const INT32_BYTES: usize = size_of::<i32>();
const GUID_DATA_BYTE_ORDER: [usize; GUID_BYTES] =
    [3, 2, 1, 0, 5, 4, 7, 6, 8, 9, 10, 11, 12, 13, 14, 15];
// Two encodings, expected/Cell comparisons and the final payload copy.
const GUID_PAYLOAD_PASSES: usize = 5;

#[derive(Clone, Copy)]
enum Carrier {
    Boolean,
    Int32,
    Int64,
    Currency,
    DateTime,
    Single,
    Double,
    Byte,
    Int16,
    Guid,
}

struct Bound<'a> {
    field: &'a NativeColumn,
    carrier: Carrier,
    width: usize,
}

#[derive(PartialEq, Eq)]
enum Cell<const N: usize> {
    Boolean(bool),
    Fixed(Option<[u8; N]>),
}

pub(in crate::table::mutation) struct Original<'a> {
    pub image: &'a NativeImage,
    pub source: &'a Source,
}

pub(in crate::table::mutation) struct Observed<'a> {
    pub bytes: &'a [u8],
    pub changed_rows: usize,
}

#[cfg(feature = "ace16-write-preview")]
pub(in crate::table::mutation) fn prove(
    image: &NativeImage,
    groups: &[FixedColumnUpdates<'_>],
    after: &[u8],
    returned_count: usize,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    check(budget, cancelled)?;
    let first = groups.first().ok_or(EditError::Preservation)?;
    if groups.len() > NativeImage::MAX_FIXED_BATCH_GROUPS {
        return Err(InspectError::StructureLimit.into());
    }
    let source = source::capture(image, first.table(), budget, cancelled)?;
    let expected = copy(image.as_bytes(), budget, cancelled)?;
    prove_original(
        Original {
            image,
            source: &source,
        },
        expected,
        groups,
        Observed {
            bytes: after,
            changed_rows: returned_count,
        },
        budget,
        cancelled,
    )
}

pub(in crate::table::mutation) fn prove_original(
    original: Original<'_>,
    expected: Vec<u8>,
    groups: &[FixedColumnUpdates<'_>],
    observed: Observed<'_>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    prove_with_policy::<WIDE_BYTES>(
        original,
        expected,
        groups,
        observed,
        FixedCarriers::Legacy,
        budget,
        cancelled,
    )
}

#[cfg(feature = "journal")]
pub(in crate::table::mutation) fn prove_ieee_original(
    original: Original<'_>,
    expected: Vec<u8>,
    groups: &[FixedColumnUpdates<'_>],
    observed: Observed<'_>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    prove_with_policy::<WIDE_BYTES>(
        original,
        expected,
        groups,
        observed,
        FixedCarriers::Ieee,
        budget,
        cancelled,
    )
}

#[cfg(feature = "journal")]
pub(in crate::table::mutation) fn prove_scalar_original(
    original: Original<'_>,
    expected: Vec<u8>,
    groups: &[FixedColumnUpdates<'_>],
    observed: Observed<'_>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    prove_with_policy::<WIDE_BYTES>(
        original,
        expected,
        groups,
        observed,
        FixedCarriers::Scalar,
        budget,
        cancelled,
    )
}

#[cfg(feature = "journal")]
pub(in crate::table::mutation) fn prove_guid_original(
    original: Original<'_>,
    expected: Vec<u8>,
    groups: &[FixedColumnUpdates<'_>],
    observed: Observed<'_>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    prove_with_policy::<GUID_BYTES>(
        original,
        expected,
        groups,
        observed,
        FixedCarriers::Guid,
        budget,
        cancelled,
    )
}

fn prove_with_policy<const N: usize>(
    original: Original<'_>,
    expected: Vec<u8>,
    groups: &[FixedColumnUpdates<'_>],
    observed: Observed<'_>,
    carriers: FixedCarriers,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    check(budget, cancelled)?;
    let image = original.image;
    let original = original.source;
    let after = observed.bytes;
    let returned_count = observed.changed_rows;
    let first = groups.first().ok_or(EditError::Preservation)?;
    if expected.len() != image.as_bytes().len() || after.len() != image.as_bytes().len() {
        return Err(EditError::Preservation);
    }
    let (expected, changed_rows) = if carriers == FixedCarriers::Legacy {
        reconstruct(original, expected, groups, None, budget, cancelled)?
    } else {
        reconstruct_with_targets::<N>(
            original, expected, groups, None, carriers, budget, cancelled,
        )?
    };
    if changed_rows != returned_count || expected.len() != after.len() {
        return Err(EditError::Preservation);
    }
    validation::expected(
        image,
        first.table(),
        &original.table,
        &expected,
        budget,
        cancelled,
    )?;
    for (expected, observed) in expected
        .chunks(COPY_CHUNK_BYTES)
        .zip(after.chunks(COPY_CHUNK_BYTES))
    {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, expected.len() as u64)
            .map_err(InspectError::from)?;
        if expected != observed {
            return Err(EditError::Preservation);
        }
    }
    check(budget, cancelled)?;
    Ok(())
}

/// Reconstruct fixed records from ORIGINAL requests and original raw records.
/// The caller must replay affected indexes into these bytes, then validate
/// original constraints and compare the whole image/count. This returns no
/// plan, event result, byte patch authority or reusable content permission.
#[cfg(feature = "journal")]
pub(in crate::table::mutation) fn reconstruct_original(
    original: Original<'_>,
    expected: Vec<u8>,
    groups: &[FixedColumnUpdates<'_>],
    indexed: Option<&CheckedIndexes<'_>>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(Vec<u8>, usize), EditError> {
    check(budget, cancelled)?;
    groups.first().ok_or(EditError::Preservation)?;
    if expected.len() != original.image.as_bytes().len() {
        return Err(EditError::Preservation);
    }
    if let Some(indexed) = indexed {
        indexed.require_source(
            original.image,
            original.source.table.schema(),
            budget,
            cancelled,
        )?;
    }
    reconstruct(
        original.source,
        expected,
        groups,
        indexed,
        budget,
        cancelled,
    )
}

fn reconstruct(
    original: &Source,
    expected: Vec<u8>,
    groups: &[FixedColumnUpdates<'_>],
    indexed: Option<&CheckedIndexes<'_>>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(Vec<u8>, usize), EditError> {
    reconstruct_with_targets::<WIDE_BYTES>(
        original,
        expected,
        groups,
        indexed.map(|facts| facts as &dyn IndexTargets),
        FixedCarriers::Legacy,
        budget,
        cancelled,
    )
}

#[cfg(feature = "journal")]
pub(in crate::table::mutation) fn reconstruct_integral_original(
    original: Original<'_>,
    expected: Vec<u8>,
    groups: &[FixedColumnUpdates<'_>],
    indexed: &super::indexed::integral_updates::IntegralUpdates<'_>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(Vec<u8>, usize), EditError> {
    check(budget, cancelled)?;
    groups.first().ok_or(EditError::Preservation)?;
    if expected.len() != original.image.as_bytes().len() {
        return Err(EditError::Preservation);
    }
    indexed.require_source(
        original.image,
        original.source.table.schema(),
        budget,
        cancelled,
    )?;
    reconstruct_with_targets::<WIDE_BYTES>(
        original.source,
        expected,
        groups,
        Some(indexed),
        FixedCarriers::Legacy,
        budget,
        cancelled,
    )
}

#[cfg(feature = "journal")]
pub(in crate::table::mutation) fn reconstruct_i32_updates_original(
    original: Original<'_>,
    expected: Vec<u8>,
    groups: &[FixedColumnUpdates<'_>],
    indexed: &super::indexed::i32_updates::I32UpdateIndexes<'_>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(Vec<u8>, usize), EditError> {
    if let Some(legacy) = indexed.legacy() {
        return reconstruct_original(original, expected, groups, Some(legacy), budget, cancelled);
    }
    check(budget, cancelled)?;
    groups.first().ok_or(EditError::Preservation)?;
    if expected.len() != original.image.as_bytes().len() {
        return Err(EditError::Preservation);
    }
    indexed.require_source(
        original.image,
        original.source.table.schema(),
        budget,
        cancelled,
    )?;
    reconstruct_with_targets::<WIDE_BYTES>(
        original.source,
        expected,
        groups,
        Some(indexed),
        FixedCarriers::Legacy,
        budget,
        cancelled,
    )
}

fn reconstruct_with_targets<const N: usize>(
    original: &Source,
    mut expected: Vec<u8>,
    groups: &[FixedColumnUpdates<'_>],
    indexed: Option<&dyn IndexTargets>,
    carriers: FixedCarriers,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(Vec<u8>, usize), EditError> {
    if groups.len() > NativeImage::MAX_FIXED_BATCH_GROUPS {
        return Err(InspectError::StructureLimit.into());
    }
    let assignments = groups
        .iter()
        .try_fold(0_usize, |count, group| {
            count.checked_add(group.updates().len())
        })
        .filter(|count| *count <= NativeImage::MAX_FIXED_BATCH_ASSIGNMENTS)
        .ok_or(InspectError::StructureLimit)?;
    budget
        .charge_allocation(groups.len() as u64, size_of::<Bound<'_>>() as u64)
        .map_err(InspectError::from)?;
    let mut bound: Vec<Bound<'_>> = Vec::new();
    bound
        .try_reserve_exact(groups.len())
        .map_err(|_| InspectError::Allocation)?;
    for group in groups {
        check(budget, cancelled)?;
        let selected = select(original, group, indexed, carriers, budget, cancelled)?;
        if selected.width > N {
            return Err(EditError::Preservation);
        }
        budget
            .charge(Resource::Steps, bound.len() as u64)
            .map_err(InspectError::from)?;
        if bound
            .iter()
            .any(|prior| prior.field.number() == selected.field.number())
        {
            return Err(EditError::Conflict);
        }
        bound.push(selected);
    }

    // Every declaration, including an empty group's, is qualified before any
    // requested record is visited. Nothing is borrowed from the producer stage.
    budget
        .charge_allocation(assignments as u64, size_of::<u32>() as u64)
        .map_err(InspectError::from)?;
    let mut references = Vec::new();
    references
        .try_reserve_exact(assignments)
        .map_err(|_| InspectError::Allocation)?;
    for (group, selected) in groups.iter().zip(&bound) {
        let number = selected.field.number();
        for (position, request) in group.updates().iter().enumerate() {
            check(budget, cancelled)?;
            let payload_work = if carriers.includes_guid() {
                N.checked_mul(GUID_PAYLOAD_PASSES)
                    .ok_or(InspectError::StructureLimit)?
            } else {
                selected.width * 2
            };
            budget
                .charge(Resource::Steps, position as u64 + payload_work as u64)
                .map_err(InspectError::from)?;
            if group.updates()[..position]
                .iter()
                .any(|prior| prior.row() == request.row())
            {
                return Err(EditError::Conflict);
            }
            let before = encode::<N>(selected.carrier, request.expected())?;
            let replacement = encode::<N>(selected.carrier, request.value())?;
            let record = original
                .records
                .get(request.row())
                .ok_or(EditError::Conflict)?;
            let layout = record_layout::parse(&record.data, original.table.schema(), budget)?;
            if number >= layout.count {
                return Err(EditError::Preservation);
            }
            let mask_at = layout
                .mask_start
                .checked_add(number / MASK_BITS)
                .ok_or(EditError::Preservation)?;
            let mask = *record.data.get(mask_at).ok_or(InspectError::Malformed)?;
            let bit = 1 << (number % MASK_BITS);
            let output_mask_at = record
                .offset
                .checked_add(mask_at)
                .ok_or(EditError::Preservation)?;
            match (&before, &replacement) {
                (Cell::Boolean(before), Cell::Boolean(value)) => {
                    if (mask & bit != 0) != *before {
                        return Err(EditError::Conflict);
                    }
                    if before == value {
                        continue;
                    }
                    set_bit(&mut expected, output_mask_at, bit, *value)?;
                }
                (Cell::Fixed(before), Cell::Fixed(value)) => {
                    if (mask & bit != 0) != before.is_some() {
                        return Err(EditError::Conflict);
                    }
                    let at = COLUMN_COUNT_BYTES
                        .checked_add(selected.field.field.offset)
                        .ok_or(EditError::Preservation)?;
                    let end = at
                        .checked_add(selected.width)
                        .ok_or(EditError::Preservation)?;
                    if end > layout.data_end {
                        return Err(EditError::Preservation);
                    }
                    let actual = record.data.get(at..end).ok_or(InspectError::Malformed)?;
                    if before
                        .as_ref()
                        .is_some_and(|bytes| actual != &bytes[..selected.width])
                    {
                        return Err(EditError::Conflict);
                    }
                    if before == value {
                        continue;
                    }
                    if let Some(bytes) = value {
                        let start = record
                            .offset
                            .checked_add(at)
                            .ok_or(EditError::Preservation)?;
                        let end = start
                            .checked_add(selected.width)
                            .ok_or(EditError::Preservation)?;
                        expected
                            .get_mut(start..end)
                            .ok_or(EditError::Preservation)?
                            .copy_from_slice(&bytes[..selected.width]);
                    }
                    if before.is_some() != value.is_some() {
                        // Accumulate only this bit in expected, never assign an
                        // original whole mask byte over another group's edit.
                        set_bit(&mut expected, output_mask_at, bit, value.is_some())?;
                    }
                }
                _ => return Err(EditError::Preservation),
            }
            references.push(record.reference);
        }
    }
    let changed_rows = unique(&mut references, budget, cancelled)?;
    Ok((expected, changed_rows))
}

fn select<'a>(
    original: &'a Source,
    group: &FixedColumnUpdates<'_>,
    indexed: Option<&dyn IndexTargets>,
    carriers: FixedCarriers,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Bound<'a>, EditError> {
    let schema = original.table.schema();
    name_work(schema.name(), group.table(), budget)?;
    if !schema.name().eq_ignore_ascii_case(group.table()) {
        return Err(EditError::Preservation);
    }
    let mut selected = None;
    for (position, field) in schema.columns().iter().enumerate() {
        check(budget, cancelled)?;
        name_work(field.name(), group.column(), budget)?;
        if field.name().eq_ignore_ascii_case(group.column())
            && selected.replace((position, field)).is_some()
        {
            return Err(EditError::Preservation);
        }
    }
    let (position, field) = selected.ok_or(EditError::Unsupported(WriteBlocker::Column))?;
    let (carrier, width) = match (field.native_type(), field.scalar_type()) {
        (BOOLEAN, Some(ScalarType::Boolean)) => (Carrier::Boolean, 0),
        (INT32, Some(ScalarType::Int32)) => (Carrier::Int32, INT32_BYTES),
        (INT64, Some(ScalarType::Int64)) => (Carrier::Int64, WIDE_BYTES),
        (CURRENCY, Some(ScalarType::Currency)) => (Carrier::Currency, WIDE_BYTES),
        (DATE_TIME, Some(ScalarType::DateTime)) => (Carrier::DateTime, WIDE_BYTES),
        (SINGLE, Some(ScalarType::Single)) if carriers != FixedCarriers::Legacy => {
            (Carrier::Single, size_of::<u32>())
        }
        (DOUBLE, Some(ScalarType::Double)) if carriers != FixedCarriers::Legacy => {
            (Carrier::Double, WIDE_BYTES)
        }
        (BYTE, Some(ScalarType::Byte))
            if (carriers == FixedCarriers::Scalar || carriers.includes_guid()) =>
        {
            (Carrier::Byte, size_of::<u8>())
        }
        (INT16, Some(ScalarType::Int16))
            if (carriers == FixedCarriers::Scalar || carriers.includes_guid()) =>
        {
            (Carrier::Int16, size_of::<i16>())
        }
        (GUID, Some(ScalarType::Guid)) if carriers.includes_guid() => (Carrier::Guid, GUID_BYTES),
        _ => return Err(EditError::Unsupported(WriteBlocker::Column)),
    };
    if !field.fixed()
        || field.storage_length() != width
        || field.extended_flags() != 0
        || field.flags() != (FIXED_FLAG | UPDATABLE_FLAG)
    {
        return Err(EditError::Unsupported(WriteBlocker::Column));
    }
    if width != 0 {
        let end = field
            .field
            .offset
            .checked_add(width)
            .ok_or(EditError::Preservation)?;
        for other in schema.columns() {
            check(budget, cancelled)?;
            if other.number() != field.number() && other.fixed() && other.storage_length() != 0 {
                let other_end = other
                    .field
                    .offset
                    .checked_add(other.storage_length())
                    .ok_or(EditError::Preservation)?;
                if other.field.offset < end && other_end > field.field.offset {
                    return Err(EditError::Unsupported(WriteBlocker::Column));
                }
            }
        }
    }
    for dependency in &original.dependencies {
        check(budget, cancelled)?;
        if *dependency == field.number() {
            return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
        }
    }
    for index in original.indexes.indexes() {
        for key in index.columns() {
            check(budget, cancelled)?;
            if key.number() == field.number() {
                let Some(indexed) = indexed else {
                    return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
                };
                indexed.require_target(schema, position, budget, cancelled)?;
            }
        }
    }
    Ok(Bound {
        field,
        carrier,
        width,
    })
}

fn encode<const N: usize>(carrier: Carrier, value: &Value) -> Result<Cell<N>, EditError> {
    let mut bytes = [0; N];
    match (carrier, value) {
        (Carrier::Boolean, Value::Boolean(value)) => return Ok(Cell::Boolean(*value)),
        (
            Carrier::Int32
            | Carrier::Int64
            | Carrier::Currency
            | Carrier::DateTime
            | Carrier::Single
            | Carrier::Double
            | Carrier::Byte
            | Carrier::Int16
            | Carrier::Guid,
            Value::Null,
        ) => return Ok(Cell::Fixed(None)),
        (Carrier::Byte, Value::Byte(value)) => {
            *bytes.first_mut().ok_or(EditError::Preservation)? = *value;
        }
        (Carrier::Int16, Value::Int16(value)) => {
            bytes
                .get_mut(..size_of::<i16>())
                .ok_or(EditError::Preservation)?
                .copy_from_slice(&value.to_le_bytes());
        }
        (Carrier::Int32, Value::Int32(value)) => {
            bytes
                .get_mut(..INT32_BYTES)
                .ok_or(EditError::Preservation)?
                .copy_from_slice(&value.to_le_bytes());
        }
        (Carrier::Single, Value::Single(value)) => {
            bytes
                .get_mut(..size_of::<u32>())
                .ok_or(EditError::Preservation)?
                .copy_from_slice(&value.to_bits().to_le_bytes());
        }
        (Carrier::Double, Value::Double(value)) => {
            bytes
                .get_mut(..WIDE_BYTES)
                .ok_or(EditError::Preservation)?
                .copy_from_slice(&value.to_bits().to_le_bytes());
        }
        (Carrier::Int64, Value::Int64(value)) => {
            bytes
                .get_mut(..WIDE_BYTES)
                .ok_or(EditError::Preservation)?
                .copy_from_slice(&value.to_le_bytes());
        }
        (Carrier::Currency, Value::Currency(value)) => {
            bytes
                .get_mut(..WIDE_BYTES)
                .ok_or(EditError::Preservation)?
                .copy_from_slice(&value.scaled().to_le_bytes());
        }
        (Carrier::DateTime, Value::DateTime(value)) => {
            bytes
                .get_mut(..WIDE_BYTES)
                .ok_or(EditError::Preservation)?
                .copy_from_slice(&value.to_bits().to_le_bytes());
        }
        (Carrier::Guid, Value::Guid(value)) => {
            // Independent native DATA layout, not scalar_encoding or wire bytes.
            let canonical = value.to_bytes();
            let native = GUID_DATA_BYTE_ORDER.map(|at| canonical[at]);
            bytes
                .get_mut(..GUID_BYTES)
                .ok_or(EditError::Preservation)?
                .copy_from_slice(&native);
        }
        _ => return Err(EditError::Unsupported(WriteBlocker::Column)),
    }
    Ok(Cell::Fixed(Some(bytes)))
}

fn set_bit(expected: &mut [u8], at: usize, bit: u8, value: bool) -> Result<(), EditError> {
    let byte = expected.get_mut(at).ok_or(EditError::Preservation)?;
    if value {
        *byte |= bit;
    } else {
        *byte &= !bit;
    }
    Ok(())
}
