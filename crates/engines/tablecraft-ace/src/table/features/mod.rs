use super::{NativeColumn, TableSchema, inspect_table_schema};
use crate::{
    InspectError, PropertyGroup, PropertyGroupKind, TableProperties, inspect_table_properties,
};
use std::io::{Read, Seek};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;

pub(super) const CALCULATED_MASK: u8 = 0xc0;
const HYPERLINK_MASK: u8 = 0x80;
const AUTO_NUMBER_MASK: u8 = 0x44;
const COMPLEX_FOREIGN_KEY_MASK: u8 = 0x08;
const EXPRESSION: &str = "Expression";
const RESULT_TYPE: &str = "ResultType";
const APPEND_ONLY: &str = "AppendOnly";
const TEXT_FORMAT: &str = "TextFormat";

/// Inert joined schema/property metadata; no expression or active content runs.
pub struct TableFeatures {
    schema: TableSchema,
    properties: TableProperties,
}
impl TableFeatures {
    /// Validated native table/column geometry, including unsupported types.
    pub fn schema(&self) -> &TableSchema {
        &self.schema
    }
    /// Complete original property stream and scoped declarations.
    pub fn properties(&self) -> &TableProperties {
        &self.properties
    }
    /// Borrowed feature declarations in native column order.
    pub fn columns(&self) -> impl ExactSizeIterator<Item = ColumnFeatures<'_>> {
        self.schema.columns().iter().map(|column| ColumnFeatures {
            column,
            properties: self
                .properties
                .group(PropertyGroupKind::Column, column.name()),
        })
    }
}

/// Borrowed field declarations, not authorization for mutation or recalculation.
pub struct ColumnFeatures<'a> {
    column: &'a NativeColumn,
    properties: Option<&'a PropertyGroup>,
}
impl<'a> ColumnFeatures<'a> {
    /// Native schema descriptor; logical numbers are not persistent project IDs.
    pub fn column(&self) -> &'a NativeColumn {
        self.column
    }
    /// Calculated-field flag; the result remains outside ordinary scalar reads.
    pub fn calculated(&self) -> bool {
        self.column.extended_flags() & CALCULATED_MASK != 0
    }
    /// Native complex-child foreign-key marker, without interpreting it as data.
    pub fn complex_foreign_key(&self) -> bool {
        self.column.extended_flags() & COMPLEX_FOREIGN_KEY_MASK != 0
    }
    /// Native AutoNumber/replication-ID marker, without allocation semantics.
    pub fn auto_number(&self) -> bool {
        self.column.flags() & AUTO_NUMBER_MASK != 0
    }
    /// Native Hyperlink marker; content is still preserved as stored text.
    pub fn hyperlink(&self) -> bool {
        self.column.flags() & HYPERLINK_MASK != 0
    }
    /// Declared calculation text, never executed or inferred from stored data.
    pub fn expression(&self) -> Result<Option<&'a str>, InspectError> {
        self.properties
            .and_then(|p| p.get(EXPRESSION))
            .map(|p| p.text())
            .transpose()
    }
    /// Declared calculation result's native type tag, not its physical row codec.
    pub fn result_native_type(&self) -> Result<Option<u8>, InspectError> {
        self.properties
            .and_then(|p| p.get(RESULT_TYPE))
            .map(|p| p.byte())
            .transpose()
    }
    /// Explicit append-only declaration; absent is unknown, not false.
    pub fn append_only(&self) -> Result<Option<bool>, InspectError> {
        self.properties
            .and_then(|p| p.get(APPEND_ONLY))
            .map(|p| p.boolean())
            .transpose()
    }
    /// Explicit TextFormat declaration (0 plain / 1 rich); other values stay raw.
    pub fn text_format(&self) -> Result<Option<u8>, InspectError> {
        self.properties
            .and_then(|p| p.get(TEXT_FORMAT))
            .map(|p| p.byte())
            .transpose()
    }
}

/// Join inert column declarations with qualified property maps under shared limits.
/// The host must provide a stable/offline source, as for other native inspection.
/// Unknown properties and values remain in the returned original property stream.
pub fn inspect_table_features<R: Read + Seek>(
    source: &mut R,
    scope: CatalogScope,
    name: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<TableFeatures, InspectError> {
    let _span = tracing::debug_span!("native.features.read").entered();
    let schema = inspect_table_schema(source, scope, name, budget, cancelled)?;
    let properties = inspect_table_properties(source, scope, name, budget, cancelled)?;
    if schema.object_id() != properties.object_id() {
        return Err(InspectError::Malformed);
    }
    Ok(TableFeatures { schema, properties })
}
