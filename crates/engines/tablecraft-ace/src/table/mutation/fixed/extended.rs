//! Original-source body and allocation protection for explicit extended captures.
use super::FixedScalarUpdate;
use crate::{
    EditError, InspectError, NativeImage, TableCheck, WriteBlocker,
    format::{
        allocation::{self, MapRole, RepresentedMap},
        bytes::u32_at,
        layout::TABLE_FREE_MAP_OFFSET,
        reader::Reader,
    },
    header::check,
    table::mutation::{page_update::roots, variable_split::metadata::Metadata},
};
use tablecraft_core::limits::{Resource, ResourceBudget};

pub(super) fn qualify<'a>(
    image: &NativeImage,
    checked: &TableCheck,
    names: impl Iterator<Item = &'a str>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    if image.fixed_update_limits().is_none() {
        return Ok(());
    }
    protect_source(image, checked, names, budget, cancelled)
}

pub(in crate::table::mutation) fn protect_source<'a>(
    image: &NativeImage,
    checked: &TableCheck,
    names: impl Iterator<Item = &'a str>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    let schema = checked.schema();
    budget
        .charge_allocation(schema.columns().len() as u64, size_of::<usize>() as u64)
        .map_err(InspectError::from)?;
    let mut columns = Vec::new();
    columns
        .try_reserve_exact(schema.columns().len())
        .map_err(|_| InspectError::Allocation)?;
    for name in names {
        let mut selected = None;
        for (position, field) in schema.columns().iter().enumerate() {
            check(budget, cancelled)?;
            budget
                .charge(Resource::Steps, field.name().len().max(name.len()) as u64)
                .map_err(InspectError::from)?;
            if field.name().eq_ignore_ascii_case(name) {
                selected = Some(position);
                break;
            }
        }
        let selected = selected.ok_or(EditError::Unsupported(WriteBlocker::Column))?;
        budget
            .charge(Resource::Steps, columns.len() as u64)
            .map_err(InspectError::from)?;
        if !columns.contains(&selected) {
            columns.push(selected);
        }
    }
    let metadata = Metadata::capture_for_fitted_updates(
        image,
        schema,
        checked.indexes(),
        &columns,
        budget,
        cancelled,
    )?;
    metadata.require_source(image, schema.definition_page())?;
    // Empty requests still census every original logical root and exclusive
    // owned final record. No payload copies or fabricated row ordinals are needed.
    let roots = roots::capture(
        image,
        schema,
        &[] as &[FixedScalarUpdate],
        budget,
        cancelled,
    )?;
    for &page in &roots.owned {
        metadata.require_data_page(page, budget, cancelled)?;
    }

    let mut source = image.reader();
    let mut reader = Reader {
        source: &mut source,
        header: image.catalog().header(),
        budget,
        cancelled,
        reads: 0,
    };
    let global = RepresentedMap::read(
        &mut reader,
        allocation::GLOBAL_FREE_REFERENCE,
        MapRole::GlobalFree,
    )?;
    global.require_allocated_storage(&mut reader, &global)?;
    global.require_allocated(&mut reader, schema.definition_page())?;
    let owned = RepresentedMap::read(&mut reader, schema.layout.usage, MapRole::Owned)?;
    owned.require_allocated_storage(&mut reader, &global)?;
    let definition = reader.page(schema.definition_page())?;
    let hint = u32_at(&definition, TABLE_FREE_MAP_OFFSET)?;
    if hint != 0 {
        let candidate = RepresentedMap::read_storage(&mut reader, hint, MapRole::TableFreeSpace)?;
        for page in candidate.pages {
            global.require_allocated(&mut reader, page)?;
        }
    }
    for &page in &roots.owned {
        reader.tick()?;
        global.require_allocated(&mut reader, page)?;
    }
    for (position, index) in checked.indexes().iter().enumerate() {
        reader.work(position)?;
        if checked.indexes()[..position]
            .iter()
            .any(|prior| prior.index().data_number() == index.index().data_number())
        {
            continue;
        }
        let owned = RepresentedMap::read(&mut reader, index.index().usage_map(), MapRole::Owned)?;
        owned.require_allocated_storage(&mut reader, &global)?;
        for page in allocation::pages(&mut reader, index.index().usage_map())? {
            reader.tick()?;
            global.require_allocated(&mut reader, page)?;
        }
    }
    check(reader.budget, reader.cancelled)?;
    Ok(())
}
