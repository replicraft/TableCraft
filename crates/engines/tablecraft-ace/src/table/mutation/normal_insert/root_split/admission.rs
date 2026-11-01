//! Fresh actual scalar/index coverage, ownership and raw manual-key admission.
use crate::{
    EditError, IndexConstraintViolation, InspectError, NativeImage, RowInsert, WriteBlocker,
    format::{
        layout::{FIXED_FLAG, UPDATABLE_FLAG},
        page::PAGE_BYTES,
    },
    header::check,
    table::{
        indexes::{
            header::IndexHeader,
            mutation::{insertion, normal_index_audit::Ownership},
            page::LEAF,
        },
        mutation::{
            fixed_records::indexed::CheckedIndexes, lval_ownership::Inventory,
            preserved_source::Source,
        },
    },
};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_schema::ScalarType;

const VERSION: u8 = 1;
const MANUAL_FIXED_FLAGS: u8 = FIXED_FLAG | UPDATABLE_FLAG;

pub(super) fn qualify<'image>(
    image: &'image NativeImage,
    source: &Source,
    table: &str,
    requests: &[RowInsert],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(CheckedIndexes<'image>, usize), EditError> {
    check(budget, cancelled)?;
    let facts = CheckedIndexes::capture_actions(image, table, budget, cancelled)?;
    let schema = source.table.schema();
    facts.require_source(image, schema, budget, cancelled)?;
    if schema.layout.physical_indexes != 1 || facts.checked().rows() != source.records.len() {
        return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
    }
    let index = insertion::qualify(facts.checked(), 0, budget, cancelled)?;
    let [key] = index.columns() else {
        return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
    };
    let constraints = index.constraints().ok_or(InspectError::Unsupported)?;
    if !constraints.primary_key()
        || !constraints.unique()
        || !constraints.required()
        || constraints.ignore_nulls()
        || key.descending()
    {
        return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
    }
    let ownership = Ownership::capture_actions(image, facts.checked(), budget, cancelled)?;
    let tree = ownership.require(image, schema, index, budget, cancelled)?;
    let live = Inventory::capture(image, budget, cancelled)?;
    live.require_no_long_value_data_overlap(tree.pages(), budget, cancelled)?;
    let at = index.root_page() as usize * PAGE_BYTES;
    let page = image
        .as_bytes()
        .get(at..at + PAGE_BYTES)
        .ok_or(EditError::Preservation)?;
    let header = IndexHeader::read(page)?;
    if tree.pages() != [index.root_page()]
        || header.kind != LEAF
        || header.version != VERSION
        || !header.qualified_for_update(0)
        || header.owner.get() != schema.definition_page()
        || header.previous.get() != 0
        || header.next.get() != 0
        || header.tail.get() != 0
    {
        return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
    }
    let mut column = None;
    for (position, field) in schema.columns().iter().enumerate() {
        check(budget, cancelled)?;
        if field.number() == key.number() {
            if field.scalar_type() != Some(ScalarType::Int32)
                || field.flags() != MANUAL_FIXED_FLAGS
                || field.extended_flags() != 0
                || !field.fixed()
                || field.storage_length() != size_of::<i32>()
            {
                return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
            }
            column = Some(position);
        }
    }
    let column = column.ok_or(EditError::Preservation)?;
    for row in source
        .table
        .rows()
        .iter()
        .map(Vec::as_slice)
        .chain(requests.iter().map(RowInsert::values))
    {
        check(budget, cancelled)?;
        match row.get(column) {
            Some(Value::Int32(_)) => {}
            Some(Value::Null) => {
                return Err(
                    InspectError::IndexConstraint(IndexConstraintViolation::NullKey).into(),
                );
            }
            _ => return Err(EditError::Unsupported(WriteBlocker::IndexedColumn)),
        }
    }
    check(budget, cancelled)?;
    Ok((facts, column))
}
