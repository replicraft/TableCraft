//! Fresh original rows, independent reservations, header bytes and map deltas.
use crate::{
    EditError, FittedColumnUpdates, InspectError, NativeEditPlan, NativeImage, TableSchema,
    WriteBlocker,
    format::{
        allocation::{GLOBAL_FREE_REFERENCE, MapRole, RepresentedMap},
        bytes::u32_at,
        definition,
        layout::TABLE_FREE_MAP_OFFSET,
        page::PAGE_BYTES,
        reader::Reader,
    },
    header::check,
    table::mutation::{
        normal_fitted::{binding, owned_growth::proof::reconstruct},
        record_placement::{future::Future, proof as placement, space::Space},
        record_roots,
    },
};
use std::io::Cursor;
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_model::CatalogScope;

const DATA_SIGNATURE: [u8; 2] = [1, 1];
const DATA_OWNER_OFFSET: usize = 4;
const DATA_FREE_OFFSET: usize = 2;
const EMPTY_DATA_FREE: u16 = 4082;

pub(super) fn prove(
    plan: &NativeEditPlan,
    scope: CatalogScope,
    table: &str,
    groups: &[FittedColumnUpdates<'_>],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    let _span = tracing::debug_span!("native.ordinary.eof.prove").entered();
    let image = NativeImage::capture(
        &mut Cursor::new(plan.before_bytes()),
        scope,
        budget,
        cancelled,
    )?;
    let source = super::capture(&image, table, budget, cancelled)?;
    let bound = binding::all(&source, groups, budget, cancelled)?;
    let captured = record_roots::capture(
        &image,
        source.table.schema(),
        &bound.rows,
        budget,
        cancelled,
    )?;
    let space = Space::capture_normal(
        &image,
        source.table.schema(),
        &captured.owned,
        budget,
        cancelled,
    )?;
    let mut records = binding::vector(captured.selected.len(), budget)?;
    for root in &captured.selected {
        check(budget, cancelled)?;
        let original = source
            .records
            .get(root.update.ordinal)
            .ok_or(EditError::Preservation)?;
        if original.offset != root.final_record.offset
            || !binding::equal_bytes(&original.data, &root.final_record.data, budget, cancelled)?
        {
            return Err(EditError::Preservation);
        }
        if let Some(row) = reconstruct(
            &image,
            &source,
            &bound,
            root.update,
            groups,
            budget,
            cancelled,
        )? {
            records.push((root, row));
        }
    }
    let mut owned =
        placement::Placement::capture(&image, &space, &captured.owned, budget, cancelled)?;
    let mut exhausted = false;
    for (root, row) in &records {
        if owned.try_reserve(root, row, budget, cancelled)?.is_none() {
            exhausted = true;
            break;
        }
    }
    if !exhausted {
        return Err(EditError::Unsupported(WriteBlocker::RowLayout));
    }
    // This is fresh allocation qualification, not the producer's token or
    // chosen map edits. The expected map bytes are independently read below.
    let future = Future::capture(&image, source.table.schema(), budget, cancelled)?;
    let mut choices =
        placement::Placement::capture(&image, &space, &captured.owned, budget, cancelled)?;
    choices.include_eof(future.number(), budget, cancelled)?;
    let mut operations = binding::vector(records.len(), budget)?;
    let mut used = false;
    for (root, row) in records {
        let choice = choices.reserve(root, &row, budget, cancelled)?;
        used |= choice.page_number() == future.number();
        operations.push((root, row, choice));
    }
    if !used || operations.len() != plan.changed_rows() {
        return Err(EditError::Preservation);
    }
    let before_len = image.as_bytes().len();
    let length = before_len
        .checked_add(PAGE_BYTES)
        .ok_or(InspectError::StructureLimit)?;
    if plan.after_bytes().len() != length {
        return Err(EditError::Preservation);
    }
    budget
        .charge_allocation(length as u64, 1)
        .map_err(InspectError::from)?;
    budget
        .charge(Resource::Steps, length as u64)
        .map_err(InspectError::from)?;
    let mut expected = Vec::new();
    expected
        .try_reserve_exact(length)
        .map_err(|_| InspectError::Allocation)?;
    expected.extend_from_slice(image.as_bytes());
    expected.resize(length, 0);
    expected[before_len..before_len + DATA_SIGNATURE.len()].copy_from_slice(&DATA_SIGNATURE);
    expected[before_len + DATA_FREE_OFFSET..before_len + DATA_FREE_OFFSET + size_of::<u16>()]
        .copy_from_slice(&EMPTY_DATA_FREE.to_le_bytes());
    expected[before_len + DATA_OWNER_OFFSET..before_len + DATA_OWNER_OFFSET + size_of::<u32>()]
        .copy_from_slice(&source.table.schema().definition_page().to_le_bytes());
    for (root, row, choice) in operations {
        placement::apply_eof(
            &image,
            &space,
            root,
            &row,
            &choice,
            &future,
            &mut expected,
            budget,
            cancelled,
        )?;
    }
    space.audit(image.as_bytes(), &expected[..before_len], budget, cancelled)?;
    maps(
        &image,
        source.table.schema(),
        future.number(),
        &mut expected,
        budget,
        cancelled,
    )?;
    if !binding::equal_bytes(&expected, plan.after_bytes(), budget, cancelled)? {
        return Err(EditError::Preservation);
    }
    super::validate_after(&image, table, &source, &expected, budget, cancelled)?;
    check(budget, cancelled)?;
    Ok(())
}

fn maps(
    image: &NativeImage,
    schema: &TableSchema,
    number: u32,
    expected: &mut [u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    let mut source = image.reader();
    let mut reader = Reader {
        source: &mut source,
        header: image.catalog().header(),
        budget,
        cancelled,
        reads: 0,
    };
    let raw = definition::read(&mut reader, schema.definition_page())?;
    let references = [
        GLOBAL_FREE_REFERENCE,
        schema.layout.usage,
        u32_at(&raw, TABLE_FREE_MAP_OFFSET)?,
    ];
    let roles = [MapRole::GlobalFree, MapRole::Owned, MapRole::TableFreeSpace];
    for (ordinal, (reference, role)) in references.into_iter().zip(roles).enumerate() {
        reader.tick()?;
        let map = RepresentedMap::read(&mut reader, reference, role)?;
        let cell = map.future_cell(&mut reader, number)?;
        if cell.mask.count_ones() != 1 || (cell.before & cell.mask != 0) != (ordinal == 0) {
            return Err(EditError::Preservation);
        }
        let before = image
            .as_bytes()
            .get(cell.offset)
            .ok_or(EditError::Preservation)?;
        let after = expected
            .get_mut(cell.offset)
            .ok_or(EditError::Preservation)?;
        if *before != cell.before || *after != cell.before {
            return Err(EditError::Preservation);
        }
        *after = cell.before ^ cell.mask;
    }
    Ok(())
}
