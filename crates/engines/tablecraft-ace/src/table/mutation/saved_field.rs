use crate::{
    NativeColumn, NativeImage, PropertyGroupKind, TableProperties, TableSchema,
    format::{
        bytes::u32_at,
        data_type, definition,
        layout::{self, FIXED_FLAG, UNICODE_COMPRESSION_FLAG, UPDATABLE_FLAG},
        page::PAGE_BYTES,
        reader::Reader,
    },
    header::check,
    inspect_table_indexes, inspect_table_properties, inspect_table_schema,
    properties::framing,
    table::{
        long_value::trace::Kind,
        mutation::{
            catalog_property::{column, descriptor},
            field,
            record_insertion::payload as insertion_payload,
            record_serialization::storage,
        },
    },
};
use std::ops::Range;
use tablecraft_core::{
    ids::ObjectId,
    limits::{LimitError, Resource, ResourceBudget},
};
use tablecraft_schema::ScalarType;
use thiserror::Error;

#[derive(Debug, Error)]
pub(super) enum Error {
    #[error(transparent)]
    Inspect(#[from] crate::InspectError),
    #[error("saved field target is outside the supported source profile")]
    Source,
}

impl From<LimitError> for Error {
    fn from(error: LimitError) -> Self {
        Self::Inspect(error.into())
    }
}

#[derive(Clone, Copy)]
pub(super) struct Identity {
    pub object: ObjectId,
    pub native_table: i32,
    pub definition: u32,
    pub number: usize,
    pub column_id: u16,
    pub name_units: usize,
}

impl Identity {
    pub fn column_number(self) -> usize {
        self.number
    }
}

const DDL_FLAG: u8 = 1;
const USER_DEFINITION: u32 = 7;
const USER_TYPE: i16 = 1;
const OBSERVED_TEXT_BYTES: usize = 255 * size_of::<u16>();
// Tagged byte length, reference and reserved word precede an inline body.
const INLINE_DESCRIPTOR_BYTES: usize = 3 * size_of::<u32>();

pub(super) struct Observed {
    pub schema: TableSchema,
    pub properties: TableProperties,
    pub identity: Identity,
    pub definition_name: Range<usize>,
    pub property_name: Range<usize>,
    pub property_body: Range<usize>,
    pub property_descriptor: Range<usize>,
    pub property_reference: Option<u32>,
    pub definition_entry: Range<usize>,
    pub group: usize,
    index: usize,
}

#[derive(Clone, Copy)]
enum Purpose {
    SavedField,
    CurrentTail,
    CurrentRename,
    RetainedTail,
    RetainedRename,
    RetainedTypedRename,
    RetainedTypedTail,
}

impl Purpose {
    fn retained(self) -> bool {
        matches!(
            self,
            Self::RetainedTail
                | Self::RetainedRename
                | Self::RetainedTypedRename
                | Self::RetainedTypedTail
        )
    }
}

impl Observed {
    pub fn column(&self) -> &NativeColumn {
        &self.schema.columns()[self.index]
    }

