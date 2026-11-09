//! Fresh original DATA/key reconstruction; no producer patches or encoded keys.
use super::super::{binding, proof as data_proof};
use crate::{
    EditError, FittedColumnUpdates, InspectError, NativeEditPlan, NativeImage, NativeIndex,
    format::{bytes::u16_at, data_type::SHORT_BINARY, page::PAGE_BYTES},
    header::check,
    table::{
        indexes::mutation::normal_index_audit::binary::{
            Key, original_value, requested, sort, wire,
        },
        mutation::{
            fixed_records,
            preserved_source::{Source, validation},
            record_serialization::proof::equal,
        },
    },
};
use std::io::Cursor;
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_model::CatalogScope;

const FREE_AT: usize = 2;
const MASK_AT: usize = 27;
const ACTIVE_AT: usize = 480;
const MASK_BYTES: usize = ACTIVE_AT - MASK_AT;
const BIT_WIDTH: usize = u8::BITS as usize;

pub(in crate::table::mutation::normal_fitted) fn prove(
    plan: &NativeEditPlan,
    scope: CatalogScope,
    table: &str,
    groups: &[FittedColumnUpdates<'_>],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    let image = NativeImage::capture(
        &mut Cursor::new(plan.before_bytes()),
        scope,
        budget,
        cancelled,
    )?;
    let source = super::capture(&image, table, budget, cancelled)?;
    source.require_preserved_definition_tail(&image, plan.after_bytes(), budget, cancelled)?;
    let facts = super::qualify(&image, &source, table, budget, cancelled)?;
    let (mut expected, changed) =
        data_proof::reconstruct_indexed_binary(&image, &source, groups, budget, cancelled)?;
    if changed != plan.changed_rows() {
        return Err(EditError::Preservation);
    }
    for (position, checked) in facts.checked().indexes().iter().enumerate() {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, position as u64)
            .map_err(InspectError::from)?;
        let index = checked.index();
        if facts.checked().indexes()[..position]
            .iter()
            .any(|prior| prior.index().data_number() == index.data_number())
        {
            continue;
        }
        let [key] = index.columns() else {
            return Err(EditError::Preservation);
        };
        let field = source
            .table
            .schema()
            .columns()
            .get(key.number())
            .ok_or(EditError::Preservation)?;
        if field.native_type() != SHORT_BINARY {
            continue;
        }
        reconstruct(
            &image,
            &source,
            index,
            groups,
            &mut expected,
            budget,
            cancelled,
        )?;
    }
    // Recapture the complete final Source: actual rows, native properties,
    // constraints, required/unique keys, aliases and ownership must all agree.
    let after_image = NativeImage::capture(&mut Cursor::new(&expected), scope, budget, cancelled)?;
    let after_source = super::capture(&after_image, table, budget, cancelled)?;
    super::qualify(&after_image, &after_source, table, budget, cancelled)?;
    validation::expected(&image, table, &source.table, &expected, budget, cancelled)?;
    equal(&expected, plan.after_bytes(), budget, cancelled)?;
    check(budget, cancelled)?;
    Ok(())
}

fn reconstruct(
    image: &NativeImage,
    source: &Source,
    index: &NativeIndex,
    groups: &[FittedColumnUpdates<'_>],
    expected: &mut [u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    let [column] = index.columns() else {
        return Err(EditError::Preservation);
    };
    let schema = source.table.schema();
    let field = schema
        .columns()
        .get(column.number())
        .ok_or(EditError::Preservation)?;
    let mut original = binding::vector(source.records.len(), budget)?;
    let mut final_keys = binding::vector(source.records.len(), budget)?;
    for (ordinal, (record, row)) in source.records.iter().zip(source.table.rows()).enumerate() {
        check(budget, cancelled)?;
        let before = row.get(column.number()).ok_or(EditError::Preservation)?;
        let value = original_value(&record.data, schema, field, before, budget, cancelled)?;
        original.push((value, record.reference));
        let mut final_value = before;
        for group in groups {
            check(budget, cancelled)?;
            fixed_records::name_work(group.name(), field.name(), budget)?;
            if !group.name().eq_ignore_ascii_case(field.name()) {
                continue;
            }
            let FittedColumnUpdates::Variable { updates, .. } = group else {
                return Err(EditError::Preservation);
            };
            for update in *updates {
                check(budget, cancelled)?;
                if update.row() == ordinal {
                    if !binding::equal(before, update.expected(), budget, cancelled)? {
                        return Err(EditError::Conflict);
                    }
                    final_value = update.value();
                }
            }
        }
        final_keys.push((requested(field, final_value)?, record.reference));
    }
    sort(&mut original, column.descending(), budget, cancelled)?;
    sort(&mut final_keys, column.descending(), budget, cancelled)?;
    let base = index.root_page() as usize * PAGE_BYTES;
    let before = image
        .as_bytes()
        .get(base..base + PAGE_BYTES)
        .ok_or(EditError::Preservation)?;
    require_original(before, &original, column.descending(), budget, cancelled)?;
    let mut mask = [0; MASK_BYTES];
    let mut active = 0_usize;
    for entry in final_keys {
        check(budget, cancelled)?;
        let (bytes, width) = wire(entry, column.descending())?;
        let next = active
            .checked_add(width)
            .filter(|next| *next <= PAGE_BYTES - ACTIVE_AT)
            .ok_or(EditError::Preservation)?;
        budget
            .charge(Resource::Steps, width as u64)
            .map_err(InspectError::from)?;
        expected
            .get_mut(base + ACTIVE_AT + active..base + ACTIVE_AT + next)
            .ok_or(EditError::Preservation)?
            .copy_from_slice(&bytes[..width]);
        mask[next / BIT_WIDTH] |= 1 << (next % BIT_WIDTH);
        active = next;
    }
    budget
        .charge(Resource::Steps, (MASK_BYTES + size_of::<u16>()) as u64)
        .map_err(InspectError::from)?;
    expected[base + MASK_AT..base + ACTIVE_AT].copy_from_slice(&mask);
    let free =
        u16::try_from(PAGE_BYTES - ACTIVE_AT - active).map_err(|_| EditError::Preservation)?;
    expected[base + FREE_AT..base + FREE_AT + size_of::<u16>()]
        .copy_from_slice(&free.to_le_bytes());
    // All old history bytes and the original inactive tail remain untouched.
    Ok(())
}

fn require_original(
    before: &[u8],
    keys: &[Key<'_>],
    descending: bool,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    let mut mask = [0; MASK_BYTES];
    let mut active = 0_usize;
    for entry in keys {
        check(budget, cancelled)?;
        let (bytes, width) = wire(*entry, descending)?;
        let next = active
            .checked_add(width)
            .filter(|next| *next <= PAGE_BYTES - ACTIVE_AT)
            .ok_or(EditError::Preservation)?;
        budget
            .charge(Resource::Steps, width as u64)
            .map_err(InspectError::from)?;
        if before.get(ACTIVE_AT + active..ACTIVE_AT + next) != Some(&bytes[..width]) {
            return Err(EditError::Preservation);
        }
        mask[next / BIT_WIDTH] |= 1 << (next % BIT_WIDTH);
        active = next;
    }
    budget
        .charge(Resource::Steps, MASK_BYTES as u64)
        .map_err(InspectError::from)?;
    if before[MASK_AT..ACTIVE_AT] != mask
        || usize::from(u16_at(before, FREE_AT)?) != PAGE_BYTES - ACTIVE_AT - active
    {
        return Err(EditError::Preservation);
    }
    check(budget, cancelled)?;
    Ok(())
}
