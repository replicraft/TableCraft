//! Retained schema admission keeps metadata observation distinct from row writes.
use super::{Error, Family, Table, UserStorage, dependencies, graph_error};
use crate::{
    NativeImage,
    format::{layout::CALCULATED_EXT_MASK, reader::Reader},
    header::check,
    inspect_database_properties, inspect_table_properties, inspect_table_schema,
    table::{
        long_value::trace::Kind,
        mutation::{
            catalog_property::{column, descriptor, ledger},
            normal_ordinary, schema_source,
        },
        table_append::Inventory,
    },
};
use tablecraft_core::limits::ResourceBudget;

pub(super) fn observe(
    image: &NativeImage,
    requested: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Table, Error> {
    check(budget, cancelled)?;
    image.require_retained_capture()?;
    if !image.has_retained_table_source(requested, budget, cancelled)? {
        return Err(Error::Source);
    }
    let inventory = Inventory::capture(image, budget, cancelled)?;
    inventory.require_source(image)?;

    // A separate schema-purpose certificate proves programs, complete current
    // declarations, historical record interpretation and physical protection.
    // The whole-file inventory alone cannot authorize any schema change.
    let source = normal_ordinary::capture_retained_schema(image, requested, budget, cancelled)?;
    if !source.dependencies.is_empty()
        || source
            .table
            .schema()
            .columns()
            .iter()
            .any(|column| column.extended_flags() & CALCULATED_EXT_MASK != 0)
    {
        return Err(Error::Source);
    }
    let (schema, _, _) = source.table.into_parts();
    let scope = image.catalog().catalog().scope();
    let properties =
        inspect_table_properties(&mut image.reader(), scope, schema.name(), budget, cancelled)?;
    if properties.object_id() != schema.object_id() {
        return Err(Error::Source);
    }
    dependencies(&properties, &schema, budget, cancelled)?;

    let database_origin =
        schema_source::find_role(image, "MSysDb", budget, cancelled).map_err(graph_error)?;
    let database = inspect_database_properties(&mut image.reader(), scope, budget, cancelled)?
        .ok_or(Error::Source)?;
    if database.object_id() != database_origin.object_id() {
        return Err(Error::Source);
    }
    let catalog =
        inspect_table_schema(&mut image.reader(), scope, "MSysObjects", budget, cancelled)?;
    if catalog.definition_page() != inventory.catalog_definition()
        || catalog.declared_rows() != inventory.catalog_live_rows()
    {
        return Err(Error::Source);
    }

    let mut target = None;
    for origin in image.catalog().origins() {
        check(budget, cancelled)?;
        if origin.object_id() == schema.object_id() && target.replace(origin).is_some() {
            return Err(Error::Source);
        }
    }
    let target = target.ok_or(Error::Source)?;
    let selected = column(&catalog)?;
    let mut input = image.reader();
    let mut reader = Reader {
        source: &mut input,
        header: image.catalog().header(),
        budget,
        cancelled,
        reads: 0,
    };
    let descriptor = descriptor::capture_retained(
        &mut reader,
        &catalog,
        selected,
        target,
        inventory.catalog_pages(),
    )?;
    let kind = descriptor.kind()?;
    let ledger = ledger::capture_retained(image.as_bytes(), image.catalog(), budget, cancelled)?;
    let (database_reference, database_bytes) = ledger
        .single_reference(database_origin.object_id(), budget, cancelled)?
        .ok_or(Error::Source)?;
    if database_bytes != database.raw_bytes().len() {
        return Err(Error::Source);
    }

    // Other tables can retain their own inline or external property histories.
    // The ledger proves their disjoint live ownership; unlike the older source,
    // this observation does not assume that only two property bodies exist.
    let user_storage = match kind {
        Kind::Null if properties.raw_bytes().is_empty() => UserStorage::Absent,
        Kind::Inline if !properties.raw_bytes().is_empty() => UserStorage::Inline,
        Kind::Single => {
            let (reference, bytes) = ledger
                .single_reference(schema.object_id(), budget, cancelled)?
                .ok_or(Error::Source)?;
            if bytes != properties.raw_bytes().len() || reference == database_reference {
                return Err(Error::Source);
            }
            UserStorage::Single { reference }
        }
        _ => return Err(Error::Source),
    };
    check(budget, cancelled)?;
    Ok(Table {
        family: Family::Retained,
        schema,
        properties,
        catalog,
        user_storage,
        database,
        database_reference,
    })
}

#[cfg(test)]
mod tests;
