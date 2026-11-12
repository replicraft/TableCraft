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
        indexes::mutation::{NumericKey, bulk_numeric::NumericKeyRow},
        mutation::{
            field,
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
use tablecraft_schema::{ScalarType, SchemaName};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Construction {
    OriginalCount,
    Fresh,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Language {
    Long,
    Numeric,
    UniqueLong,
}

#[derive(Clone, Copy)]
pub(super) enum Mode {
    OriginalLong,
    FreshLong,
    Numeric,
    UniqueLong,
}

pub(super) struct Original<'image> {
    pub inventory: Inventory<'image>,
    language: Language,
    pub kind: NumericKey,
    pub scalar: ScalarType,
    pub construction: Construction,
    pub logical: u32,
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

pub(super) fn capture<'image>(
    image: &'image NativeImage,
    table: &str,
    request: &SecondaryIndexCreate<'_>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Original<'image>, EditError> {
    capture_mode(image, table, request, Mode::OriginalLong, budget, cancelled)
}

pub(super) fn capture_fresh<'image>(
    image: &'image NativeImage,
    table: &str,
    request: &SecondaryIndexCreate<'_>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Original<'image>, EditError> {
    capture_mode(image, table, request, Mode::FreshLong, budget, cancelled)
}

pub(super) fn capture_numeric<'image>(
    image: &'image NativeImage,
    table: &str,
    request: &SecondaryIndexCreate<'_>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Original<'image>, EditError> {
    capture_mode(image, table, request, Mode::Numeric, budget, cancelled)
}

pub(super) fn capture_mode<'image>(
    image: &'image NativeImage,
    table: &str,
    request: &SecondaryIndexCreate<'_>,
    mode: Mode,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Original<'image>, EditError> {
    let (construction, language) = match mode {
        Mode::OriginalLong => (Construction::OriginalCount, Language::Long),
        Mode::FreshLong => (Construction::Fresh, Language::Long),
        Mode::Numeric => (Construction::Fresh, Language::Numeric),
        Mode::UniqueLong => (Construction::Fresh, Language::UniqueLong),
    };
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
    if construction == Construction::Fresh && !request.name.is_ascii() {
        return Err(EditError::Unsupported(WriteBlocker::ConstraintCoverage));
    }
    let bytes = buffer::copy(request.name.as_bytes(), budget, cancelled)?;
    let name =
        SchemaName::new(String::from_utf8(bytes).map_err(|_| crate::InspectError::Malformed)?)
            .map_err(crate::InspectError::from)?;
    let content = match language {
        Language::Long | Language::UniqueLong => {
            normal_ordinary::capture_retained_schema(image, table, budget, cancelled)?
        }
        Language::Numeric => normal_ordinary::capture_retained_numeric_index_schema(
            image, table, &inventory, budget, cancelled,
        )?,
    };
    if !content.dependencies.is_empty() {
        return Err(EditError::Unsupported(WriteBlocker::ConstraintCoverage));
    }
    let schema = content.table.schema();
    if schema.layout.physical_indexes >= crate::limits::MAX_INDEXES
        || schema.layout.logical_indexes >= crate::limits::MAX_INDEXES
    {
        return Err(EditError::Unsupported(WriteBlocker::RowLayout));
    }
    let mut next_logical = u32::try_from(schema.layout.logical_indexes)
        .map_err(|_| crate::InspectError::StructureLimit)?;
    let mut selected = None;
    for (position, column) in schema.columns().iter().enumerate() {
        check(budget, cancelled)?;
        budget
            .charge(
                Resource::Steps,
                column.name().len().max(request.column.len()) as u64,
            )
            .map_err(crate::InspectError::from)?;
        if column.name().eq_ignore_ascii_case(request.column)
            && selected.replace((position, column)).is_some()
        {
            return Err(crate::InspectError::Malformed.into());
        }
    }
    let (position, selected) = selected.ok_or(crate::InspectError::UnknownColumn)?;
    let (scalar, kind) = match language {
        Language::Long | Language::UniqueLong => {
            if selected.native_type() != INT32
                || selected.scalar_type() != Some(ScalarType::Int32)
                || selected.flags() != (FIXED_FLAG | UPDATABLE_FLAG)
                || selected.extended_flags() != 0
                || selected.storage_length() != size_of::<i32>()
            {
                return Err(EditError::Unsupported(WriteBlocker::Column));
            }
            (ScalarType::Int32, NumericKey::new(ScalarType::Int32)?)
        }
        Language::Numeric => {
            let scalar = selected
                .scalar_type()
                .ok_or(EditError::Unsupported(WriteBlocker::Column))?;
            let kind = NumericKey::new(scalar)?;
            let width = if scalar == ScalarType::Boolean {
                0
            } else {
                kind.bytes()
            };
            // The shared fixed-column check examines the native format-bounded
            // field list for physical overlap; account that actual traversal.
            budget
                .charge(Resource::Steps, schema.columns().len() as u64 + 1)
                .map_err(crate::InspectError::from)?;
            field::fixed_column_at(schema, position, scalar, width)?;
            (scalar, kind)
        }
    };
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
    if construction == Construction::Fresh {
        next_logical = fresh_logical(next_logical, content.indexes.indexes(), budget, cancelled)?;
    }
    for index in content.indexes.indexes() {
        check(budget, cancelled)?;
        budget
            .charge(
                Resource::Steps,
                index.name().len().max(name.as_str().len()) as u64,
            )
            .map_err(crate::InspectError::from)?;
        // Fresh construction does not infer native Unicode identifier
        // equivalence from SQL text collation or ASCII-only comparison.
        if construction == Construction::Fresh && !index.name().is_ascii() {
            return Err(EditError::Unsupported(WriteBlocker::ConstraintCoverage));
        }
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
        inventory,
        language,
        scalar,
        kind,
        construction,
        logical: next_logical,
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

impl Original<'_> {
    pub(super) fn is_unique(&self) -> bool {
        self.language == Language::UniqueLong
    }

    pub(super) fn is_numeric(&self) -> bool {
        self.language == Language::Numeric
    }

    pub(super) fn value(&self, value: &Value) -> Result<Option<i64>, EditError> {
        match self.language {
            Language::Long | Language::UniqueLong => match value {
                Value::Null => Ok(None),
                Value::Int32(value) => Ok(Some(i64::from(*value))),
                _ => Err(EditError::Preservation),
            },
            Language::Numeric => {
                if matches!(value, Value::Single(value) if !value.to_float().is_finite())
                    || matches!(value, Value::Double(value) if !value.to_float().is_finite())
                {
                    return Err(EditError::Unsupported(WriteBlocker::Column));
                }
                self.kind.value(value).map_err(EditError::from)
            }
        }
    }

    /// Reconstruct source keys independently on each invocation; the audit does
    /// not consume the population or topology prepared by the producer.
    pub(super) fn keys(
        &self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<NumericKeyRow>, EditError> {
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
            let value = self.value(row.get(self.dense).ok_or(EditError::Preservation)?)?;
            output.push(NumericKeyRow {
                value,
                reference: record.reference,
            });
        }
        Ok(output)
    }
}

/// Generated identity policy; input metadata alone does not grant source authority.
pub(super) fn fresh_logical(
    count: u32,
    indexes: &[crate::NativeIndex],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<u32, crate::InspectError> {
    check(budget, cancelled)?;
    let mut occupied = false;
    let mut highest = None;
    for index in indexes {
        check(budget, cancelled)?;
        budget.charge(Resource::Steps, 1)?;
        occupied |= index.number() == count;
        highest = Some(highest.map_or(index.number(), |value: u32| value.max(index.number())));
    }
    if occupied {
        highest
            .ok_or(crate::InspectError::Malformed)?
            .checked_add(1)
            .ok_or(crate::InspectError::StructureLimit)
    } else {
        Ok(count)
    }
}
