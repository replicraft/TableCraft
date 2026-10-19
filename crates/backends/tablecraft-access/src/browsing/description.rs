use tablecraft_ace::{NativeColumn, TableSchema};

/// Column-level scalar support from inert native schema metadata only.
/// This does not read records, validate row histories or prove whole-table
/// readability, integrity, collation behavior or resource requirements.
pub struct TableDescription {
    pub(super) schema: TableSchema,
}
impl TableDescription {
    /// Captured native definition, including raw type/width/flag metadata.
    pub fn schema(&self) -> &TableSchema {
        &self.schema
    }
    /// Whether the nonempty schema contains only currently supported scalar
    /// carriers. Record contents/layout histories and native declarations remain unchecked.
    pub fn scalar_columns_supported(&self) -> bool {
        !self.schema.columns().is_empty() && self.unsupported_columns().next().is_none()
    }
    /// Columns excluded by the engine's authoritative scalar carrier classifier.
    /// No unsupported column is dropped or replaced with a stored NULL.
    pub fn unsupported_columns(&self) -> impl Iterator<Item = &NativeColumn> {
        self.schema
            .columns()
            .iter()
            .filter(|column| column.scalar_type().is_none())
    }
}
