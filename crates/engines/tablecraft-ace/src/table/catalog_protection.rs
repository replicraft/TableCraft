//! Shared read-only catalog payload/settings checks; no mutation source constructor.
use crate::{
    DatabaseQueryMode, EditError, InspectError, NativeImage, NativeObjectOrigin, PayloadPresence,
    PropertyGroupKind, format::data_type, header::check, inspect_database_properties,
};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_model::{ObjectDescriptor, ObjectKind};

const NAME_OPTIONS: [&str; 2] = ["Track Name AutoCorrect Info", "Perform Name AutoCorrect"];

fn refused() -> EditError {
    EditError::Unsupported(crate::WriteBlocker::ActiveContent)
}

/// This checks one real catalog descriptor. The caller still owns its closed
/// object-kind/ancestry/source predicate and complete physical qualification.
pub(in crate::table) fn require_object(
    image: &NativeImage,
    object: &ObjectDescriptor,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    let payload = image.inspect_object_payloads(object.id(), budget, cancelled)?;
    if [payload.body(), payload.module(), payload.extra()]
        .into_iter()
        .any(|value| value != PayloadPresence::Absent)
        || (object.kind() == ObjectKind::Table
            && !image
                .inspect_object_children(object.id(), budget, cancelled)?
                .is_empty())
    {
        return Err(refused());
    }
    Ok(())
}

/// Preserve the actual absent-settings cohort or the explicit known OFF
/// settings. A supplied descriptor must match the same image's real metadata.
pub(in crate::table) fn require_database_properties(
    image: &NativeImage,
    database: Option<&NativeObjectOrigin>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    let scope = image.catalog().catalog().scope();
    if let Some(db) = database {
        let properties =
            inspect_database_properties(&mut image.reader(), scope, budget, cancelled)?
                .ok_or_else(refused)?;
        if properties.object_id() != db.object_id() || properties.groups().len() != 1 {
            return Err(refused());
        }
        let group = &properties.groups()[0];
        if group.kind() != PropertyGroupKind::Table
            || !group.name().is_empty()
            || ![NAME_OPTIONS.len(), NAME_OPTIONS.len() + 1].contains(&group.properties().len())
        {
            return Err(refused());
        }
        for name in NAME_OPTIONS {
            let mut found = None;
            for property in group.properties() {
                check(budget, cancelled)?;
                budget
                    .charge(
                        Resource::Steps,
                        property.name().len().max(name.len()) as u64,
                    )
                    .map_err(InspectError::from)?;
                if property.name().eq_ignore_ascii_case(name) && found.replace(property).is_some() {
                    return Err(refused());
                }
            }
            let property = found.ok_or_else(refused)?;
            if property.native_type() != data_type::INT32
                || property.flags() != 1
                || property.raw_value() != 0_i32.to_le_bytes()
            {
                return Err(refused());
            }
        }
        // With both required names uniquely accounted for, the sole optional
        // entry must be the existing owner's exact qualified query-mode property.
        // Unknown settings and other properties grant no inert-source authority.
        if group.properties().len() > NAME_OPTIONS.len()
            && !matches!(
                properties.query_mode(budget, cancelled)?,
                DatabaseQueryMode::Ansi89 | DatabaseQueryMode::Ansi92
            )
        {
            return Err(refused());
        }
    } else if inspect_database_properties(&mut image.reader(), scope, budget, cancelled)?.is_some()
    {
        return Err(refused());
    }
    Ok(())
}
