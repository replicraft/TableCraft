//! Existing retained certificates prove source content, not index-creation effects.
use crate::{
    EditError, NativeImage, WriteBlocker,
    format::{
        data_type::INT32,
        layout::{self, FIXED_FLAG, UPDATABLE_FLAG},
        page::PAGE_BYTES,
        reader::Reader,
    },
    header::check,
    identifier,
    table::{
        indexes::mutation::bulk_long::LongKey,
        mutation::{
            index_creation::{SecondaryIndexCreate, buffer, definition::DefinitionLayout},
            normal_ordinary, preserved_source,
        },
        table_append::Inventory,
    },
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_schema::SchemaName;

pub(super) struct Original {
    pub content: preserved_source::Source,
    pub layout: DefinitionLayout,
    pub name: SchemaName,
    pub native: usize,
    pub native_table: i32,
    pub column_id: u16,
    pub dense: usize,
    pub descending: bool,
    pub index_start: u32,
}

pub(super) fn capture(
    image: &NativeImage,
    table: &str,
    request: &SecondaryIndexCreate<'_>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Original, EditError> {
    check(budget, cancelled)?;
    image.require_retained_capture()?;
    if !image.has_retained_table_source(table, budget, cancelled)? {
        return Err(EditError::Unsupported(WriteBlocker::ConstraintCoverage));
    }
    let inventory = Inventory::capture(image, budget, cancelled)?;
    inventory.require_source(image)?;
    if !identifier::valid_name(request.name, budget, cancelled)? {
        return Err(EditError::Unsupported(WriteBlocker::Column));
    }
    let bytes = buffer::copy(request.name.as_bytes(), budget, cancelled)?;
    let name =
        SchemaName::new(String::from_utf8(bytes).map_err(|_| crate::InspectError::Malformed)?)
            .map_err(crate::InspectError::from)?;
    let content = normal_ordinary::capture_retained_schema(image, table, budget, cancelled)?;
    if !content.dependencies.is_empty() {
        return Err(EditError::Unsupported(WriteBlocker::ConstraintCoverage));
    }
    let schema = content.table.schema();
    if schema.layout.physical_indexes >= crate::limits::MAX_INDEXES
        || schema.layout.logical_indexes >= crate::limits::MAX_INDEXES
    {
        return Err(EditError::Unsupported(WriteBlocker::RowLayout));
    }
    let next_logical = u32::try_from(schema.layout.logical_indexes)
        .map_err(|_| crate::InspectError::StructureLimit)?;
    let mut selected = None;
    for column in schema.columns() {
        check(budget, cancelled)?;
        budget
            .charge(
                Resource::Steps,
                column.name().len().max(request.column.len()) as u64,
            )
            .map_err(crate::InspectError::from)?;
        if column.name().eq_ignore_ascii_case(request.column) && selected.replace(column).is_some()
        {
            return Err(crate::InspectError::Malformed.into());
        }
    }
    let selected = selected.ok_or(crate::InspectError::UnknownColumn)?;
    if selected.native_type() != INT32
        || selected.scalar_type() != Some(tablecraft_schema::ScalarType::Int32)
        || selected.flags() != (FIXED_FLAG | UPDATABLE_FLAG)
        || selected.extended_flags() != 0
        || selected.storage_length() != size_of::<i32>()
    {
        return Err(EditError::Unsupported(WriteBlocker::Column));
    }
    let native = selected.number();
    let mut dense = None;
    for (position, column) in content.table.columns().iter().enumerate() {
        check(budget, cancelled)?;
        budget
            .charge(
                Resource::Steps,
                column.name().len().max(selected.name().len()) as u64,
            )
            .map_err(crate::InspectError::from)?;
        if column.name() == selected.name() && dense.replace(position).is_some() {
            return Err(crate::InspectError::Malformed.into());
        }
    }
    let dense = dense.ok_or(crate::InspectError::Malformed)?;
    for index in content.indexes.indexes() {
        check(budget, cancelled)?;
        budget
            .charge(
                Resource::Steps,
                index.name().len().max(name.as_str().len()) as u64,
            )
            .map_err(crate::InspectError::from)?;
        if index.number() == next_logical {
            // Existing logical identities may have holes; count is usable as a
            // new identity only when the original descriptor set leaves it free.
            return Err(EditError::Unsupported(WriteBlocker::ConstraintCoverage));
        }
        if index.name().eq_ignore_ascii_case(name.as_str()) {
            return Err(EditError::Conflict);
        }
        if index.columns().iter().any(|key| key.number() == native) {
            return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
        }
    }
    let mut native_table = None;
    for origin in image.catalog().origins() {
        check(budget, cancelled)?;
        if origin.object_id() == schema.object_id()
            && native_table.replace(origin.native_id()).is_some()
        {
            return Err(crate::InspectError::Malformed.into());
        }
    }
    let native_table = native_table.ok_or(crate::InspectError::Malformed)?;
    let mut input = image.reader();
    let mut reader = Reader {
        source: &mut input,
        header: image.catalog().header(),
        budget,
        cancelled,
        reads: 0,
    };
    let mut column_id = None;
    layout::read_with_field_names(&mut reader, schema.definition_page(), |number, id, _, _| {
        if number == native && column_id.replace(id).is_some() {
            return Err(crate::InspectError::Malformed);
        }
        Ok(())
    })?;
    let column_id = column_id.ok_or(crate::InspectError::Malformed)?;
    let layout = DefinitionLayout::read(image, schema, budget, cancelled)?;
    let units = name.as_str().encode_utf16().count();
    layout.new_length(
        units
            .checked_mul(size_of::<u16>())
            .ok_or(crate::InspectError::StructureLimit)?,
    )?;
    let index_start = u32::try_from(image.as_bytes().len() / PAGE_BYTES)
        .map_err(|_| crate::InspectError::StructureLimit)?;
    Ok(Original {
        content,
        layout,
        name,
        native,
        native_table,
        column_id,
        dense,
        descending: request.descending,
        index_start,
    })
}

impl Original {
    /// Reconstruct source keys independently on each invocation; the audit does
    /// not consume the population or topology prepared by the producer.
    pub(super) fn keys(
        &self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<LongKey>, EditError> {
        if self.content.records.len() != self.content.table.rows().len() {
            return Err(EditError::Preservation);
        }
        let mut output = buffer::vector(self.content.records.len(), budget, cancelled)?;
        let mut previous = None;
        for (record, row) in self.content.records.iter().zip(self.content.table.rows()) {
            check(budget, cancelled)?;
            if previous.is_some_and(|value| value >= record.reference) {
                return Err(EditError::Unsupported(WriteBlocker::RowLayout));
            }
            previous = Some(record.reference);
            let value = match row.get(self.dense) {
                Some(Value::Null) => None,
                Some(Value::Int32(value)) => Some(i64::from(*value)),
                _ => return Err(EditError::Preservation),
            };
            output.push(LongKey {
                value,
                reference: record.reference,
            });
        }
        Ok(output)
    }
}
