use super::{
    NativeLinkedTableDefinition,
    fields::{self, Field},
    metadata::OwnedText,
};
use crate::{
    InspectError, NativeReadSource, NativeTableStorage, format::reader::Reader, header::check,
    inspect_catalog, inspect_table_schema, table::decode_projected_record,
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_model::{CatalogScope, ObjectKind};

const CATALOG_TABLE: &str = "MSysObjects";

/// Read one inert linked-table definition from a stable qualified Jet/ACE source.
/// Only its selected catalog fields are decoded; no target path, credentials,
/// provider, saved query, object body or user table records are followed.
/// Missing/unqualified optional Text fields remain visible and block Access-target
/// interpretation. Existing cumulative read/Text/allocation/work/cancel limits
/// apply; a failure returns no partial definition. Inspection does not authorize
/// a later target open and does not alter ordinary linked-query refusal.
pub fn read_linked_table_definition<R: NativeReadSource>(
    source: &mut R,
    scope: CatalogScope,
    name: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<NativeLinkedTableDefinition, InspectError> {
    let _span = tracing::debug_span!("native.linked.definition.read").entered();
    check(budget, cancelled)?;
    let catalog = inspect_catalog(source, scope, budget, cancelled)?;
    let object = catalog
        .catalog()
        .lookup(name, Some(ObjectKind::Table), None)?
        .ok_or(InspectError::UnknownTable)?;
    if object
        .parent()
        .is_some_and(|parent| catalog.catalog().get(parent).is_none())
    {
        return Err(InspectError::Malformed);
    }
    let mut origin = None;
    for candidate in catalog.origins() {
        check(budget, cancelled)?;
        if candidate.object_id() == object.id() {
            if origin.is_some() {
                return Err(InspectError::Malformed);
            }
            origin = Some(candidate);
        }
    }
    let origin = origin.ok_or(InspectError::Malformed)?;
    let storage = match origin.table_storage() {
        Some(storage @ (NativeTableStorage::Linked | NativeTableStorage::LinkedOdbc)) => storage,
        _ => return Err(InspectError::Unsupported),
    };

    let schema = inspect_table_schema(source, scope, CATALOG_TABLE, budget, cancelled)?;
    let fields = fields::select(&schema, budget, cancelled)?;
    budget.charge_allocation(fields.selected.len() as u64, size_of::<usize>() as u64)?;
    let mut positions = Vec::new();
    positions
        .try_reserve_exact(fields.selected.len())
        .map_err(|_| InspectError::Allocation)?;
    positions.extend(fields.selected.iter().map(|(position, _)| *position));
    let mut reader = Reader {
        source,
        header: catalog.header(),
        budget,
        cancelled,
        reads: 0,
    };
    reader.budget.charge(Resource::Rows, 1)?;
    let raw = reader.row(origin.page(), origin.slot(), Some(schema.definition_page()))?;
    let values = decode_projected_record(&raw, &schema, Some(&positions), &mut reader)?;
    if values.len() != fields.selected.len() {
        return Err(InspectError::Malformed);
    }

    reader
        .budget
        .charge(Resource::Bytes, object.name().as_str().len() as u64)?;
    let mut local_name = String::new();
    local_name
        .try_reserve_exact(object.name().as_str().len())
        .map_err(|_| InspectError::Allocation)?;
    local_name.push_str(object.name().as_str());
    let mut result = NativeLinkedTableDefinition {
        object: object.id(),
        native_id: origin.native_id(),
        name: local_name,
        storage,
        database: fields.database,
        foreign_name: fields.foreign_name,
        connect: fields.connect,
    };
    for ((_, field), value) in fields.selected.iter().zip(values) {
        reader.tick()?;
        match field {
            Field::Id if value == Value::Int32(origin.native_id()) => {}
            Field::Parent if value == Value::Int32(origin.parent_id()) => {}
            Field::Flags if value == Value::Int32(origin.flags() as i32) => {}
            Field::Name if matches!(&value, Value::Text(name) if name == object.name().as_str()) => {
            }
            Field::Type if value == Value::Int16(origin.type_code()) => {}
            Field::Id | Field::Parent | Field::Flags | Field::Name | Field::Type => {
                return Err(InspectError::Malformed);
            }
            Field::Database => result.database = text(value)?,
            Field::ForeignName => result.foreign_name = text(value)?,
            Field::Connect => result.connect = text(value)?,
        }
    }
    reader.tick()?;
    Ok(result)
}

fn text(value: Value) -> Result<OwnedText, InspectError> {
    match value {
        Value::Null => Ok(OwnedText::Null),
        Value::Text(value) => Ok(OwnedText::Text(value)),
        _ => Err(InspectError::Malformed),
    }
}