    pub fn capture(
        image: &NativeImage,
        table: &str,
        field: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, Error> {
        Self::capture_for(image, table, field, Purpose::SavedField, budget, cancelled)
    }

    pub(in crate::table::mutation) fn capture_current_tail(
        image: &NativeImage,
        table: &str,
        field: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, Error> {
        Self::capture_for(image, table, field, Purpose::CurrentTail, budget, cancelled)
    }

    pub(in crate::table::mutation) fn capture_current_rename(
        image: &NativeImage,
        table: &str,
        field: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, Error> {
        Self::capture_for(
            image,
            table,
            field,
            Purpose::CurrentRename,
            budget,
            cancelled,
        )
    }

    pub(in crate::table::mutation) fn capture_retained_schema_drop(
        image: &NativeImage,
        table: &str,
        field: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, Error> {
        Self::capture_for(
            image,
            table,
            field,
            Purpose::RetainedTail,
            budget,
            cancelled,
        )
    }

    pub(in crate::table::mutation) fn capture_retained_schema_rename(
        image: &NativeImage,
        table: &str,
        field: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, Error> {
        Self::capture_for(
            image,
            table,
            field,
            Purpose::RetainedRename,
            budget,
            cancelled,
        )
    }

    pub(in crate::table::mutation) fn capture_retained_typed_rename(
        image: &NativeImage,
        table: &str,
        field: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, Error> {
        Self::capture_for(
            image,
            table,
            field,
            Purpose::RetainedTypedRename,
            budget,
            cancelled,
        )
    }

    pub(in crate::table::mutation) fn capture_retained_typed_tail(
        image: &NativeImage,
        table: &str,
        field: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, Error> {
        Self::capture_for(
            image,
            table,
            field,
            Purpose::RetainedTypedTail,
            budget,
            cancelled,
        )
    }

    fn capture_for(
        image: &NativeImage,
        table: &str,
        field: &str,
        purpose: Purpose,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, Error> {
        check(budget, cancelled)?;
        let scope = image.catalog().catalog().scope();
        let schema = inspect_table_schema(&mut image.reader(), scope, table, budget, cancelled)?;
        let origin = image
            .catalog()
            .origins()
            .iter()
            .find(|origin| origin.object_id() == schema.object_id())
            .ok_or(Error::Source)?;
        if origin.type_code() != USER_TYPE
            || origin.flags() != 0
            || !purpose.retained()
                && (origin.native_id() != USER_DEFINITION as i32
                    || schema.definition_page() != USER_DEFINITION)
        {
            return Err(Error::Source);
        }
        let mut selected = None;
        for (index, candidate) in schema.columns().iter().enumerate() {
            check(budget, cancelled)?;
            budget.charge(
                Resource::Steps,
                candidate.name().len().max(field.len()) as u64,
            )?;
            // This first profile has no proved native locale identifier comparator.
            // Every possible collision partner must belong to the ASCII namespace,
            // not only the selected target and proposed replacement.
            if !candidate.name().is_ascii() {
                return Err(Error::Source);
            }
            if candidate.name().eq_ignore_ascii_case(field) && selected.replace(index).is_some() {
                return Err(Error::Source);
            }
        }
        let index = selected.ok_or(Error::Source)?;
        let target = &schema.columns()[index];
        if target.number() == 0
            && !matches!(
                purpose,
                Purpose::RetainedTypedRename | Purpose::RetainedTypedTail
            )
            || !target.name().is_ascii()
        {
            return Err(Error::Source);
        }
        if matches!(
            purpose,
            Purpose::CurrentTail | Purpose::RetainedTail | Purpose::RetainedTypedTail
        ) && index + 1 != schema.columns().len()
        {
            return Err(Error::Source);
        }
        if matches!(purpose, Purpose::CurrentTail)
            && target.number().checked_add(1) != Some(schema.physical_layout().max_columns)
        {
            return Err(Error::Source);
        }
        if matches!(purpose, Purpose::RetainedTypedTail) && schema.columns().len() < 2 {
            // The qualified table source requires a surviving declaration.
            return Err(Error::Source);
        }
        let indexes =
            inspect_table_indexes(&mut image.reader(), scope, schema.name(), budget, cancelled)?;
        for key in indexes.indexes() {
            check(budget, cancelled)?;
            for component in key.columns() {
                check(budget, cancelled)?;
                if component.number() == target.number() {
                    return Err(Error::Source);
                }
            }
        }
        let properties =
            inspect_table_properties(&mut image.reader(), scope, schema.name(), budget, cancelled)?;
        let mut group = None;
        for (number, candidate) in properties.groups().iter().enumerate() {
            check(budget, cancelled)?;
            budget.charge(
                Resource::Steps,
                candidate.name().len().max(target.name().len()) as u64,
            )?;
            if candidate.kind() == PropertyGroupKind::Column
                && candidate.name() == target.name()
                && group.replace(number).is_some()
            {
                return Err(Error::Source);
            }
        }
        let group = group.ok_or(Error::Source)?;
        if matches!(
            purpose,
            Purpose::RetainedTypedRename | Purpose::RetainedTypedTail
        ) {
            // The consuming retained-schema source separately qualifies every
            // actual property, literal default, dependency and program. This
            // observation preserves that group rather than inventing a type-
            // specific default spelling or rewriting its property values.
            qualify_typed_declaration(&schema, index)?;
        } else {
            qualify_declaration(target, &properties, group, purpose, budget, cancelled)?;
        }
        let mut input = image.reader();
        let mut reader = Reader {
            source: &mut input,
            header: image.catalog().header(),
            budget,
            cancelled,
            reads: 0,
        };
        let root = reader.page(schema.definition_page())?;
        if u32_at(&root, definition::NEXT_PAGE_OFFSET)? != 0
            || u32_at(&root, definition::LENGTH_OFFSET)? as usize > PAGE_BYTES
        {
            return Err(Error::Source);
        }
        let mut parsed = None;
        layout::read_with_field_names(
            &mut reader,
            schema.definition_page(),
            |number, id, entry, name| {
                if number == target.number() && parsed.replace((id, entry, name)).is_some() {
                    return Err(crate::InspectError::Malformed);
                }
                Ok(())
            },
        )?;
        let (column_id, entry, name) = parsed.ok_or(Error::Source)?;
        if name.start < entry.end || name.len() != target.name().len() * size_of::<u16>() {
            return Err(Error::Source);
        }
        let base = schema.definition_page() as usize * PAGE_BYTES;
        let definition_name = base + name.start..base + name.end;
        let definition_entry = base + entry.start..base + entry.end;
        let catalog = inspect_table_schema(
            &mut image.reader(),
            scope,
            "MSysObjects",
            reader.budget,
            cancelled,
        )?;
        let selected = column(&catalog)?;
        let descriptor = if purpose.retained() {
            image.require_retained_capture()?;
            let inventory =
                crate::table::table_append::Inventory::capture(image, reader.budget, cancelled)
                    .map_err(|error| match error {
                        crate::EditError::Inspect(error) => Error::Inspect(error),
                        _ => Error::Source,
                    })?;
            inventory.require_source(image).map_err(|_| Error::Source)?;
            if inventory.catalog_definition() != catalog.definition_page()
                || inventory.catalog_live_rows() != catalog.declared_rows()
            {
                return Err(Error::Source);
            }
            descriptor::capture_retained(
                &mut reader,
                &catalog,
                selected,
                origin,
                inventory.catalog_pages(),
            )?
        } else {
            descriptor::capture(&mut reader, &catalog, selected, origin)?
        };
        let traced = descriptor.traced(&mut reader)?;
        if traced.value.as_deref() != Some(properties.raw_bytes()) {
            return Err(Error::Source);
        }
        let property_descriptor = descriptor.absolute_range()?.ok_or(Error::Source)?;
        let (property_body, property_reference) = match traced.kind {
            Kind::Single => {
                let [chunk] = traced.chunks.as_slice() else {
                    return Err(Error::Source);
                };
                if !descriptor.canonical_single(chunk.reference, properties.raw_bytes().len())?
                    || chunk.bytes != properties.raw_bytes().len()
                {
                    return Err(Error::Source);
                }
                let end = chunk.offset.checked_add(chunk.bytes).ok_or(Error::Source)?;
                (chunk.offset..end, Some(chunk.reference))
            }
            Kind::Inline if purpose.retained() && traced.chunks.is_empty() => {
                let start = property_descriptor
                    .start
                    .checked_add(INLINE_DESCRIPTOR_BYTES)
                    .ok_or(Error::Source)?;
                let end = start
                    .checked_add(properties.raw_bytes().len())
                    .ok_or(Error::Source)?;
                if end > property_descriptor.end {
                    return Err(Error::Source);
                }
                (start..end, None)
            }
            _ => return Err(Error::Source),
        };
        reader.work(2 * properties.raw_bytes().len())?;
        if image.as_bytes().get(property_body.start..property_body.end)
            != Some(properties.raw_bytes())
        {
            return Err(Error::Source);
        }
        let mut group_span = None;
        let visit = |number, span| {
            if number == group && group_span.replace(span).is_some() {
                return Err(crate::InspectError::Malformed);
            }
            Ok(())
        };
        match purpose {
            Purpose::SavedField => {
                framing::ordered_names_with_groups(&properties, reader.budget, cancelled, visit)?
            }
            Purpose::CurrentTail
            | Purpose::CurrentRename
            | Purpose::RetainedTail
            | Purpose::RetainedRename
            | Purpose::RetainedTypedRename
            | Purpose::RetainedTypedTail => framing::current_schema_names_with_groups(
                &properties,
                reader.budget,
                cancelled,
                visit,
            )?,
        };
        let name = group_span.ok_or(Error::Source)?;
        if name.len() != target.name().len() * size_of::<u16>() || name.end > property_body.len() {
            return Err(Error::Source);
        }
        let property_name = property_body.start + name.start..property_body.start + name.end;
        // NO_CLONE: Verify encoded stored name bytes and retain both definition/property
        // spans in the resulting SavedField observation. Only range bounds are
        // copied, not the payload.
        exact_name(
            image.as_bytes(),
            definition_name.clone(),
            target.name(),
            reader.budget,
        )?;
        // NO_CLONE: Verify encoded stored name bytes and retain both definition/property
        // spans in the resulting SavedField observation. Only range bounds are
        // copied, not the payload.
        exact_name(
            image.as_bytes(),
            property_name.clone(),
            target.name(),
            reader.budget,
        )?;
        let identity = Identity {
            object: schema.object_id(),
            native_table: origin.native_id(),
            definition: schema.definition_page(),
            number: target.number(),
            column_id,
            name_units: target.name().len(),
        };
        check(reader.budget, cancelled)?;
        Ok(Self {
            schema,
            properties,
            identity,
            definition_name,
            property_name,
            property_body,
            property_descriptor,
            property_reference,
            definition_entry,
            group,
            index,
        })
    }
}

fn exact_name(
    image: &[u8],
    span: Range<usize>,
    name: &str,
    budget: &mut ResourceBudget,
) -> Result<(), Error> {
    let raw = image.get(span).ok_or(Error::Source)?;
    budget.charge(Resource::Steps, raw.len() as u64)?;
    if !raw
        .as_chunks::<2>()
        .0
        .iter()
        .map(|word| u16::from_le_bytes(*word))
        .eq(name.encode_utf16())
    {
        return Err(Error::Source);
    }
    Ok(())
}

fn qualify_declaration(
    column: &NativeColumn,
    properties: &TableProperties,
    group: usize,
    purpose: Purpose,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), Error> {
    if original_declaration_matches(column, properties, group, purpose, budget, cancelled)? {
        Ok(())
    } else {
        Err(Error::Source)
    }
}

fn original_declaration_matches(
    column: &NativeColumn,
    properties: &TableProperties,
    group: usize,
    purpose: Purpose,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<bool, Error> {
    let number = column.native_type() == data_type::INT32
        && column.fixed()
        && column.storage_length() == size_of::<i32>()
        && column.flags() == (FIXED_FLAG | UPDATABLE_FLAG)
        && column.extended_flags() == 0;
    let text = column.native_type() == data_type::SHORT_TEXT
        && !column.fixed()
        && column.flags() == UPDATABLE_FLAG
        && match purpose {
            Purpose::SavedField => {
                column.storage_length() == OBSERVED_TEXT_BYTES
                    && column.extended_flags() == UNICODE_COMPRESSION_FLAG
            }
            Purpose::CurrentTail
            | Purpose::CurrentRename
            | Purpose::RetainedTail
            | Purpose::RetainedRename
            | Purpose::RetainedTypedRename
            | Purpose::RetainedTypedTail => {
                (size_of::<u16>()..=OBSERVED_TEXT_BYTES).contains(&column.storage_length())
                    && column.storage_length().is_multiple_of(size_of::<u16>())
                    && column.extended_flags() & !UNICODE_COMPRESSION_FLAG == 0
            }
        };
    if !number && !text {
        return Ok(false);
    }
    let entries = properties.groups()[group].properties();
    if entries.len() != 2 {
        return Ok(false);
    }
    let mut required = false;
    let mut second = false;
    for property in entries {
        check(budget, cancelled)?;
        budget.charge(
            Resource::Steps,
            (property.name().len() + property.raw_value().len()) as u64,
        )?;
        if property.flags() != DDL_FLAG {
            return Ok(false);
        }
        match property.name() {
            "Required"
                if property.native_type() == data_type::BOOLEAN && property.raw_value() == [0] =>
            {
                required = true
            }
            "DefaultValue"
                if number
                    && property.native_type() == data_type::LONG_TEXT
                    && property.raw_value() == [b'0', 0] =>
            {
                second = true
            }
            "AllowZeroLength"
                if text
                    && property.native_type() == data_type::BOOLEAN
                    && property.raw_value() == [255] =>
            {
                second = true
            }
            _ => return Ok(false),
        }
    }
    Ok(required && second)
}

fn qualify_typed_declaration(schema: &TableSchema, position: usize) -> Result<(), Error> {
    let column = schema.columns().get(position).ok_or(Error::Source)?;
    let (kind, width) = match column.scalar_type() {
        Some(ScalarType::Boolean) => (ScalarType::Boolean, data_type::BOOLEAN_BYTES),
        Some(ScalarType::Byte) => (ScalarType::Byte, size_of::<u8>()),
        Some(ScalarType::Int16) => (ScalarType::Int16, size_of::<i16>()),
        Some(ScalarType::Int32) => (ScalarType::Int32, size_of::<i32>()),
        Some(ScalarType::Int64) => (ScalarType::Int64, size_of::<i64>()),
        Some(ScalarType::Single) => (ScalarType::Single, size_of::<f32>()),
        Some(ScalarType::Double) => (ScalarType::Double, size_of::<f64>()),
        Some(ScalarType::Currency) => (ScalarType::Currency, size_of::<i64>()),
        Some(ScalarType::DateTime) => (ScalarType::DateTime, size_of::<f64>()),
        Some(ScalarType::Guid) => (ScalarType::Guid, data_type::GUID_BYTES),
        Some(ScalarType::Decimal) => return insertion_payload::qualify(column).map_err(edit_error),
        Some(ScalarType::Text) => return storage::qualify(column).map_err(edit_error),
        _ => return Err(Error::Source),
    };
    field::fixed_column_at(schema, position, kind, width).map_err(edit_error)?;
    Ok(())
}

fn edit_error(error: crate::EditError) -> Error {
    match error {
        crate::EditError::Inspect(error) => Error::Inspect(error),
        _ => Error::Source,
    }
}

/// Choose the typed language from actual metadata, before consuming a source.
pub(in crate::table::mutation) fn retained_field_uses_typed_language(
    image: &NativeImage,
    table: &str,
    requested: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<bool, Error> {
    check(budget, cancelled)?;
    let scope = image.catalog().catalog().scope();
    let schema = inspect_table_schema(&mut image.reader(), scope, table, budget, cancelled)?;
    let mut selected = None;
    for (position, column) in schema.columns().iter().enumerate() {
        check(budget, cancelled)?;
        budget.charge(
            Resource::Steps,
            column.name().len().max(requested.len()) as u64,
        )?;
        if column.name().eq_ignore_ascii_case(requested) && selected.replace(position).is_some() {
            return Err(Error::Source);
        }
    }
    let position = selected.ok_or(Error::Source)?;
    qualify_typed_declaration(&schema, position)?;
    let column = &schema.columns()[position];
    let properties =
        inspect_table_properties(&mut image.reader(), scope, table, budget, cancelled)?;
    let mut selected_group = None;
    for (number, group) in properties.groups().iter().enumerate() {
        check(budget, cancelled)?;
        budget.charge(
            Resource::Steps,
            group.name().len().max(column.name().len()) as u64,
        )?;
        if group.kind() == PropertyGroupKind::Column
            && group.name() == column.name()
            && selected_group.replace(number).is_some()
        {
            return Err(Error::Source);
        }
    }
    let group = selected_group.ok_or(Error::Source)?;
    Ok(column.number() == 0
        || !original_declaration_matches(
            column,
            &properties,
            group,
            Purpose::RetainedRename,
            budget,
            cancelled,
        )?)
}

#[cfg(test)]
mod tests;
