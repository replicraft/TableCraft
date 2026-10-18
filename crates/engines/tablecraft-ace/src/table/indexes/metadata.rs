use super::super::schema::TableSchema;
use tablecraft_schema::SchemaName;

/// One native key column, in key order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IndexColumn {
    pub(super) number: usize,
    pub(super) descending: bool,
}
impl IndexColumn {
    /// Native logical column number, not vector/display position.
    pub const fn number(self) -> usize {
        self.number
    }
    /// Whether this key component uses descending native order.
    pub const fn descending(self) -> bool {
        self.descending
    }
}
/// Native index definition; names and physical references stay format-specific.
pub struct NativeIndex {
    pub(super) name: SchemaName,
    pub(super) number: u32,
    pub(super) data_number: u32,
    pub(super) root: u32,
    pub(super) usage: u32,
    pub(super) kind: u8,
    pub(super) flags: u8,
    pub(super) backing_primary: bool,
    pub(super) columns: Vec<IndexColumn>,
}
impl NativeIndex {
    /// Original validated native name.
    pub fn name(&self) -> &str {
        self.name.as_str()
    }
    /// Logical index number.
    pub const fn number(&self) -> u32 {
        self.number
    }
    /// Physical index-data number; multiple logical indexes may share it.
    pub const fn data_number(&self) -> u32 {
        self.data_number
    }
    /// Root page, not a mutation handle or proof of a valid tree.
    pub const fn root_page(&self) -> u32 {
        self.root
    }
    /// Packed owned-page usage-map reference.
    pub const fn usage_map(&self) -> u32 {
        self.usage
    }
    /// Native logical type (normal 0, primary 1, foreign reference 2).
    pub const fn native_type(&self) -> u8 {
        self.kind
    }
    /// Raw physical flags; no implicit uniqueness validation is performed.
    pub const fn flags(&self) -> u8 {
        self.flags
    }
    /// Key components in native definition order.
    pub fn columns(&self) -> &[IndexColumn] {
        &self.columns
    }
}
/// Completed native index metadata and its owning table schema.
pub struct IndexInspection {
    pub(super) schema: TableSchema,
    pub(super) indexes: Vec<NativeIndex>,
}
impl IndexInspection {
    /// Owning table metadata.
    pub fn schema(&self) -> &TableSchema {
        &self.schema
    }
    /// Logical indexes in native definition order, including foreign references.
    pub fn indexes(&self) -> &[NativeIndex] {
        &self.indexes
    }
}
