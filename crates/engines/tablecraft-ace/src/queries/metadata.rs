use super::QueryAttribute;
use crate::{CatalogInspection, NativeObjectOrigin};
use tablecraft_model::ObjectDescriptor;

/// Completed inert native saved-query fragments. This is neither reconstructed
/// SQL nor a parsed/executable query, and does not authorize any action.
pub struct SavedQueryDefinition {
    pub(super) catalog: CatalogInspection,
    pub(super) position: usize,
    pub(super) rows: Vec<SavedQueryRow>,
}
impl SavedQueryDefinition {
    /// Scoped query identity/name and original containment metadata.
    pub fn object(&self) -> &ObjectDescriptor {
        &self.catalog.catalog().objects()[self.position]
    }
    /// Unmodified signed native ID and flags, not a physical table/page reference.
    pub fn origin(&self) -> &NativeObjectOrigin {
        &self.catalog.origins()[self.position]
    }
    /// Selected query's fragments in sequential native row order. Stored Order
    /// bytes remain separate; no clause ordering or completeness is inferred.
    pub fn rows(&self) -> &[SavedQueryRow] {
        &self.rows
    }
}

/// Exact nullable fields of one selected MSysQueries row. Strings are inert
/// stored fragments, not validated identifiers, connection settings or SQL.
pub struct SavedQueryRow {
    pub(super) object_id: i32,
    pub(super) attribute: Option<QueryAttribute>,
    pub(super) expression: Option<String>,
    pub(super) flag: Option<i16>,
    pub(super) extra: Option<i32>,
    pub(super) name1: Option<String>,
    pub(super) name2: Option<String>,
    pub(super) order: Option<Vec<u8>>,
}
impl SavedQueryRow {
    /// Exact native object ID, matched to this definition without page masking.
    pub const fn native_object_id(&self) -> i32 {
        self.object_id
    }
    /// Native fragment attribute, preserving NULL and unknown codes.
    pub const fn attribute(&self) -> Option<QueryAttribute> {
        self.attribute
    }
    /// Stored expression text; never evaluated by this reader.
    pub fn expression(&self) -> Option<&str> {
        self.expression.as_deref()
    }
    /// Exact signed native flag field, without defaults or bit interpretation.
    pub const fn flag(&self) -> Option<i16> {
        self.flag
    }
    /// Exact signed LvExtra scalar metadata; unrelated to object payload streams.
    pub const fn extra(&self) -> Option<i32> {
        self.extra
    }
    /// First inert name/metadata field; NULL and empty remain distinct.
    pub fn name1(&self) -> Option<&str> {
        self.name1.as_deref()
    }
    /// Second inert name/metadata field; NULL and empty remain distinct.
    pub fn name2(&self) -> Option<&str> {
        self.name2.as_deref()
    }
    /// Original nullable binary ordering key, without sorting or coercion.
    pub fn order(&self) -> Option<&[u8]> {
        self.order.as_deref()
    }
}
