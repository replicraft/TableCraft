//! Original requested Text values and bounded protected DATA placement.
use super::super::super::{name_work, source::Source};
use crate::{
    EditError, InspectError, NativeColumn, NativeImage, SlotScalarUpdate, WriteBlocker,
    format::{page::PAGE_BYTES, reader::Reader, row::reference_page},
    header::check,
    table::{
        mutation::{
            page_update::{record, roots, stage::Placement},
            slot::storage,
            variable_split::metadata::Metadata,
            variable_update::placement::Plan,
        },
        record as native_record,
    },
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};

pub(super) fn prepare(
    image: &NativeImage,
    original: &Source,
    column: &str,
    updates: &[SlotScalarUpdate],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(Plan, usize), EditError> {
    check(budget, cancelled)?;
    if updates.len() > NativeImage::MAX_FIXED_BATCH_ASSIGNMENTS {
        return Err(InspectError::StructureLimit.into());
    }
    let schema = original.table.schema();
    let mut selected = None;
    for (position, field) in schema.columns().iter().enumerate() {
        check(budget, cancelled)?;
        name_work(field.name(), column, budget)?;
        if field.name().eq_ignore_ascii_case(column) && selected.replace(position).is_some() {
            return Err(EditError::Unsupported(WriteBlocker::Column));
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
    for update in updates {
        validate(field, update.expected(), budget, cancelled)?;
        validate(field, update.value(), budget, cancelled)?;
    }

    // The coordinator captured Source from this same immutable image. Placement
    // reacquires actual root words/ranges, unique terminal ownership and selected
    // original ordinals; source projection caches never supply those facts.
    let captured = roots::capture(image, schema, updates, budget, cancelled)?;
    let metadata = Metadata::capture_for_preserved_boolean(
        image,
        schema,
        original.indexes.indexes(),
        budget,
        cancelled,
    )?;
    let mut plan = Plan::capture_protected_represented(image, schema, metadata, budget, cancelled)?;
    let space = plan.capture_space(image, schema, &captured.owned, budget, cancelled)?;
    let mut source = image.reader();
    let mut reader = Reader {
        source: &mut source,
        header: image.catalog().header(),
        budget,
        cancelled,
        reads: 0,
    };
    let mut changed = 0_usize;

    // roots::capture returns native live-row order, independently of caller
    // request order. The independent replay derives this same original ordering.
    for root in captured.selected {
        reader.tick()?;
        let root_page = reference_page(root.reference);
        let terminal_page = u32::try_from(root.final_record.offset / PAGE_BYTES)
            .map_err(|_| InspectError::Malformed)?;
        // No-ops retain full root/terminal role admission as well.
        space.require_page(image, root_page, false, reader.budget, cancelled)?;
        space.require_page(image, terminal_page, false, reader.budget, cancelled)?;
        let cells = native_record::decode_selected(
            &root.final_record.data,
            schema,
            Some(&[selected]),
            &mut reader,
        )?;
        let [actual] = cells.as_slice() else {
            return Err(EditError::Preservation);
        };
        validate(field, actual, reader.budget, cancelled)?;
        if !equal(actual, root.update.expected(), reader.budget, cancelled)? {
            return Err(EditError::Conflict);
        }
        if equal(actual, root.update.value(), reader.budget, cancelled)? {
            // Preserve original compression and inactive NULL payload/offsets.
            continue;
        }

        let replacement = record::build(
            schema,
            selected,
            &root.final_record.data,
            root.update.value(),
            reader.budget,
            cancelled,
        )?;
        plan.apply(image, &root, &replacement, &space, reader.budget, cancelled)?;
        changed = changed.checked_add(1).ok_or(InspectError::StructureLimit)?;
    }
    plan.audit(reader.budget, cancelled)?;
    reader.tick()?;
    Ok((plan, changed))
}

fn validate(
    field: &NativeColumn,
    value: &Value,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    check(budget, cancelled)?;
    if !matches!(value, Value::Text(_) | Value::Null) {
        return Err(EditError::Unsupported(WriteBlocker::Column));
    }
    budget
        .charge(Resource::Steps, width(value))
        .map_err(InspectError::from)?;
    storage::validate(field, value)
}

fn equal(
    left: &Value,
    right: &Value,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<bool, EditError> {
    check(budget, cancelled)?;
    let work = width(left)
        .checked_add(width(right))
        .ok_or(InspectError::StructureLimit)?;
    budget
        .charge(Resource::Steps, work)
        .map_err(InspectError::from)?;
    Ok(left == right)
}

fn width(value: &Value) -> u64 {
    match value {
        Value::Text(text) => text.len() as u64,
        _ => size_of::<Value>() as u64,
    }
}
