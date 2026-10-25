use super::RecomputedNativeSource;
use tablecraft_ace::NativeReadSource;
use tablecraft_core::values::Value;
use tablecraft_query::{
    QueryError,
    source::{MultivalueVisitor, ProjectedRow, ScanConstraint, Source, TextCollationId},
};
use tablecraft_schema::{CollectionMember, Column, SchemaName};
use tablecraft_sql::query::Query;

impl<R: NativeReadSource> Source for RecomputedNativeSource<'_, '_, R> {
    fn query(&self, name: &str) -> Result<Option<&Query>, QueryError> {
        Self::tick(self.source)?;
        for view in &self.views {
            Self::tick(self.source)?;
            if view.name.eq_ignore_ascii_case(name) {
                return Ok(Some(&view.query));
            }
        }
        self.source.query(name)
    }

    fn text_equality_hints(&self, table: &str) -> Result<bool, QueryError> {
        self.source.text_equality_hints(table)
    }

    fn columns(&self, table: &str) -> Result<&[Column], QueryError> {
        self.source.columns(table)
    }

    fn text_collations(&self, table: &str) -> Result<&[Option<TextCollationId>], QueryError> {
        self.source.text_collations(table)
    }

    fn unavailable_columns(&self, table: &str) -> Result<&[SchemaName], QueryError> {
        self.source.unavailable_columns(table)
    }

    fn multivalue_columns(&self, table: &str) -> Result<&[Column], QueryError> {
        self.source.multivalue_columns(table)
    }

    fn multivalue_text_collations(
        &self,
        table: &str,
    ) -> Result<&[Option<TextCollationId>], QueryError> {
        self.source.multivalue_text_collations(table)
    }

    fn collection_members(&self, table: &str) -> Result<&[CollectionMember], QueryError> {
        self.source.collection_members(table)
    }

    fn collection_member_text_collations(
        &self,
        table: &str,
    ) -> Result<&[Option<TextCollationId>], QueryError> {
        self.source.collection_member_text_collations(table)
    }

    fn scan_multivalues(
        &self,
        table: &str,
        indices: &[usize],
        collections: &[usize],
        visitor: &mut MultivalueVisitor<'_>,
    ) -> Result<(), QueryError> {
        self.source
            .scan_multivalues(table, indices, collections, visitor)
    }

    fn scan_collection_members(
        &self,
        table: &str,
        indices: &[usize],
        values: &[usize],
        members: &[usize],
        visitor: &mut MultivalueVisitor<'_>,
    ) -> Result<(), QueryError> {
        self.source
            .scan_collection_members(table, indices, values, members, visitor)
    }

    fn scan(
        &self,
        table: &str,
        visitor: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        self.source.scan(table, visitor)
    }

    fn scan_projected(
        &self,
        table: &str,
        indices: &[usize],
        constraint: Option<ScanConstraint>,
        visitor: &mut dyn FnMut(ProjectedRow<'_>) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        self.source
            .scan_projected(table, indices, constraint, visitor)
    }

    fn scan_projected_progressive(
        &self,
        table: &str,
        indices: &[usize],
        visitor: &mut dyn FnMut(ProjectedRow<'_>) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        self.source
            .scan_projected_progressive(table, indices, visitor)
    }

    fn scan_constrained(
        &self,
        table: &str,
        constraint: ScanConstraint,
        visitor: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        self.source.scan_constrained(table, constraint, visitor)
    }
}
