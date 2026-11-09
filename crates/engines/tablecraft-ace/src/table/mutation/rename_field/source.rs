use super::{FieldRenameError as Error, NativeFieldRenameIdentity};
use crate::{
    NativeColumn, NativeImage, PropertyGroupKind, TableProperties, TableSchema,
    creation::FieldHistoryOffSource,
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
        mutation::catalog_property::{column, descriptor},
    },
};
use std::ops::Range;
use tablecraft_core::limits::{Resource, ResourceBudget};

const DDL_FLAG: u8 = 1;
const USER_DEFINITION: u32 = 7;
const USER_TYPE: i16 = 1;
const OBSERVED_TEXT_BYTES: usize = 255 * size_of::<u16>();

/// Its private source can only come from the complete shared settled proof.
pub(super) struct Qualified {
    proof: FieldHistoryOffSource,
    pub observed: Observed,
}

impl Qualified {
    pub fn capture(
        image: NativeImage,
        table: &str,
        field: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, Error> {
        let proof = FieldHistoryOffSource::capture(image, budget, cancelled)?;
        let observed = Observed::capture(proof.image(), table, field, budget, cancelled)?;
        Ok(Self { proof, observed })
    }

    pub fn image(&self) -> &NativeImage {
        self.proof.image()
    }
}

pub(super) struct Observed {
    pub schema: TableSchema,
    pub properties: TableProperties,
    pub identity: NativeFieldRenameIdentity,
    pub definition_name: Range<usize>,
    pub property_name: Range<usize>,
    pub definition_entry: Range<usize>,
    pub group: usize,
    index: usize,
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
        check(budget, cancelled)?;
        let scope = image.catalog().catalog().scope();
        let schema = inspect_table_schema(&mut image.reader(), scope, table, budget, cancelled)?;
        let origin = image
            .catalog()
            .origins()
            .iter()
            .find(|origin| origin.object_id() == schema.object_id())
            .ok_or(Error::Source)?;
        if origin.native_id() != USER_DEFINITION as i32
            || origin.type_code() != USER_TYPE
            || origin.flags() != 0
            || schema.definition_page() != USER_DEFINITION
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
        if target.number() == 0 || !target.name().is_ascii() {
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
        qualify_declaration(target, &properties, group, budget, cancelled)?;
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
        let descriptor = descriptor::capture(&mut reader, &catalog, selected, origin)?;
        let traced = descriptor.traced(&mut reader)?;
        let [chunk] = traced.chunks.as_slice() else {
            return Err(Error::Source);
        };
        if !matches!(traced.kind, Kind::Single)
            || !descriptor.canonical_single(chunk.reference, properties.raw_bytes().len())?
            || traced.value.as_deref() != Some(properties.raw_bytes())
            || chunk.bytes != properties.raw_bytes().len()
        {
            return Err(Error::Source);
        }
        reader.work(2 * properties.raw_bytes().len())?;
        let end = chunk.offset.checked_add(chunk.bytes).ok_or(Error::Source)?;
        if image.as_bytes().get(chunk.offset..end) != Some(properties.raw_bytes()) {
            return Err(Error::Source);
        }
        let mut group_span = None;
        framing::ordered_names_with_groups(
            &properties,
            reader.budget,
            cancelled,
            |number, span| {
                if number == group && group_span.replace(span).is_some() {
                    return Err(crate::InspectError::Malformed);
                }
                Ok(())
            },
        )?;
        let name = group_span.ok_or(Error::Source)?;
        if name.len() != target.name().len() * size_of::<u16>() || name.end > chunk.bytes {
            return Err(Error::Source);
        }
        let property_name = chunk.offset + name.start..chunk.offset + name.end;
        exact_name(
            image.as_bytes(),
            definition_name.clone(),
            target.name(),
            reader.budget,
        )?;
        exact_name(
            image.as_bytes(),
            property_name.clone(),
            target.name(),
            reader.budget,
        )?;
        let identity = NativeFieldRenameIdentity {
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
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), Error> {
    let number = column.native_type() == data_type::INT32
        && column.fixed()
        && column.storage_length() == size_of::<i32>()
        && column.flags() == (FIXED_FLAG | UPDATABLE_FLAG)
        && column.extended_flags() == 0;
    let text = column.native_type() == data_type::SHORT_TEXT
        && !column.fixed()
        && column.storage_length() == OBSERVED_TEXT_BYTES
        && column.flags() == UPDATABLE_FLAG
        && column.extended_flags() == UNICODE_COMPRESSION_FLAG;
    if !number && !text {
        return Err(Error::Source);
    }
    let entries = properties.groups()[group].properties();
    if entries.len() != 2 {
        return Err(Error::Source);
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
            return Err(Error::Source);
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
            _ => return Err(Error::Source),
        }
    }
    if !required || !second {
        return Err(Error::Source);
    }
    Ok(())
}
