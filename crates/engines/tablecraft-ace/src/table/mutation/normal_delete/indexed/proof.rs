//! Independent declining populations; no staged keys or index patches.
use super::{Admission, references, vector};
use crate::{
    EditError, InspectError, NativeColumn, NativeImage, NativeIndex, RowDelete, TableSchema,
    format::{
        data_type::{CURRENCY, INT64, SHORT_BINARY},
        page::PAGE_BYTES,
        row::{COLUMN_COUNT_BYTES, MASK_BITS},
    },
    header::check,
    table::{
        indexes::{
            header::IndexHeader,
            mutation::retirement::{RetiredPage, RetirementTable, audit_global, audit_retirements},
            page::ENTRY_START,
        },
        mutation::preserved_source::Source,
        record_layout,
    },
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};

const ASC_NULL: u8 = 0;
const ASC_VALUE: u8 = 0x7f;
const DESC_NULL: u8 = 0xff;
const DESC_VALUE: u8 = 0x80;
const SIGN_BIT: u32 = 1 << (u32::BITS - 1);
const ROW_BYTES: usize = size_of::<u32>();
const MAX_ENTRY_BYTES: usize = size_of::<u8>() + size_of::<i32>() + ROW_BYTES;
const FREE_OFFSET: usize = std::mem::offset_of!(IndexHeader, free);
const PREFIX_OFFSET: usize = std::mem::offset_of!(IndexHeader, prefix);
const MASK_START: usize = size_of::<IndexHeader>();
const MASK_BYTES: usize = ENTRY_START - MASK_START;

#[derive(Clone, Copy)]
struct Key {
    bytes: [u8; MAX_ENTRY_BYTES],
    length: usize,
    reference: u32,
}

impl Key {
    fn wire(&self) -> &[u8] {
        &self.bytes[..self.length]
    }
}

