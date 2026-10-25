use super::{ExternalNativeSource, LocalSource};
use tablecraft_core::values::Value;
use tablecraft_query::{
    QueryError,
    source::{
        ExternalSourceRequest, MultivalueVisitor, ProjectedRow, ResolvedExternalSource,
        ScanConstraint, Source, TextCollationId,
    },
};
use tablecraft_schema::{CollectionMember, Column, SchemaName};
use tablecraft_sql::query::Query;

impl Source for LocalSource<'_> {
    fn query(&self, name: &str) -> Result<Option<&Query>, QueryError> {
        self.handle.check()?;
        self.handle.source().query(name)
    }

    fn text_equality_hints(&self, table: &str) -> Result<bool, QueryError> {
        let (source, table) = self.route(table)?;
        source.text_equality_hints(table)
    }

    fn columns(&self, table: &str) -> Result<&[Column], QueryError> {
        let (source, table) = self.route(table)?;
        source.columns(table)
    }

    fn text_collations(&self, table: &str) -> Result<&[Option<TextCollationId>], QueryError> {
        let (source, table) = self.route(table)?;
        source.text_collations(table)
    }

    fn unavailable_columns(&self, table: &str) -> Result<&[SchemaName], QueryError> {
        let (source, table) = self.route(table)?;
        source.unavailable_columns(table)
    }

    fn multivalue_columns(&self, table: &str) -> Result<&[Column], QueryError> {
        let (source, table) = self.route(table)?;
        source.multivalue_columns(table)
    }

    fn multivalue_text_collations(
        &self,
        table: &str,
    ) -> Result<&[Option<TextCollationId>], QueryError> {
        let (source, table) = self.route(table)?;
        source.multivalue_text_collations(table)
    }

    fn collection_members(&self, table: &str) -> Result<&[CollectionMember], QueryError> {
        let (source, table) = self.route(table)?;
        source.collection_members(table)
    }

    fn collection_member_text_collations(
        &self,
        table: &str,
    ) -> Result<&[Option<TextCollationId>], QueryError> {
        let (source, table) = self.route(table)?;
        source.collection_member_text_collations(table)
    }

    fn scan_multivalues(
        &self,
        table: &str,
        indices: &[usize],
        collections: &[usize],
        visitor: &mut MultivalueVisitor<'_>,
    ) -> Result<(), QueryError> {
        let (source, table) = self.route(table)?;
        source.scan_multivalues(table, indices, collections, visitor)
    }

    fn scan_collection_members(
        &self,
        table: &str,
        indices: &[usize],
        values: &[usize],
        members: &[usize],
        visitor: &mut MultivalueVisitor<'_>,
    ) -> Result<(), QueryError> {
        let (source, table) = self.route(table)?;
        source.scan_collection_members(table, indices, values, members, visitor)
    }

    fn scan(
        &self,
        table: &str,
        visitor: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        let (source, table) = self.route(table)?;
        source.scan(table, visitor)
    }

    fn scan_projected(
        &self,
        table: &str,
        indices: &[usize],
        constraint: Option<ScanConstraint>,
        visitor: &mut dyn FnMut(ProjectedRow<'_>) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        let (source, table) = self.route(table)?;
        source.scan_projected(table, indices, constraint, visitor)
    }

    fn scan_projected_progressive(
        &self,
        table: &str,
        indices: &[usize],
        visitor: &mut dyn FnMut(ProjectedRow<'_>) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        let (source, table) = self.route(table)?;
        source.scan_projected_progressive(table, indices, visitor)
    }

    fn scan_constrained(
        &self,
        table: &str,
        constraint: ScanConstraint,
        visitor: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        let (source, table) = self.route(table)?;
        source.scan_constrained(table, constraint, visitor)
    }
}

impl Source for ExternalNativeSource<'_> {
    fn external_source(
        &self,
        request: ExternalSourceRequest<'_>,
    ) -> Result<Option<ResolvedExternalSource<'_>>, QueryError> {
        self.resolve(request)
    }

    fn query(&self, name: &str) -> Result<Option<&Query>, QueryError> {
        self.root.handle().check()?;
        self.root.source().query(name)
    }

    fn text_equality_hints(&self, table: &str) -> Result<bool, QueryError> {
        self.root.handle().check()?;
        let source = self.root.source();
        source.text_equality_hints(table)
    }

    fn columns(&self, table: &str) -> Result<&[Column], QueryError> {
        self.root.handle().check()?;
        let source = self.root.source();
        source.columns(table)
    }

    fn text_collations(&self, table: &str) -> Result<&[Option<TextCollationId>], QueryError> {
        self.root.handle().check()?;
        let source = self.root.source();
        source.text_collations(table)
    }

    fn unavailable_columns(&self, table: &str) -> Result<&[SchemaName], QueryError> {
        self.root.handle().check()?;
        let source = self.root.source();
        source.unavailable_columns(table)
    }

    fn multivalue_columns(&self, table: &str) -> Result<&[Column], QueryError> {
        self.root.handle().check()?;
        let source = self.root.source();
        source.multivalue_columns(table)
    }

    fn multivalue_text_collations(
        &self,
        table: &str,
    ) -> Result<&[Option<TextCollationId>], QueryError> {
        self.root.handle().check()?;
        let source = self.root.source();
        source.multivalue_text_collations(table)
    }

    fn collection_members(&self, table: &str) -> Result<&[CollectionMember], QueryError> {
        self.root.handle().check()?;
        let source = self.root.source();
        source.collection_members(table)
    }

    fn collection_member_text_collations(
        &self,
        table: &str,
    ) -> Result<&[Option<TextCollationId>], QueryError> {
        self.root.handle().check()?;
        let source = self.root.source();
        source.collection_member_text_collations(table)
    }

    fn scan_multivalues(
        &self,
        table: &str,
        indices: &[usize],
        collections: &[usize],
        visitor: &mut MultivalueVisitor<'_>,
    ) -> Result<(), QueryError> {
        self.root.handle().check()?;
        let source = self.root.source();
        source.scan_multivalues(table, indices, collections, visitor)
    }

    fn scan_collection_members(
        &self,
        table: &str,
        indices: &[usize],
        values: &[usize],
        members: &[usize],
        visitor: &mut MultivalueVisitor<'_>,
    ) -> Result<(), QueryError> {
        self.root.handle().check()?;
        let source = self.root.source();
        source.scan_collection_members(table, indices, values, members, visitor)
    }

    fn scan(
        &self,
        table: &str,
        visitor: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        self.root.handle().check()?;
        let source = self.root.source();
        source.scan(table, visitor)
    }

    fn scan_projected(
        &self,
        table: &str,
        indices: &[usize],
        constraint: Option<ScanConstraint>,
        visitor: &mut dyn FnMut(ProjectedRow<'_>) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        self.root.handle().check()?;
        let source = self.root.source();
        source.scan_projected(table, indices, constraint, visitor)
    }

    fn scan_projected_progressive(
        &self,
        table: &str,
        indices: &[usize],
        visitor: &mut dyn FnMut(ProjectedRow<'_>) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        self.root.handle().check()?;
        let source = self.root.source();
        source.scan_projected_progressive(table, indices, visitor)
    }

    fn scan_constrained(
        &self,
        table: &str,
        constraint: ScanConstraint,
        visitor: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        self.root.handle().check()?;
        let source = self.root.source();
        source.scan_constrained(table, constraint, visitor)
    }
}
