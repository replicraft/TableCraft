use crate::format::{
    data_type, layout::UNICODE_COMPRESSION_FLAG, page::PAGE_NUMBER_MASK, text::UTF16_UNIT_BYTES,
};
use crate::{
    InspectError,
    format::{
        layout::{self, Field, Layout},
        reader::Reader,
    },
    inspect_catalog,
};
use std::io::{Read, Seek};
use tablecraft_core::ids::ObjectId;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::{CatalogScope, ObjectKind};
use tablecraft_schema::{Column, NameComparison, ScalarType, SchemaName, validate_columns};

const LOCAL_TABLE_TYPE: i16 = 1;

/// Native column metadata. Flags/lengths describe storage, not validation rules.
pub struct NativeColumn {
    pub(super) field: Field,
    name: SchemaName,
}
impl NativeColumn {
    /// Original validated field name.
    pub fn name(&self) -> &str {
        self.name.as_str()
    }
    /// Native data type code, including unsupported types.
    pub const fn native_type(&self) -> u8 {
        self.field.kind
    }
    /// Logical native column number, distinct from vector/display position.
    pub const fn number(&self) -> usize {
        self.field.number
    }
    /// Native storage length in bytes; Boolean has no payload bytes.
    pub const fn storage_length(&self) -> usize {
        self.field.length
    }
    /// Native flags, without inferring Required or validation properties.
    pub const fn flags(&self) -> u8 {
        self.field.flags
    }
    /// Native extended flags, including compression/calculated-field metadata.
    pub const fn extended_flags(&self) -> u8 {
        self.field.extended_flags
    }
    /// Whether this column uses the fixed data region.
    pub const fn fixed(&self) -> bool {
        self.field.fixed
    }
    /// Exact scalar carrier supported by the current row reader.
    /// Calculated, complex and other unqualified extended layouts are rejected.
    pub fn scalar_type(&self) -> Option<ScalarType> {
        if (self.field.extended_flags != 0
            && !(matches!(
                self.field.kind,
                data_type::SHORT_TEXT | data_type::LONG_TEXT
            ) && self.field.extended_flags == UNICODE_COMPRESSION_FLAG))
            || (self.field.kind == data_type::BOOLEAN && !self.field.fixed)
        {
            return None;
        }
        Some(match (self.field.kind, self.field.length) {
            (data_type::BOOLEAN, data_type::BOOLEAN_BYTES) => ScalarType::Boolean,
            (data_type::BYTE, data_type::BYTE_BYTES) => ScalarType::Byte,
            (data_type::INT16, data_type::INT16_BYTES) => ScalarType::Int16,
            (data_type::INT32, data_type::INT32_BYTES) => ScalarType::Int32,
            (data_type::CURRENCY, data_type::CURRENCY_BYTES) => ScalarType::Currency,
            (data_type::SINGLE, data_type::SINGLE_BYTES) => ScalarType::Single,
            (data_type::DOUBLE, data_type::DOUBLE_BYTES) => ScalarType::Double,
            (data_type::DATE_TIME, data_type::DATE_TIME_BYTES) => ScalarType::DateTime,
            (data_type::GUID, data_type::GUID_BYTES) => ScalarType::Guid,
            (data_type::LONG_TEXT, 0) if !self.field.fixed => ScalarType::Text,
            (data_type::SHORT_BINARY, 1..=data_type::MAX_SHORT_VALUE_BYTES) => ScalarType::Binary,
            (data_type::SHORT_TEXT, UTF16_UNIT_BYTES..=data_type::MAX_SHORT_VALUE_BYTES)
                if self.field.length.is_multiple_of(UTF16_UNIT_BYTES) =>
            {
                ScalarType::Text
            }
            _ => return None,
        })
    }
    /// Construct the shared scalar column. Required/default/validation properties
    /// are not decoded: all non-Boolean columns conservatively permit NULL.
    /// Native Text sizes count UTF-16 units, not schema Unicode-scalar limits.
    pub fn column(&self) -> Result<Column, InspectError> {
        let kind = self.scalar_type().ok_or(InspectError::Unsupported)?;
        Ok(Column::new(
            self.name().to_owned(),
            kind,
            kind != ScalarType::Boolean,
        )?)
    }
}
/// A local table's metadata, independent of record decoding/query execution.
pub struct TableSchema {
    pub(super) header: crate::Header,
    pub(super) name: SchemaName,
    pub(super) object: ObjectId,
    pub(super) definition: u32,
    pub(super) columns: Vec<NativeColumn>,
    pub(super) layout: Layout,
}
impl TableSchema {
    /// Original table name.
    pub fn name(&self) -> &str {
        self.name.as_str()
    }
    /// Catalog-scoped identity of the table.
    pub const fn object_id(&self) -> ObjectId {
        self.object
    }
    /// Original table definition page, not a mutation handle.
    pub const fn definition_page(&self) -> u32 {
        self.definition
    }
    /// Declared live record count; verified only when all rows are decoded.
    pub const fn declared_rows(&self) -> u32 {
        self.layout.records
    }
    /// Native columns in logical column-number order.
    pub fn columns(&self) -> &[NativeColumn] {
        &self.columns
    }
    /// Shared input schema, failing if any column is unsupported.
    pub fn scalar_columns(&self) -> Result<Vec<Column>, InspectError> {
        let mut columns = Vec::new();
        columns
            .try_reserve_exact(self.columns.len())
            .map_err(|_| InspectError::Allocation)?;
        for c in &self.columns {
            columns.push(c.column()?);
        }
        validate_columns(&columns, NameComparison::AsciiInsensitive)?;
        Ok(columns)
    }
}
/// Inspect one explicitly named local table on a stable/offline source. Linked
/// tables and saved queries are rejected; no provider or active content is run.
/// The shared catalog/header and all metadata reads consume the caller's budget.
pub fn inspect_table_schema<R: Read + Seek>(
    source: &mut R,
    scope: CatalogScope,
    name: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<TableSchema, InspectError> {
    let catalog = inspect_catalog(source, scope, budget, cancelled)?;
    let object = catalog
        .catalog()
        .lookup(name, Some(ObjectKind::Table), None)?
        .ok_or(InspectError::UnknownTable)?;
    let origin = catalog
        .origins()
        .iter()
        .find(|o| o.object_id() == object.id())
        .ok_or(InspectError::Malformed)?;
    if origin.type_code() != LOCAL_TABLE_TYPE {
        return Err(InspectError::Unsupported);
    }
    let definition = origin.native_id() as u32 & PAGE_NUMBER_MASK;
    let mut reader = Reader {
        source,
        header: catalog.header(),
        budget,
        cancelled,
        reads: 0,
    };
    let mut layout = layout::read(&mut reader, definition)?;
    layout.fields.sort_by_key(|f| f.number);
    reader.budget.charge_allocation(
        layout.fields.len() as u64,
        std::mem::size_of::<NativeColumn>() as u64,
    )?;
    let mut columns = Vec::new();
    columns
        .try_reserve_exact(layout.fields.len())
        .map_err(|_| InspectError::Allocation)?;
    for field in std::mem::take(&mut layout.fields) {
        reader.tick()?;
        reader
            .budget
            .charge_allocation(field.name.len() as u64, 1)?;
        let name = SchemaName::new(field.name.clone())?;
        if columns
            .iter()
            .any(|c: &NativeColumn| c.name().eq_ignore_ascii_case(name.as_str()))
        {
            return Err(InspectError::Malformed);
        }
        columns.push(NativeColumn { field, name });
    }
    reader
        .budget
        .charge_allocation(object.name().as_str().len() as u64, 1)?;
    Ok(TableSchema {
        header: catalog.header(),
        name: object.name().clone(),
        object: object.id(),
        definition,
        columns,
        layout,
    })
}
