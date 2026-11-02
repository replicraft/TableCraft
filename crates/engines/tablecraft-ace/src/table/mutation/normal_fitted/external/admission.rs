//! Complete native ownership before selecting external reuse, including empty groups.
use super::super::binding::{Bound, Target};
use crate::{
    EditError, InspectError, LongValueCheckStatus, NativeImage, WriteBlocker,
    check_table_long_values,
    format::{
        allocation::{GLOBAL_FREE_REFERENCE, MapRole, RepresentedMap},
        reader::Reader,
    },
    header::check,
    table::mutation::{lval_ownership::Inventory, preserved_source::Source, storage_scope::Scope},
};
use tablecraft_core::limits::ResourceBudget;

pub(super) fn selected(
    image: &NativeImage,
    source: &Source,
    bound: &Bound,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<bool, EditError> {
    if !bound.has_long_targets {
        return Ok(false);
    }
    let schema = source.table.schema();
    let storage = check_table_long_values(
        &mut image.reader(),
        image.catalog().catalog().scope(),
        schema.name(),
        budget,
        cancelled,
    )?;
    if storage.rows() != source.records.len()
        || storage.schema().definition_page() != schema.definition_page()
    {
        return Err(EditError::Preservation);
    }
    let mut external = false;
    for target in &bound.targets {
        check(budget, cancelled)?;
        let Target::Variable { column } = *target else {
            continue;
        };
        let field = &schema.columns()[column];
        if !field.is_ordinary_long_value() {
            continue;
        }
        let mut report = None;
        for candidate in storage.columns() {
            check(budget, cancelled)?;
            if candidate.column_number() == field.number() && report.replace(candidate).is_some() {
                return Err(EditError::Preservation);
            }
        }
        let report = report.ok_or(EditError::Preservation)?;
        if report.status() != LongValueCheckStatus::Checked
            || report.shared_references() != 0
            || report.shared_owned_pages() != 0
        {
            return Err(EditError::Unsupported(WriteBlocker::RowLayout));
        }
        external |= report.owned_pages() != 0
            || report.values().single() != 0
            || report.values().chained() != 0;
    }
    if !external {
        return Ok(false);
    }

    let inventory = Inventory::capture(image, budget, cancelled)?;
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
    for target in &bound.targets {
        reader.tick()?;
        let Target::Variable { column } = *target else {
            continue;
        };
        let field = &schema.columns()[column];
        if !field.is_ordinary_long_value() {
            continue;
        }
        let mut report = None;
        for candidate in storage.columns() {
            reader.tick()?;
            if candidate.column_number() == field.number() {
                report = Some(candidate);
            }
        }
        let references = report
            .and_then(|report| report.maps())
            .ok_or(InspectError::Malformed)?;
        let owned = Scope::capture(
            &mut reader,
            references.owned(),
            references.candidate(),
            MapRole::LongValueCandidate,
            &global,
        )?;
        inventory.require_exclusive(
            schema,
            field.number(),
            &owned.pages,
            reader.budget,
            cancelled,
        )?;
    }
    reader.tick()?;
    Ok(true)
}
