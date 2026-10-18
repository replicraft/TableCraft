use crate::format::{
    data_type,
    layout::{COMPLEX_FK_EXT_FLAG, UNICODE_COMPRESSION_FLAG},
    page::PAGE_NUMBER_MASK,
    text::UTF16_UNIT_BYTES,
};
use crate::{
    InspectError, ReadProfile,
    format::{
        layout::{self, Field, Layout},
        reader::Reader,
    },
    inspect_catalog,
};
use std::io::{Read, Seek};
use tablecraft_core::ids::ObjectId;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_core::values::Decimal;
use tablecraft_model::{CatalogScope, ObjectKind};
use tablecraft_schema::{Column, NameComparison, ScalarType, SchemaName, validate_columns};

const LOCAL_TABLE_TYPE: i16 = 1;

/// Native column metadata. Flags/lengths describe storage, not validation rules.
pub struct NativeColumn {
    pub(super) field: Field,
    name: SchemaName,
    profile: ReadProfile,
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
    /// Raw ACE complex-type identifier. Child definitions require separate lookup;
    /// this is neither the parent row's foreign key nor a decoded collection.
    pub fn complex_type_id(&self) -> Option<u32> {
        if self.profile == ReadProfile::Jet4 {
            None
        } else {
            self.field.complex_type_id
        }
    }
    /// Whether this column uses the fixed data region.
    pub const fn fixed(&self) -> bool {
        self.field.fixed
    }
    /// Declared native Decimal precision/scale, including unqualified layouts.
    /// Other native types use these bytes for different metadata.
    pub const fn decimal_parameters(&self) -> Option<(u8, u8)> {
        if self.field.kind == data_type::NUMERIC {
            Some((self.field.precision, self.field.scale))
        } else {
            None
        }
    }
    /// Exact scalar carrier supported by the current row reader.
    /// Calculated, complex and other unqualified extended layouts are rejected.
    pub fn scalar_type(&self) -> Option<ScalarType> {
        let complex_foreign_key =
            matches!(self.profile, ReadProfile::Ace2016 | ReadProfile::Ace2019)
                && self.field.kind == data_type::INT32
                && self.field.fixed
                && self.field.length == data_type::INT32_BYTES
                && self.field.extended_flags == COMPLEX_FK_EXT_FLAG;
        if (self.field.extended_flags != 0
            && !complex_foreign_key
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
            (data_type::INT64, data_type::INT64_BYTES)
                if self.field.fixed
                    && matches!(self.profile, ReadProfile::Ace2016 | ReadProfile::Ace2019) =>
            {
                ScalarType::Int64
            }
            (data_type::EXTENDED_DATE_TIME, data_type::EXTENDED_DATE_TIME_BYTES)
                if self.field.fixed && self.profile == ReadProfile::Ace2019 =>
            {
                ScalarType::DateTimeExtended
            }
            (data_type::CURRENCY, data_type::CURRENCY_BYTES) => ScalarType::Currency,
            (data_type::SINGLE, data_type::SINGLE_BYTES) => ScalarType::Single,
            (data_type::DOUBLE, data_type::DOUBLE_BYTES) => ScalarType::Double,
            (data_type::DATE_TIME, data_type::DATE_TIME_BYTES) => ScalarType::DateTime,
            (data_type::NUMERIC, data_type::NUMERIC_BYTES)
                if !self.field.fixed
                    && (1..=Decimal::MAX_PRECISION).contains(&self.field.precision)
                    && self.field.scale <= self.field.precision =>
            {
                ScalarType::Decimal
            }
            (data_type::GUID, data_type::GUID_BYTES) => ScalarType::Guid,
            (data_type::OLE, 0) if !self.field.fixed => ScalarType::Binary,
            (data_type::LONG_TEXT, 0) if !self.field.fixed => ScalarType::Text,
            (data_type::SHORT_BINARY, 1..=data_type::MAX_SHORT_BINARY_BYTES) => ScalarType::Binary,
            (data_type::SHORT_TEXT, UTF16_UNIT_BYTES..=data_type::MAX_SHORT_TEXT_BYTES)
                if self.field.length.is_multiple_of(UTF16_UNIT_BYTES) =>
            {
                ScalarType::Text
            }
            _ => return None,
        })
    }
    /// Construct the shared scalar column. Required/default/validation properties
    /// are not loaded by this API: all non-Boolean columns permit NULL. Use
    /// inspect_table_properties for separate inert property inspection.
    /// Native Text sizes count UTF-16 units, not schema Unicode-scalar limits.
    pub fn column(&self) -> Result<Column, InspectError> {
        let kind = self.scalar_type().ok_or(InspectError::Unsupported)?;
        Ok(Column::new(
            copy_name(self.name())?,
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
    /// Construct shared scalar columns, charging the vector and owned names to
    /// the caller before allocation. Does not decode any table records.
    pub fn scalar_columns_with_budget(
        &self,
        budget: &mut ResourceBudget,
    ) -> Result<Vec<Column>, InspectError> {
        budget.charge_allocation(self.columns.len() as u64, size_of::<Column>() as u64)?;
        for column in &self.columns {
            budget.charge_allocation(column.name().len() as u64, 1)?;
        }
        self.scalar_columns()
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
    // Layout decoding rejects duplicate column numbers. Equal-key stability is
    // unnecessary, and the in-place sort needs no uncharged scratch allocation.
    layout.fields.sort_unstable_by_key(|f| f.number);
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
        let name = SchemaName::new(copy_name(&field.name)?)?;
        if columns
            .iter()
            .any(|c: &NativeColumn| c.name().eq_ignore_ascii_case(name.as_str()))
        {
            return Err(InspectError::Malformed);
        }
        columns.push(NativeColumn {
            field,
            name,
            profile: catalog.header().read_profile()?,
        });
    }
    reader
        .budget
        .charge_allocation(object.name().as_str().len() as u64, 1)?;
    Ok(TableSchema {
        header: catalog.header(),
        name: SchemaName::new(copy_name(object.name().as_str())?)?,
        object: object.id(),
        definition,
        columns,
        layout,
    })
}

// Callers own logical budget charging; reserve before copying so all three
// native schema name conversions preserve the typed allocation failure path.
fn copy_name(name: &str) -> Result<String, InspectError> {
    let mut owned = String::new();
    owned
        .try_reserve_exact(name.len())
        .map_err(|_| InspectError::Allocation)?;
    owned.push_str(name);
    Ok(owned)
}
