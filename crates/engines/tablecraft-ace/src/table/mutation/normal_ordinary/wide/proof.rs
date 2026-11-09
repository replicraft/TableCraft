//! Fresh original fixed records and manual signed key/ROW populations.
mod population;
mod tree;
use super::vector;
use crate::{
    EditError, FixedColumnUpdates, InspectError, NativeEditPlan, NativeImage, NativeIndex,
    format::{bytes::u16_at, page::PAGE_BYTES},
    header::check,
    table::{
        indexes::header::IndexHeader,
        mutation::{
            fixed_records::{self, proof as records},
            normal_ordinary::capture,
            preserved_source::{Source, validation},
        },
    },
};
use std::io::Cursor;
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_model::CatalogScope;

const ACTIVE: usize = 480;
const MASK_START: usize = size_of::<IndexHeader>();
const MASK_BYTES: usize = ACTIVE - MASK_START;
const FREE_AT: usize = std::mem::offset_of!(IndexHeader, free);

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
    let population::Population {
        original,
        final_keys,
    } = population::capture(source, selected, groups, budget, cancelled)?;
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
    source.require_preserved_definition_tail(&image, plan.after_bytes(), budget, cancelled)?;
    let facts = super::capture_indexes(&image, &source, budget, cancelled)?;
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
    if facts.retained_tree() {
        tree::reconstruct(
            &image,
            &source,
            &facts,
            groups,
            &mut expected,
            budget,
            cancelled,
        )?;
    } else {
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
    }
    if changed_rows != plan.changed_rows() || expected.len() != plan.after_bytes().len() {
        return Err(EditError::Preservation);
    }
    super::compare(&expected, plan.after_bytes(), budget, cancelled)?;
    validation::expected(&image, table, &source.table, &expected, budget, cancelled)?;
    let after = NativeImage::capture(&mut Cursor::new(&expected), scope, budget, cancelled)?;
    let final_source = capture(&after, table, budget, cancelled)?;
    let final_facts = super::capture_indexes(&after, &final_source, budget, cancelled)?;
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

#[cfg(test)]
mod ordinary_siblings;
