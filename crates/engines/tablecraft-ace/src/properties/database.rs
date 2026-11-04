//! Inert database property maps, bound to their native catalog containment role.
use super::{PropertyGroup, PropertyGroupKind, decode, inspect::read_property_bytes};
use crate::{InspectError, NativeReadSource, format::data_type, header::check, inspect_catalog};
use tablecraft_core::{
    ids::ObjectId,
    limits::{Resource, ResourceBudget},
};
use tablecraft_model::{CatalogScope, ObjectKind};

const DATABASE_CONTAINER: &str = "Databases";
const DATABASE_OBJECT: &str = "MSysDb";
const CONTAINER_TYPE: i16 = 3;
const DATABASE_TYPE: i16 = 2;
const QUERY_MODE: &str = "ANSI Query Mode";
const DDL_PROPERTY: u8 = 1;

/// Exact qualified query-mode observations. Absence is not a guessed default.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DatabaseQueryMode {
    /// No query-mode property in the inspected unnamed map.
    Absent,
    /// Exact signed LONG zero.
    Ansi89,
    /// Exact signed LONG one.
    Ansi92,
    /// Unqualified property type, flags or numeric value.
    Unknown,
}

/// Original database properties, not table metadata or executable authority.
pub struct DatabaseProperties {
    object: ObjectId,
    raw: Vec<u8>,
    groups: Vec<PropertyGroup>,
}

impl DatabaseProperties {
    /// Caller-scoped MSysDb catalog identity.
    pub const fn object_id(&self) -> ObjectId {
        self.object
    }

    /// Complete original LvProp bytes, including unknown properties.
    pub fn raw_bytes(&self) -> &[u8] {
        &self.raw
    }

    /// Inert decoded maps in native order.
    pub fn groups(&self) -> &[PropertyGroup] {
        &self.groups
    }

    /// Observe only the unnamed default map and exact LONG/DDL 0 or 1 values.
    /// These mappings are reference-supported; they are not a native Access
    /// execution comparison. A missing property never implies ANSI-89.
    pub fn query_mode(
        &self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<DatabaseQueryMode, InspectError> {
        for group in &self.groups {
            check(budget, cancelled)?;
            if group.kind() != PropertyGroupKind::Table || !group.name().is_empty() {
                continue;
            }
            for property in group.properties() {
                check(budget, cancelled)?;
                budget.charge(
                    Resource::Steps,
                    (property.name().len() + QUERY_MODE.len()) as u64,
                )?;
                if !property.name().eq_ignore_ascii_case(QUERY_MODE) {
                    continue;
                }
                if property.native_type() != data_type::INT32 || property.flags() != DDL_PROPERTY {
                    return Ok(DatabaseQueryMode::Unknown);
                }
                budget.charge(Resource::Steps, size_of::<i32>() as u64)?;
                let raw = property
                    .raw_value()
                    .try_into()
                    .map_err(|_| InspectError::Malformed)?;
                return Ok(match i32::from_le_bytes(raw) {
                    0 => DatabaseQueryMode::Ansi89,
                    1 => DatabaseQueryMode::Ansi92,
                    _ => DatabaseQueryMode::Unknown,
                });
            }
        }
        check(budget, cancelled)?;
        Ok(DatabaseQueryMode::Absent)
    }
}

/// Read MSysDb's inert default property map through its exact Databases parent.
/// Missing database metadata returns None, independently of table readability.
/// Ambiguous, orphaned or incorrectly typed system/metadata roles fail explicitly.
/// Ordinary user objects in other roles may share these names. No arbitrary
/// object lookup, table reinterpretation or property execution occurs.
pub fn inspect_database_properties<R: NativeReadSource>(
    source: &mut R,
    scope: CatalogScope,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Option<DatabaseProperties>, InspectError> {
    let _span = tracing::debug_span!("native.database.properties.read").entered();
    let catalog = inspect_catalog(source, scope, budget, cancelled)?;
    let mut container = None;
    let mut database = None;
    for (object, origin) in catalog.catalog().objects().iter().zip(catalog.origins()) {
        check(budget, cancelled)?;
        budget.charge(
            Resource::Steps,
            (2 * object.name().as_str().len() + DATABASE_CONTAINER.len() + DATABASE_OBJECT.len())
                as u64,
        )?;
        if object
            .name()
            .as_str()
            .eq_ignore_ascii_case(DATABASE_CONTAINER)
            && (origin.type_code() == CONTAINER_TYPE || origin.is_system())
        {
            if container.is_some() {
                return Err(InspectError::Malformed);
            }
            if object.kind() != ObjectKind::Container
                || origin.type_code() != CONTAINER_TYPE
                || !origin.is_system()
            {
                return Err(InspectError::Unsupported);
            }
            container = Some(origin.native_id());
        }
        if object.name().as_str().eq_ignore_ascii_case(DATABASE_OBJECT)
            && (origin.type_code() == DATABASE_TYPE || origin.is_system())
        {
            if database.is_some() {
                return Err(InspectError::Malformed);
            }
            if object.kind() != ObjectKind::Unknown
                || origin.type_code() != DATABASE_TYPE
                || !origin.is_system()
            {
                return Err(InspectError::Unsupported);
            }
            database = Some(origin);
        }
    }
    let Some(database) = database else {
        return Ok(None);
    };
    if container != Some(database.parent_id()) {
        return Err(InspectError::Malformed);
    }
    let raw = read_property_bytes(source, scope, database.native_id(), budget, cancelled)?;
    let groups = decode::decode(&raw, budget, cancelled)?;
    Ok(Some(DatabaseProperties {
        object: database.object_id(),
        raw,
        groups,
    }))
}
