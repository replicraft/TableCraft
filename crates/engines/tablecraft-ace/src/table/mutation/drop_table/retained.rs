//! Retained table identity and dependencies precede any row or catalog effect.
use super::{
    NativeTableDropIdentity, TableDropError,
    source::{Qualified, SourceProfile},
};
use crate::{
    InspectError, NativeImage, NativeTableStorage,
    header::check,
    inspect_table_schema,
    table::{
        mutation::{normal_delete, ordinary_content::schema_dependencies},
        table_append::Inventory,
    },
};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_model::ObjectKind;

pub(super) fn qualify(
    image: &NativeImage,
    requested: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Qualified, TableDropError> {
    check(budget, cancelled)?;
    let source = normal_delete::retained::capture(image, requested, budget, cancelled)?;
    schema_dependencies::require_empty(image, budget, cancelled)?;
    if !source.dependencies.is_empty() {
        return Err(TableDropError::Dependency);
    }

    let schema = source.table.schema();
    let inventory = Inventory::capture(image, budget, cancelled)?;
    inventory.require_source(image)?;
    let catalog = inspect_table_schema(
        &mut image.reader(),
        image.catalog().catalog().scope(),
        "MSysObjects",
        budget,
        cancelled,
    )?;
    if inventory.catalog_definition() != catalog.definition_page()
        || inventory.catalog_live_rows() != catalog.declared_rows()
    {
        return Err(TableDropError::Preservation);
    }

    let mut selected = None;
    for (object, origin) in image
        .catalog()
        .catalog()
        .objects()
        .iter()
        .zip(image.catalog().origins())
    {
        check(budget, cancelled)?;
        if object.id() != schema.object_id() {
            continue;
        }
        budget.charge(
            Resource::Steps,
            object.name().as_str().len().max(schema.name().len()) as u64,
        )?;
        if object.kind() != ObjectKind::Table
            || origin.is_system()
            || origin.table_storage() != Some(NativeTableStorage::Local)
            || object.name().as_str() != schema.name()
            || origin.object_id() != schema.object_id()
            || selected.replace(origin).is_some()
        {
            return Err(TableDropError::Source);
        }
    }
    let origin = selected.ok_or(TableDropError::MissingTable)?;
    if !crate::identifier::valid_name(schema.name(), budget, cancelled)? {
        return Err(TableDropError::Source);
    }
    budget.charge_allocation(schema.name().len() as u64, 1)?;
    let mut table = String::new();
    table
        .try_reserve_exact(schema.name().len())
        .map_err(|_| InspectError::Allocation)?;
    budget.charge(Resource::Steps, schema.name().len() as u64)?;
    table.push_str(schema.name());
    check(budget, cancelled)?;

    Ok(Qualified {
        table,
        identity: NativeTableDropIdentity::from_origin(origin, schema.definition_page()),
        profile: SourceProfile::Retained,
        declared_rows: schema.declared_rows(),
        catalog_definition: catalog.definition_page(),
        catalog_rows: catalog.declared_rows(),
    })
}

#[cfg(test)]
mod tests;