fn value(
    raw: &[u8],
    schema: &TableSchema,
    field: &NativeColumn,
    observed: &Value,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Option<i32>, EditError> {
    check(budget, cancelled)?;
    let layout = record_layout::parse(raw, schema, budget)?;
    let mask = layout
        .mask_start
        .checked_add(field.number() / MASK_BITS)
        .ok_or(EditError::Preservation)?;
    if field.number() >= layout.count {
        return Err(EditError::Preservation);
    }
    let present =
        raw.get(mask).ok_or(EditError::Preservation)? & (1 << (field.number() % MASK_BITS)) != 0;
    if !present {
        if observed != &Value::Null {
            return Err(EditError::Preservation);
        }
        return Ok(None);
    }
    let start = COLUMN_COUNT_BYTES
        .checked_add(field.field.offset)
        .ok_or(EditError::Preservation)?;
    let end = start
        .checked_add(size_of::<i32>())
        .ok_or(EditError::Preservation)?;
    if end > layout.mask_start {
        return Err(EditError::Preservation);
    }
    budget
        .charge(Resource::Steps, size_of::<i32>() as u64)
        .map_err(InspectError::from)?;
    let value = i32::from_le_bytes(
        raw.get(start..end)
            .ok_or(EditError::Preservation)?
            .try_into()
            .map_err(|_| EditError::Preservation)?,
    );
    if observed != &Value::Int32(value) {
        return Err(EditError::Preservation);
    }
    Ok(Some(value))
}

fn keys(
    source: &Source,
    index: &NativeIndex,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<Key>, EditError> {
    let [column] = index.columns() else {
        return Err(EditError::Preservation);
    };
    let schema = source.table.schema();
    budget
        .charge(Resource::Steps, schema.columns().len() as u64)
        .map_err(InspectError::from)?;
    let position = schema
        .columns()
        .iter()
        .position(|field| field.number() == column.number())
        .ok_or(EditError::Preservation)?;
    let field = &schema.columns()[position];
    let mut result = vector(source.records.len(), budget, cancelled)?;
    for (record, row) in source.records.iter().zip(source.table.rows()) {
        check(budget, cancelled)?;
        let value = value(
            &record.data,
            schema,
            field,
            row.get(position).ok_or(EditError::Preservation)?,
            budget,
            cancelled,
        )?;
        let mut encoded = Key {
            bytes: [0; MAX_ENTRY_BYTES],
            length: 0,
            reference: record.reference,
        };
        if let Some(value) = value {
            encoded.bytes[0] = if column.descending() {
                DESC_VALUE
            } else {
                ASC_VALUE
            };
            let mut bytes = ((value as u32) ^ SIGN_BIT).to_be_bytes();
            if column.descending() {
                for byte in &mut bytes {
                    *byte = !*byte;
                }
            }
            encoded.bytes[1..1 + size_of::<i32>()].copy_from_slice(&bytes);
            encoded.length = size_of::<u8>() + size_of::<i32>();
        } else {
            encoded.bytes[0] = if column.descending() {
                DESC_NULL
            } else {
                ASC_NULL
            };
            encoded.length = size_of::<u8>();
        }
        let end = encoded.length + ROW_BYTES;
        encoded.bytes[encoded.length..end].copy_from_slice(&record.reference.to_be_bytes());
        encoded.length = end;
        budget
            .charge(Resource::Steps, encoded.length as u64)
            .map_err(InspectError::from)?;
        result.push(encoded);
    }
    let work = result
        .len()
        .checked_mul(result.len().checked_ilog2().unwrap_or(0) as usize + 1)
        .and_then(|count| count.checked_mul(MAX_ENTRY_BYTES))
        .ok_or(InspectError::StructureLimit)?;
    budget
        .charge(Resource::Steps, work as u64)
        .map_err(InspectError::from)?;
    result.sort_unstable_by(|left, right| left.wire().cmp(right.wire()));
    check(budget, cancelled)?;
    Ok(result)
}

fn original(
    raw: &[u8],
    keys: &[Key],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    let header = IndexHeader::read(raw)?;
    let prefix = usize::from(header.prefix.get());
    let mut first = [0; MAX_ENTRY_BYTES];
    let mut first_length = 0;
    let mut last = 0;
    let mut position = 0;
    for (block, byte) in raw[MASK_START..ENTRY_START].iter().enumerate() {
        for bit in 0..u8::BITS as usize {
            check(budget, cancelled)?;
            if byte & (1 << bit) == 0 {
                continue;
            }
            let end = block * u8::BITS as usize + bit;
            if end <= last || end > PAGE_BYTES - ENTRY_START {
                return Err(EditError::Preservation);
            }
            let shared = if position == 0 { 0 } else { prefix };
            let length = shared
                .checked_add(end - last)
                .ok_or(EditError::Preservation)?;
            if length > MAX_ENTRY_BYTES || (shared != 0 && shared > first_length) {
                return Err(EditError::Preservation);
            }
            let mut decoded = [0; MAX_ENTRY_BYTES];
            decoded[..shared].copy_from_slice(&first[..shared]);
            decoded[shared..length].copy_from_slice(&raw[ENTRY_START + last..ENTRY_START + end]);
            budget
                .charge(Resource::Steps, length as u64)
                .map_err(InspectError::from)?;
            if keys
                .get(position)
                .is_none_or(|key| key.wire() != &decoded[..length])
            {
                return Err(EditError::Preservation);
            }
            if position == 0 {
                first = decoded;
                first_length = length;
            }
            last = end;
            position += 1;
        }
    }
    if position != keys.len() || usize::from(header.free.get()) != PAGE_BYTES - ENTRY_START - last {
        return Err(EditError::Preservation);
    }
    Ok(())
}

fn write_leaf(
    before: &[u8],
    expected: &mut [u8],
    keys: &[Key],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    let original_prefix = usize::from(IndexHeader::read(before)?.prefix.get());
    let mut common = keys.first().map_or(0, |key| key.length);
    for key in keys.iter().skip(1) {
        check(budget, cancelled)?;
        common = common.min(key.length);
        budget
            .charge(Resource::Steps, common as u64)
            .map_err(InspectError::from)?;
        common = keys[0].wire()[..common]
            .iter()
            .zip(&key.wire()[..common])
            .take_while(|(left, right)| left == right)
            .count();
    }
    let prefix = original_prefix.min(common);
    let mut bitmap = [0_u8; MASK_BYTES];
    let mut end = 0_usize;
    for (position, key) in keys.iter().enumerate() {
        check(budget, cancelled)?;
        let shared = if position == 0 { 0 } else { prefix };
        let width = key
            .length
            .checked_sub(shared)
            .ok_or(EditError::Preservation)?;
        let next = end
            .checked_add(width)
            .filter(|next| *next <= PAGE_BYTES - ENTRY_START)
            .ok_or(EditError::Preservation)?;
        budget
            .charge(Resource::Steps, width as u64)
            .map_err(InspectError::from)?;
        expected[ENTRY_START + end..ENTRY_START + next].copy_from_slice(&key.wire()[shared..]);
        bitmap[next / MASK_BITS] |= 1 << (next % MASK_BITS);
        end = next;
    }
    budget
        .charge(Resource::Steps, MASK_BYTES as u64)
        .map_err(InspectError::from)?;
    expected[MASK_START..ENTRY_START].copy_from_slice(&bitmap);
    let free =
        u16::try_from(PAGE_BYTES - ENTRY_START - end).map_err(|_| EditError::Preservation)?;
    let prefix = u16::try_from(prefix).map_err(|_| EditError::Preservation)?;
    expected[FREE_OFFSET..FREE_OFFSET + size_of::<u16>()].copy_from_slice(&free.to_le_bytes());
    expected[PREFIX_OFFSET..PREFIX_OFFSET + size_of::<u16>()]
        .copy_from_slice(&prefix.to_le_bytes());
    check(budget, cancelled)?;
    Ok(())
}

pub(in crate::table::mutation::normal_delete) fn reconstruct(
    image: &NativeImage,
    source: &Source,
    deletes: &[RowDelete],
    expected: &mut [u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    let _span = tracing::debug_span!("native.ordinary.delete.index_proof").entered();
    let admission = Admission::capture(image, source, budget, cancelled)?;
    let removed = references(source, deletes, budget, cancelled)?;
    let checked = admission.facts.checked();
    let mut retired: Vec<RetiredPage> = Vec::new();
    for (position, checked_index) in checked.indexes().iter().enumerate() {
        check(budget, cancelled)?;
        let index = checked_index.index();
        budget
            .charge(Resource::Steps, position as u64)
            .map_err(InspectError::from)?;
        if checked.indexes()[..position]
            .iter()
            .any(|prior| prior.index().data_number() == index.data_number())
        {
            continue;
        }
        budget
            .charge(
                Resource::Steps,
                source.table.schema().columns().len() as u64,
            )
            .map_err(InspectError::from)?;
        let field = index.columns().first().and_then(|key| {
            source
                .table
                .schema()
                .columns()
                .iter()
                .find(|field| field.number() == key.number())
        });
        if field.is_some_and(|field| field.native_type() == SHORT_BINARY) {
            super::binary::reconstruct(
                image, source, index, &removed, expected, budget, cancelled,
            )?;
            continue;
        }
        let integral = field.is_some_and(|field| matches!(field.native_type(), INT64 | CURRENCY));
        if integral {
            super::integral::reconstruct(
                image, source, index, &removed, expected, budget, cancelled,
            )?;
            continue;
        }
        let keys = keys(source, index, budget, cancelled)?;
        let start = (index.root_page() as usize)
            .checked_mul(PAGE_BYTES)
            .ok_or(InspectError::StructureLimit)?;
        let end = start
            .checked_add(PAGE_BYTES)
            .ok_or(InspectError::StructureLimit)?;
        let before = image
            .as_bytes()
            .get(start..end)
            .ok_or(EditError::Preservation)?;
        if before[0] == crate::table::indexes::page::NODE {
            let tree =
                admission
                    .ownership
                    .require(image, checked.schema(), index, budget, cancelled)?;
            let mut population = vector(keys.len(), budget, cancelled)?;
            for key in &keys {
                check(budget, cancelled)?;
                let mut wire = vector(key.length, budget, cancelled)?;
                budget
                    .charge(Resource::Steps, key.length as u64)
                    .map_err(InspectError::from)?;
                wire.extend_from_slice(key.wire());
                population.push(wire);
            }
            if tree.retirement_required(&population, &removed, budget, cancelled)? {
                let pages = tree.reconstruct_retirements(
                    image,
                    &population,
                    &removed,
                    expected,
                    budget,
                    cancelled,
                )?;
                budget
                    .charge_allocation(pages.len() as u64, size_of::<RetiredPage>() as u64)
                    .map_err(InspectError::from)?;
                budget
                    .charge(
                        Resource::Steps,
                        retired
                            .len()
                            .checked_mul(size_of::<RetiredPage>())
                            .ok_or(InspectError::StructureLimit)? as u64,
                    )
                    .map_err(InspectError::from)?;
                retired
                    .try_reserve_exact(pages.len())
                    .map_err(|_| InspectError::Allocation)?;
                for number in pages {
                    check(budget, cancelled)?;
                    retired.push(RetiredPage {
                        number,
                        physical: index.data_number(),
                        usage: index.usage_map(),
                        root: index.root_page(),
                    });
                }
            } else {
                tree.reconstruct_deletions(
                    image,
                    &population,
                    &removed,
                    expected,
                    budget,
                    cancelled,
                )?;
            }
            continue;
        }
        original(before, &keys, budget, cancelled)?;
        if removed.is_empty() {
            continue;
        }
        let mut retained = vector(keys.len(), budget, cancelled)?;
        for key in keys {
            check(budget, cancelled)?;
            budget
                .charge(Resource::Steps, removed.len() as u64)
                .map_err(InspectError::from)?;
            if !removed.contains(&key.reference) {
                retained.push(key);
            }
        }
        if retained.len().checked_add(removed.len()) != Some(source.records.len()) {
            return Err(EditError::Preservation);
        }
        let page = expected
            .get_mut(start..end)
            .ok_or(EditError::Preservation)?;
        write_leaf(before, page, &retained, budget, cancelled)?;
    }
    if !retired.is_empty() {
        super::maps::reconstruct(image, checked, &retired, expected, budget, cancelled)?;
        let table = RetirementTable {
            schema: checked.schema(),
            indexes: checked.indexes(),
            retired: &retired,
        };
        audit_global(image.as_bytes(), expected, &[table], budget, cancelled)?;
        audit_retirements(
            image.as_bytes(),
            expected,
            image.catalog().catalog().scope(),
            checked.schema().name(),
            &retired,
            budget,
            cancelled,
        )?;
    }
    check(budget, cancelled)?;
    Ok(())
}
