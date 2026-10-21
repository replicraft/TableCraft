use crate::NativeSortOrder;
use tablecraft_schema::Column;

/// Qualified stored multivalue element metadata, named for its parent field.
/// The order comes from the actual flat value field, never the type template or
/// database default. Unknown descriptors remain explicit and activate no comparer.
pub struct MultivalueDeclaration {
    pub(super) column: Column,
    pub(super) order: Option<NativeSortOrder>,
}
impl MultivalueDeclaration {
    /// Shared scalar declaration for the parent's stored .Value view.
    pub fn column(&self) -> &Column {
        &self.column
    }
    /// Exact flat value-field text order, or None for a non-Text carrier.
    pub const fn text_sort_order(&self) -> Option<NativeSortOrder> {
        self.order
    }
    /// Consume the metadata wrapper without cloning its shared declaration.
    pub fn into_column(self) -> Column {
        self.column
    }
}
