mod attachments;
mod encoding;
mod history;
mod links;
mod multivalue;
mod page;
mod resolve;
mod value_schema;
pub use attachments::{AttachmentColumn, StoredAttachment, read_attachment_column};
pub(super) use encoding::supported_key;
pub use history::{StoredVersion, VersionHistoryColumn, read_version_history_column};
pub use links::{
    ComplexColumnLinks, ComplexParentLink, ComplexReferenceViolation, read_complex_column_links,
};
pub use multivalue::{MultivalueColumn, inspect_multivalue_columns, read_multivalue_column};
pub use multivalue::{
    MultivaluePageCell, MultivaluePageValue, MultivalueTablePage, read_multivalue_table_page,
};
pub use resolve::inspect_complex_schema;

use super::TableSchema;

/// Resolved complex-field declarations and their checked native table references.
/// No parent foreign keys, child values or attachment payloads are decoded.
pub struct ComplexTableSchema {
    pub(super) owner: TableSchema,
    pub(super) columns: Vec<ComplexColumnSchema>,
}
impl ComplexTableSchema {
    /// Original parent schema, including fields outside the scalar read profile.
    pub fn owner(&self) -> &TableSchema {
        &self.owner
    }
    /// Resolved complex columns in native logical-number order.
    pub fn columns(&self) -> &[ComplexColumnSchema] {
        &self.columns
    }
}

/// Complex declaration linked through MSysComplexColumns and native catalog IDs.
pub struct ComplexColumnSchema {
    pub(super) number: usize,
    pub(super) type_id: u32,
    pub(super) type_table: TableSchema,
    pub(super) flat_table: TableSchema,
    pub(super) foreign_key: usize,
    pub(super) identity: usize,
}
impl ComplexColumnSchema {
    /// Parent's native column number, not its display/vector index.
    pub const fn number(&self) -> usize {
        self.number
    }
    /// Raw complex-type identifier matched to one system declaration.
    pub const fn type_id(&self) -> u32 {
        self.type_id
    }
    /// Native conceptual value-type template; no data records are read.
    pub fn type_table(&self) -> &TableSchema {
        &self.type_table
    }
    /// Native flat child table; no scalar snapshot or collection is implied.
    pub fn flat_table(&self) -> &TableSchema {
        &self.flat_table
    }
    /// Checked child foreign-key native column number.
    pub const fn foreign_key_number(&self) -> usize {
        self.foreign_key
    }
    /// Checked child identity native column number.
    pub const fn identity_number(&self) -> usize {
        self.identity
    }
}

pub use page::{
    StoredCollectionColumn, StoredCollectionItem, StoredCollectionPageCell,
    StoredCollectionPageValue, StoredCollectionTablePage, read_stored_collection_table_page,
};
