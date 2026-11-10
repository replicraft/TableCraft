use super::{NativeTableDropIdentity, TableDropError};
use crate::{
    InspectError, NativeImage, NativeTableStorage, header::check, inspect_table_schema,
    table::mutation::rename_table::source as table_source,
};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_model::ObjectKind;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SourceProfile {
    Inline,
    CreatorExternal,
    Current,
}

pub(super) struct Qualified {
    pub table: String,
    pub identity: NativeTableDropIdentity,
    pub profile: SourceProfile,
    pub declared_rows: u32,
    pub catalog_definition: u32,
    pub catalog_rows: u32,
}

pub(super) fn select_profile(
    image: &NativeImage,
    requested: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<SourceProfile, TableDropError> {
    #[cfg(feature = "ace16-create")]
    if super::current::selected(image, requested, budget, cancelled)? {
        return Ok(SourceProfile::Current);
    }
    #[cfg(not(feature = "ace16-create"))]
    let _ = requested;
    if table_source::database_properties_external(image, budget, cancelled)? {
        Ok(SourceProfile::CreatorExternal)
    } else {
        Ok(SourceProfile::Inline)
    }
}

pub(super) fn qualify(
    image: &NativeImage,
    requested: &str,
    profile: SourceProfile,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Qualified, TableDropError> {
    check(budget, cancelled)?;
    if profile == SourceProfile::Current {
        #[cfg(feature = "ace16-create")]
        return super::current::qualify(image, requested, budget, cancelled);
        #[cfg(not(feature = "ace16-create"))]
        return Err(TableDropError::Source);
    }
    image.require_standard_capture()?;
    let inventory = image.catalog();
    let mut exists = false;
    for (object, origin) in inventory
        .catalog()
        .objects()
        .iter()
        .zip(inventory.origins())
    {
        check(budget, cancelled)?;
        budget
            .charge(
                Resource::Steps,
                object.name().as_str().len().max(requested.len()) as u64,
            )
            .map_err(InspectError::from)?;
        if object.kind() == ObjectKind::Table
            && !origin.is_system()
            && origin.table_storage() == Some(NativeTableStorage::Local)
            && object.name().as_str().eq_ignore_ascii_case(requested)
        {
            exists = true;
            break;
        }
    }
    if !exists {
        return Err(TableDropError::MissingTable);
    }

    let original = table_source::qualify(
        image,
        requested,
        requested,
        match profile {
            SourceProfile::Inline => table_source::SourceProfile::RetainedInt32,
            SourceProfile::CreatorExternal => table_source::SourceProfile::CreatorExternal,
            SourceProfile::Current => return Err(TableDropError::Source),
        },
        budget,
        cancelled,
    )?;
    if !crate::identifier::valid_name(&original.table, budget, cancelled)? {
        return Err(TableDropError::Source);
    }
    let schema = inspect_table_schema(
        &mut image.reader(),
        inventory.catalog().scope(),
        &original.table,
        budget,
        cancelled,
    )?;
    // This logical-DROP source language admits empty tables only. A populated
    // table also needs the actual row/index deletion effects; a
    // catalog tombstone alone must not stand in for that larger operation.
    if schema.declared_rows() != 0 {
        return Err(TableDropError::Source);
    }
    check(budget, cancelled)?;

    Ok(Qualified {
        table: original.table,
        identity: NativeTableDropIdentity::from_source(original.identity),
        profile,
        declared_rows: schema.declared_rows(),
        catalog_definition: original.catalog_schema.definition_page(),
        catalog_rows: original.catalog_schema.declared_rows(),
    })
}
