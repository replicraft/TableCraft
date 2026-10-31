//! Independent joint fitting Text/fixed authority from original requests.
mod fixed;
mod text;

use self::{fixed as fixed_audit, text as text_audit};
use super::super::super::{
    copy, name_work,
    source::{self, Source},
    unique, validation,
};
use crate::{
    EditError, FittedColumnUpdates, InspectError, NativeImage, WriteBlocker,
    format::reader::Reader,
    header::check,
    preservation::COPY_CHUNK_BYTES,
    table::{
        mutation::{rows, slot::storage, variable_update::record_proof},
        record,
    },
};
use tablecraft_core::limits::{Resource, ResourceBudget};

enum Bound<'a> {
    Fixed(fixed_audit::Bound<'a>),
    Text(usize),
}

impl Bound<'_> {
    fn number(&self, original: &Source) -> usize {
        match self {
            Self::Fixed(bound) => bound.field.number(),
            Self::Text(column) => original.table.schema().columns()[*column].number(),
        }
    }
}

pub(in crate::table::mutation::fixed::preserved) struct Observed<'a> {
    pub bytes: &'a [u8],
    pub changed_rows: usize,
}

pub(in crate::table::mutation::fixed::preserved) fn prove(
    image: &NativeImage,
    table: &str,
    groups: &[FittedColumnUpdates<'_>],
    observed: Observed<'_>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    check(budget, cancelled)?;
    image.require_standard_capture()?;
    if groups.len() > NativeImage::MAX_FIXED_BATCH_GROUPS {
        return Err(InspectError::StructureLimit.into());
    }
    if groups.len() < 2 {
        return Err(EditError::Unsupported(WriteBlocker::Column));
    }
    let assignments = groups
        .iter()
        .try_fold(0_usize, |count, group| {
            let length = match group {
                FittedColumnUpdates::Fixed { updates, .. } => updates.len(),
                FittedColumnUpdates::Variable { updates, .. } => updates.len(),
            };
            count.checked_add(length)
        })
        .filter(|count| *count <= NativeImage::MAX_FIXED_BATCH_ASSIGNMENTS)
        .ok_or(InspectError::StructureLimit)?;
    let original = source::capture(image, table, budget, cancelled)?;
    name_work(original.table.schema().name(), table, budget)?;
    if !original.table.schema().name().eq_ignore_ascii_case(table) {
        return Err(EditError::Preservation);
    }
    let bound = bind(&original, groups, budget, cancelled)?;
    let mut expected = copy(image.as_bytes(), budget, cancelled)?;
    if observed.bytes.len() != expected.len() {
        return Err(EditError::Preservation);
    }
    budget
        .charge_allocation(assignments as u64, size_of::<u32>() as u64)
        .map_err(InspectError::from)?;
    let mut changed = Vec::new();
    changed
        .try_reserve_exact(assignments)
        .map_err(|_| InspectError::Allocation)?;
    let schema = original.table.schema();
    let mut input = image.reader();
    let mut reader = Reader {
        source: &mut input,
        header: image.catalog().header(),
        budget,
        cancelled,
        reads: 0,
    };

    // Reconstruct only actual Text changes, from ORIGINAL records. This runs
    // before any fixed overlay, regardless of caller group order.
    for (group, bound) in groups.iter().zip(&bound) {
        let (FittedColumnUpdates::Variable { updates, .. }, Bound::Text(column)) = (group, bound)
        else {
            continue;
        };
        let field = &schema.columns()[*column];
        for request in *updates {
            reader.tick()?;
            text_audit::validate(field, request.expected(), reader.budget, cancelled)?;
            text_audit::validate(field, request.value(), reader.budget, cancelled)?;
            let row = original
                .records
                .get(request.row())
                .ok_or(EditError::Conflict)?;
            let cells = record::decode_selected(&row.data, schema, Some(&[*column]), &mut reader)?;
            let [actual] = cells.as_slice() else {
                return Err(EditError::Preservation);
            };
            text_audit::validate(field, actual, reader.budget, cancelled)?;
            if !text_audit::equal(actual, request.expected(), reader.budget, cancelled)? {
                return Err(EditError::Conflict);
            }
            if text_audit::equal(actual, request.value(), reader.budget, cancelled)? {
                continue;
            }
            let replacement = record_proof::reconstruct_declared(
                schema,
                *column,
                &row.data,
                request.value(),
                reader.budget,
                cancelled,
            )?;
            if replacement.len() != row.data.len() {
                return Err(EditError::Unsupported(WriteBlocker::RowLayout));
            }
            let end = row
                .offset
                .checked_add(replacement.len())
                .ok_or(EditError::Preservation)?;
            reader
                .budget
                .charge(Resource::Steps, replacement.len() as u64)
                .map_err(InspectError::from)?;
            expected
                .get_mut(row.offset..end)
                .ok_or(EditError::Preservation)?
                .copy_from_slice(&replacement);
            changed.push(row.reference);
        }
    }

    // This proof's fixed auditor uses explicit LE encoding and ORIGINAL
    // stale mask/payload checks. It changes individual expected bits after Text
    // reconstruction; it receives no stage binding, cache, payload or patches.
    for (group, bound) in groups.iter().zip(&bound) {
        let (FittedColumnUpdates::Fixed { updates, .. }, Bound::Fixed(selected)) = (group, bound)
        else {
            continue;
        };
        for request in *updates {
            reader.tick()?;
            reader
                .budget
                .charge(Resource::Steps, (selected.width * 2) as u64)
                .map_err(InspectError::from)?;
            if let Some(reference) = fixed_audit::overlay_request(
                &original,
                selected,
                request,
                &mut expected,
                reader.budget,
            )? {
                changed.push(reference);
            }
        }
    }

    // Check the final Text after all shared-mask changes too, including raw
    // Text/NULL noops. Opaque siblings are never fabricated as scalar Values.
    for (group, bound) in groups.iter().zip(&bound) {
        let (FittedColumnUpdates::Variable { updates, .. }, Bound::Text(column)) = (group, bound)
        else {
            continue;
        };
        for request in *updates {
            reader.tick()?;
            let row = original
                .records
                .get(request.row())
                .ok_or(EditError::Conflict)?;
            let end = row
                .offset
                .checked_add(row.data.len())
                .ok_or(EditError::Preservation)?;
            let bytes = expected
                .get(row.offset..end)
                .ok_or(EditError::Preservation)?;
            let cells = record::decode_selected(bytes, schema, Some(&[*column]), &mut reader)?;
            let [actual] = cells.as_slice() else {
                return Err(EditError::Preservation);
            };
            if !text_audit::equal(actual, request.value(), reader.budget, cancelled)? {
                return Err(EditError::Preservation);
            }
        }
    }
    if unique(&mut changed, reader.budget, cancelled)? != observed.changed_rows {
        return Err(EditError::Preservation);
    }
    validation::expected(
        image,
        table,
        &original.table,
        &expected,
        reader.budget,
        cancelled,
    )?;
    for (expected, actual) in expected
        .chunks(COPY_CHUNK_BYTES)
        .zip(observed.bytes.chunks(COPY_CHUNK_BYTES))
    {
        reader.tick()?;
        reader
            .budget
            .charge(Resource::Steps, expected.len() as u64)
            .map_err(InspectError::from)?;
        if expected != actual {
            return Err(EditError::Preservation);
        }
    }
    reader.tick()?;
    Ok(())
}

fn bind<'a>(
    original: &'a Source,
    groups: &[FittedColumnUpdates<'_>],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<Bound<'a>>, EditError> {
    budget
        .charge_allocation(groups.len() as u64, size_of::<Bound<'_>>() as u64)
        .map_err(InspectError::from)?;
    let mut selected: Vec<Bound<'a>> = Vec::new();
    selected
        .try_reserve_exact(groups.len())
        .map_err(|_| InspectError::Allocation)?;
    let mut text_groups = 0_usize;
    for group in groups {
        check(budget, cancelled)?;
        let bound = match group {
            FittedColumnUpdates::Fixed { column, updates } => {
                let bound = fixed_audit::select(original, column, budget, cancelled)?;
                rows::unique(updates, budget, cancelled)?;
                Bound::Fixed(bound)
            }
            FittedColumnUpdates::Variable { column, updates } => {
                text_groups += 1;
                if text_groups != 1 {
                    return Err(EditError::Unsupported(WriteBlocker::Column));
                }
                let column = text_column(original, column, budget, cancelled)?;
                rows::unique(updates, budget, cancelled)?;
                Bound::Text(column)
            }
        };
        budget
            .charge(Resource::Steps, selected.len() as u64)
            .map_err(InspectError::from)?;
        if selected
            .iter()
            .any(|prior| prior.number(original) == bound.number(original))
        {
            return Err(EditError::Conflict);
        }
        selected.push(bound);
    }
    if text_groups != 1 {
        return Err(EditError::Unsupported(WriteBlocker::Column));
    }
    Ok(selected)
}

fn text_column(
    original: &Source,
    name: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<usize, EditError> {
    let schema = original.table.schema();
    let mut selected = None;
    for (position, field) in schema.columns().iter().enumerate() {
        check(budget, cancelled)?;
        name_work(field.name(), name, budget)?;
        if field.name().eq_ignore_ascii_case(name) && selected.replace(position).is_some() {
            return Err(EditError::Preservation);
        }
    }
    let selected = selected.ok_or(EditError::Unsupported(WriteBlocker::Column))?;
    let field = &schema.columns()[selected];
    if !field.is_ordinary_short_text() {
        return Err(EditError::Unsupported(WriteBlocker::Column));
    }
    storage::qualify(field)?;
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
                return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
            }
        }
    }
    Ok(selected)
}
