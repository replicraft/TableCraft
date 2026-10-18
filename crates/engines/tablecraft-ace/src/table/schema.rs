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
            && !(self.field.kind == 10 && self.field.extended_flags == 1))
            || (self.field.kind == 1 && !self.field.fixed)
        {
            return None;
        }
        Some(match (self.field.kind, self.field.length) {
            (1, 0) => ScalarType::Boolean,
            (2, 1) => ScalarType::Byte,
            (3, 2) => ScalarType::Int16,
            (4, 4) => ScalarType::Int32,
            (5, 8) => ScalarType::Currency,
            (6, 4) => ScalarType::Single,
            (7, 8) => ScalarType::Double,
            (9, 1..=510) => ScalarType::Binary,
            (10, 2..=510) if self.field.length.is_multiple_of(2) => ScalarType::Text,
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
    if origin.type_code() != 1 {
        return Err(InspectError::Unsupported);
    }
    let definition = origin.native_id() as u32 & 0x00ff_ffff;
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
