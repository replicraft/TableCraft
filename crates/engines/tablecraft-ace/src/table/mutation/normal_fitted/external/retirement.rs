//! Real column-role retirement; selected refs are freshly derived in each consumer.
pub(super) mod proof;
pub(super) mod stage;

use super::super::binding::{self, Bound};
use crate::{
    EditError, FittedColumnUpdates, InspectError, LongValueCheckStatus, NativeImage, WriteBlocker,
    check_table_long_values,
    format::{
        allocation::{GLOBAL_FREE_REFERENCE, MapRole, RepresentedMap},
        page::{DATA_SIGNATURE, OWNER_OFFSET},
        reader::{Reader, row_count, row_range},
        row::{DELETED_MASK, RELOCATED_MASK, reference, reference_page},
    },
    header::check,
    table::mutation::{
        lval_ownership::{ColumnMapKind, Inventory},
        map_authority::{Owner, RecordIdentity},
        preserved_source::Source,
    },
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};

const OWNER_BYTES: usize = size_of::<u32>();
const LVAL_OWNER: &[u8; OWNER_BYTES] = b"LVAL";

pub(super) struct Retiring {
    pub ordinal: usize,
    pub column: usize,
    pub references: Vec<u32>,
    pub replacement: Value,
}

pub(super) struct Page {
    pub column: usize,
    pub number: u32,
}

pub(super) fn replacement(value: &Value) -> Option<Value> {
    match value {
        Value::Null => Some(Value::Null),
        Value::Binary(bytes) if bytes.is_empty() => Some(Value::Binary(Vec::new())),
        _ => None,
    }
}

pub(super) fn selected(
    groups: &[FittedColumnUpdates<'_>],
    bound: &Bound,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<bool, EditError> {
    let mut selected = false;
    for assignment in &bound.assignments {
        check(budget, cancelled)?;
        let cell = groups[assignment.group]
            .cell(assignment.index)
            .ok_or(EditError::Preservation)?;
        if replacement(cell.value).is_some()
            && !binding::equal(cell.expected, cell.value, budget, cancelled)?
        {
            selected = true;
        }
    }
    Ok(selected)
}

pub(super) fn authorize(
    image: &NativeImage,
    source: &Source,
    values: &[Retiring],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<Page>, EditError> {
    if values.is_empty() {
        return Ok(Vec::new());
    }
    let schema = source.table.schema();
    let mut count = 0_usize;
    for value in values {
        check(budget, cancelled)?;
        count = count
            .checked_add(value.references.len())
            .ok_or(InspectError::StructureLimit)?;
    }
    let mut pages: Vec<Page> = binding::vector(count, budget)?;
    for value in values {
        check(budget, cancelled)?;
        for &reference in &value.references {
            check(budget, cancelled)?;
            let number = reference_page(reference);
            budget
                .charge(Resource::Steps, pages.len() as u64)
                .map_err(InspectError::from)?;
            if let Some(old) = pages.iter().find(|page| page.number == number) {
                if old.column != value.column {
                    return Err(EditError::Preservation);
                }
            } else {
                pages.push(Page {
                    column: value.column,
                    number,
                });
            }
        }
    }
    let inventory = Inventory::capture(image, budget, cancelled)?;
    let storage = check_table_long_values(
        &mut image.reader(),
        image.catalog().catalog().scope(),
        schema.name(),
        budget,
        cancelled,
    )?;
    let mut bytes = image.reader();
    let mut reader = Reader {
        source: &mut bytes,
        header: image.catalog().header(),
        budget,
        cancelled,
        reads: 0,
    };
    let global = RepresentedMap::read(&mut reader, GLOBAL_FREE_REFERENCE, MapRole::GlobalFree)?;
    global.require_allocated_storage(&mut reader, &global)?;
    let global_storage =
        RepresentedMap::read_storage(&mut reader, GLOBAL_FREE_REFERENCE, MapRole::GlobalFree)?;
    inventory.require_global_record_rewrite(
        RecordIdentity {
            owner: Owner::GlobalFree,
            reference: GLOBAL_FREE_REFERENCE,
            range: global_storage.record,
        },
        &mut reader,
    )?;

    for page in &pages {
        reader.tick()?;
        let field = schema
            .columns()
            .get(page.column)
            .ok_or(EditError::Preservation)?;
        let mut report = None;
        for candidate in storage.columns() {
            reader.tick()?;
            if candidate.column_number() == field.number() {
                report = Some(candidate);
            }
        }
        let report = report.ok_or(EditError::Preservation)?;
        if report.status() != LongValueCheckStatus::Checked
            || report.shared_references() != 0
            || report.shared_owned_pages() != 0
        {
            return Err(EditError::Unsupported(WriteBlocker::RowLayout));
        }
        let maps = report.maps().ok_or(EditError::Preservation)?;
        inventory.require_exclusive(
            schema,
            field.number(),
            &[page.number],
            reader.budget,
            cancelled,
        )?;
        for (reference, role, kind) in [
            (maps.owned(), MapRole::Owned, ColumnMapKind::Owned),
            (
                maps.candidate(),
                MapRole::LongValueCandidate,
                ColumnMapKind::Candidate,
            ),
        ] {
            reader.tick()?;
            let represented = RepresentedMap::read(&mut reader, reference, role)?;
            represented.require_allocated_storage(&mut reader, &global)?;
            let original = represented.cell(&mut reader, page.number)?;
            if matches!(role, MapRole::Owned) && original.before & original.mask == 0 {
                return Err(EditError::Preservation);
            }
            let record = RepresentedMap::read_storage(&mut reader, reference, role)?;
            inventory.require_record_rewrite(
                schema,
                field.number(),
                kind,
                (reference, record.record),
                reader.budget,
                cancelled,
            )?;
        }
        global.require_allocated(&mut reader, page.number)?;
        let raw = reader.page(page.number)?;
        if raw[..DATA_SIGNATURE.len()] != DATA_SIGNATURE
            || raw.get(OWNER_OFFSET..OWNER_OFFSET + OWNER_BYTES) != Some(LVAL_OWNER)
        {
            return Err(EditError::Preservation);
        }
        let count = row_count(&raw)?;
        for slot in 0..count {
            reader.tick()?;
            let (flags, _, _) = row_range(&raw, slot)?;
            if flags & (DELETED_MASK | RELOCATED_MASK) != 0 {
                return Err(EditError::Unsupported(WriteBlocker::RowLayout));
            }
            let row = reference(
                page.number,
                u8::try_from(slot).map_err(|_| EditError::Preservation)?,
            );
            let mut selected = 0_usize;
            for value in values {
                reader.tick()?;
                reader.work(value.references.len())?;
                if value.column == page.column {
                    selected += value
                        .references
                        .iter()
                        .filter(|&&candidate| candidate == row)
                        .count();
                }
            }
            if selected != 1 {
                // Includes every unselected live row and every old orphan row.
                return Err(EditError::Unsupported(WriteBlocker::RowLayout));
            }
        }
        if count == 0 {
            return Err(EditError::Preservation);
        }
    }
    reader.tick()?;
    Ok(pages)
}
